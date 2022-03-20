use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use memo_map::MemoMap;
use self_cell::self_cell;

use crate::environment::CompiledTemplate;
use crate::error::{Error, ErrorKind};
use crate::value::RcType;

type LoadFunc = dyn for<'a> Fn(&'a str) -> Result<String, Error> + Send + Sync;

/// Utility for dynamic template loading.
///
/// Because an [`Environment`](crate::Environment) holds a reference to the
/// source lifetime it borrows templates from, it becomes very inconvenient when
/// it is shared. This object provides a solution for such cases. First templates
/// are loaded into the source, then it can be set as the "source" for an
/// environment decouping the lifetimes.  Note that once a source has been added
/// to an environment methods such as
/// [`Environment::add_template`](crate::Environment::add_template) must no
/// longer be used as otherwise the same lifetime concern arises.
///
/// Alternatively sources can also be used to implement completely dynamic template
/// lookups by using [`with_loader`](Source::with_loader) in which case templates
/// are loaded on first use.
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "source")))]
pub struct Source {
    backing: SourceBacking,
}

#[derive(Clone)]
enum SourceBacking {
    Dynamic {
        templates: MemoMap<String, RcType<LoadedTemplate>>,
        loader: Arc<LoadFunc>,
    },
    Static {
        templates: HashMap<String, RcType<LoadedTemplate>>,
    },
}

impl Default for Source {
    fn default() -> Source {
        Source::new()
    }
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.backing {
            SourceBacking::Dynamic { templates, .. } => f
                .debug_list()
                .entries(templates.iter().map(|x| x.0))
                .finish(),
            SourceBacking::Static { templates } => f
                .debug_list()
                .entries(templates.iter().map(|x| x.0))
                .finish(),
        }
    }
}

self_cell! {
    struct LoadedTemplate {
        owner: (String, String),
        #[covariant]
        dependent: CompiledTemplate,
    }
}

impl fmt::Debug for LoadedTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.borrow_dependent(), f)
    }
}

impl Source {
    /// Creates an empty source.
    ///
    /// ```rust
    /// # use minijinja::{Source, Environment};
    /// fn create_env() -> Environment<'static> {
    ///     let mut env = Environment::new();
    ///     let mut source = Source::new();
    ///     source.load_from_path("templates", &["html"]).unwrap();
    ///     env.set_source(source);
    ///     env
    /// }
    /// ```
    pub fn new() -> Source {
        Source {
            backing: SourceBacking::Static {
                templates: HashMap::new(),
            },
        }
    }

    /// Creates a source with a dynamic loader.
    ///
    /// When a source was created with the loader, the source gains the ability
    /// to dynamically load templates.  The loader is invoked with the name of
    /// the template.  If this template exists `Ok(Some(template_source))` has
    /// to be returned, otherwise `Ok(None)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use minijinja::{Source, Environment};
    /// fn create_env() -> Environment<'static> {
    ///     let mut env = Environment::new();
    ///     env.set_source(Source::with_loader(|name| {
    ///         if name == "layout.html" {
    ///             Ok(Some("...".into()))
    ///         } else {
    ///             Ok(None)
    ///         }
    ///     }));
    ///     env
    /// }
    /// ```
    pub fn with_loader<F>(f: F) -> Source
    where
        F: Fn(&str) -> Result<Option<String>, Error> + Send + Sync + 'static,
    {
        Source {
            backing: SourceBacking::Dynamic {
                templates: MemoMap::new(),
                loader: Arc::new(move |name| match f(name)? {
                    Some(rv) => Ok(rv),
                    None => Err(Error::new_not_found(name)),
                }),
            },
        }
    }

    /// Adds a new template into the source.
    ///
    /// This is similar to the method of the same name on the environment but
    /// the source is held within the [`Source`] object for you.  This means
    /// that lifetimes are not a concern.
    pub fn add_template<N: Into<String>, S: Into<String>>(
        &mut self,
        name: N,
        source: S,
    ) -> Result<(), Error> {
        let source = source.into();
        let name = name.into();
        let owner = (name.clone(), source);
        let tmpl = LoadedTemplate::try_new(owner, |(name, source)| -> Result<_, Error> {
            CompiledTemplate::from_name_and_source(name.as_str(), source)
        })?;

        match self.backing {
            SourceBacking::Dynamic { ref templates, .. } => {
                templates.insert(name, RcType::new(tmpl));
            }
            SourceBacking::Static { ref mut templates } => {
                templates.insert(name, RcType::new(tmpl));
            }
        }
        Ok(())
    }

    /// Removes an already loaded template from the source.
    pub fn remove_template(&mut self, name: &str) {
        match &mut self.backing {
            SourceBacking::Dynamic { templates, .. } => {
                templates.remove(name);
            }
            SourceBacking::Static { templates } => {
                templates.remove(name);
            }
        }
    }

    /// Loads templates from a path.
    ///
    /// This function takes two arguments: `path` which is the path to where the templates are
    /// stored and `extensions` which is a list of file extensions that should be considered to
    /// be templates.  Hidden files are always ignored.
    pub fn load_from_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        extensions: &[&str],
    ) -> Result<(), Error> {
        let path = fs::canonicalize(&path).map_err(|err| {
            Error::new(ErrorKind::InvalidOperation, "unable to load template").with_source(err)
        })?;

        fn walk(
            source: &mut Source,
            root: &Path,
            dir: &Path,
            extensions: &[&str],
        ) -> Result<(), Error> {
            if dir.is_dir() {
                for entry in fs::read_dir(dir).map_err(|err| {
                    Error::new(ErrorKind::InvalidOperation, "failed to walk directory")
                        .with_source(err)
                })? {
                    let entry = entry.map_err(|err| {
                        Error::new(ErrorKind::InvalidOperation, "failed to walk directory")
                            .with_source(err)
                    })?;
                    let path = entry.path();

                    let filename = match path.file_name().and_then(|x| x.to_str()) {
                        Some(filename) => filename,
                        None => continue,
                    };

                    if filename.starts_with('.') {
                        continue;
                    }

                    if path.is_dir() {
                        walk(source, root, &path, extensions)?;
                    } else if extensions.contains(&filename.rsplit('.').next().unwrap_or("")) {
                        let name = path
                            .strip_prefix(root)
                            .unwrap()
                            .display()
                            .to_string()
                            .replace('\\', "/");
                        source.add_template(
                            name,
                            fs::read_to_string(path).map_err(|err| {
                                Error::new(
                                    ErrorKind::TemplateNotFound,
                                    "unable to load template from file system",
                                )
                                .with_source(err)
                            })?,
                        )?;
                    }
                }
            }
            Ok(())
        }

        walk(self, &path, &path, extensions)
    }

    /// Gets a compiled template from the source.
    pub(crate) fn get_compiled_template(&self, name: &str) -> Result<&CompiledTemplate<'_>, Error> {
        match &self.backing {
            SourceBacking::Dynamic { templates, loader } => Ok(templates
                .get_or_try_insert(name, || -> Result<_, Error> {
                    let source = loader(name)?;
                    let owner = (name.to_owned(), source);
                    let tmpl =
                        LoadedTemplate::try_new(owner, |(name, source)| -> Result<_, Error> {
                            CompiledTemplate::from_name_and_source(name.as_str(), source)
                        })?;
                    Ok(RcType::new(tmpl))
                })?
                .borrow_dependent()),
            SourceBacking::Static { templates } => templates
                .get(name)
                .map(|value| value.borrow_dependent())
                .ok_or_else(|| Error::new_not_found(name)),
        }
    }
}
