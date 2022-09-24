use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use memo_map::MemoMap;
use self_cell::self_cell;

use crate::error::{Error, ErrorKind};
use crate::template::CompiledTemplate;

#[cfg(test)]
use similar_asserts::assert_eq;

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
        templates: MemoMap<String, Arc<LoadedTemplate>>,
        loader: Arc<LoadFunc>,
    },
    Static {
        templates: HashMap<String, Arc<LoadedTemplate>>,
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
    ///     source.add_template("index.html", "...").unwrap();
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
                loader: Arc::new(move |name| match ok!(f(name)) {
                    Some(rv) => Ok(rv),
                    None => Err(Error::new_not_found(name)),
                }),
            },
        }
    }

    /// Creates a source that loads on demand from a given directory.
    ///
    /// This creates a source with a dynamic loader which looks up templates in the
    /// given directory.  Templates that start with a dot (`.`) or are contained in
    /// a folder starting with a dot cannot be loaded.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use minijinja::{Source, Environment};
    /// fn create_env() -> Environment<'static> {
    ///     let mut env = Environment::new();
    ///     env.set_source(Source::from_path("path/to/templates"));
    ///     env
    /// }
    /// ```
    pub fn from_path<P: AsRef<Path>>(dir: P) -> Source {
        let dir = dir.as_ref().to_path_buf();
        Source::with_loader(move |name| {
            let path = match safe_join(&dir, name) {
                Some(path) => path,
                None => return Ok(None),
            };
            match fs::read_to_string(path) {
                Ok(result) => Ok(Some(result)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(
                    Error::new(ErrorKind::InvalidOperation, "could not read template")
                        .with_source(err),
                ),
            }
        })
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
        let tmpl = ok!(LoadedTemplate::try_new(
            owner,
            |(name, source)| -> Result<_, Error> {
                CompiledTemplate::from_name_and_source(name.as_str(), source)
            }
        ));

        match self.backing {
            SourceBacking::Dynamic {
                ref mut templates, ..
            } => {
                templates.replace(name, Arc::new(tmpl));
            }
            SourceBacking::Static { ref mut templates } => {
                templates.insert(name, Arc::new(tmpl));
            }
        }
        Ok(())
    }

    /// Removes an already loaded template from the source.
    pub fn remove_template(&mut self, name: &str) {
        match &mut self.backing {
            SourceBacking::Dynamic { templates, .. } => templates.remove(name),
            SourceBacking::Static { templates } => templates.remove(name),
        };
    }

    /// Gets a compiled template from the source.
    pub(crate) fn get_compiled_template(&self, name: &str) -> Result<&CompiledTemplate<'_>, Error> {
        match &self.backing {
            SourceBacking::Dynamic { templates, loader } => Ok(ok!(templates.get_or_try_insert(
                name,
                || -> Result<_, Error> {
                    let source = ok!(loader(name));
                    let owner = (name.to_owned(), source);
                    let tmpl = ok!(LoadedTemplate::try_new(
                        owner,
                        |(name, source)| -> Result<_, Error> {
                            CompiledTemplate::from_name_and_source(name.as_str(), source)
                        }
                    ));
                    Ok(Arc::new(tmpl))
                }
            ))
            .borrow_dependent()),
            SourceBacking::Static { templates } => templates
                .get(name)
                .map(|value| value.borrow_dependent())
                .ok_or_else(|| Error::new_not_found(name)),
        }
    }
}

fn safe_join(base: &Path, template: &str) -> Option<PathBuf> {
    let mut rv = base.to_path_buf();
    for segment in template.split('/') {
        if segment.starts_with('.') || segment.contains('\\') {
            return None;
        }
        rv.push(segment);
    }
    Some(rv)
}

#[test]
fn test_source_replace_static() {
    let mut source = Source::new();
    source.add_template("a", "1").unwrap();
    source.add_template("a", "2").unwrap();
    let mut env = crate::Environment::new();
    env.set_source(source);
    let rv = env.get_template("a").unwrap().render(()).unwrap();
    assert_eq!(rv, "2");
}

#[test]
fn test_source_replace_dynamic() {
    let mut source = Source::with_loader(|_| Ok(None));
    source.add_template("a", "1").unwrap();
    source.add_template("a", "2").unwrap();
    let mut env = crate::Environment::new();
    env.set_source(source);
    let rv = env.get_template("a").unwrap().render(()).unwrap();
    assert_eq!(rv, "2");
}

#[test]
fn test_safe_join() {
    assert_eq!(
        safe_join(Path::new("foo"), "bar/baz"),
        Some(PathBuf::from("foo").join("bar").join("baz"))
    );
    assert_eq!(safe_join(Path::new("foo"), ".bar/baz"), None);
    assert_eq!(safe_join(Path::new("foo"), "bar/.baz"), None);
    assert_eq!(safe_join(Path::new("foo"), "bar/../baz"), None);
}
