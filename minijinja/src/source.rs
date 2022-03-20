use std::fmt;
use std::fs;
use std::hash::Hash;
use std::path::Path;
use std::sync::Arc;

use memo_map::MemoMap;
use self_cell::self_cell;

use crate::environment::CompiledTemplate;
use crate::error::{Error, ErrorKind};
use crate::value::RcType;

type LoadFunc = dyn for<'a> Fn(&'a str) -> Result<String, Error>;

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
/// # Example
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
#[derive(Clone, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "source")))]
pub struct Source {
    templates: MemoMap<String, RcType<LoadedTemplate>>,
    loader: Option<Arc<LoadFunc>>,
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        pub struct KeysDebug<'a, K: fmt::Debug, V>(pub &'a MemoMap<K, V>);

        impl<'a, K: Hash + Eq + fmt::Debug, V> fmt::Debug for KeysDebug<'a, K, V> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.iter().map(|x| x.0)).finish()
            }
        }
        fmt::Debug::fmt(&KeysDebug(&self.templates), f)
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
    pub fn new() -> Source {
        Source::default()
    }

    /// Sets a dynamic loader function.
    ///
    /// When a loader is set the source gains the ability to dynamically
    /// load templates.  The loader is invoked with the name of the template.
    /// If this template exists `Ok(Some(template_source))` has to be returned,
    /// otherwise `Ok(None)`.  It's also possible to signal out other errors.
    pub fn set_loader<F>(&mut self, f: F)
    where
        F: Fn(&str) -> Result<Option<String>, Error> + 'static,
    {
        self.loader = Some(Arc::new(move |name| match f(name)? {
            Some(rv) => Ok(rv),
            None => Err(Error::new_not_found(name)),
        }));
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
        self.templates.insert(name, RcType::new(tmpl));
        Ok(())
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
        if let Some(ref loader) = self.loader {
            Ok(self
                .templates
                .get_or_try_insert(name, || -> Result<_, Error> {
                    let source = loader(name)?;
                    let owner = (name.to_owned(), source);
                    let tmpl =
                        LoadedTemplate::try_new(owner, |(name, source)| -> Result<_, Error> {
                            CompiledTemplate::from_name_and_source(name.as_str(), source)
                        })?;
                    Ok(RcType::new(tmpl))
                })?
                .borrow_dependent())
        } else {
            self.templates
                .get(name)
                .map(|value| value.borrow_dependent())
                .ok_or_else(|| Error::new_not_found(name))
        }
    }
}
