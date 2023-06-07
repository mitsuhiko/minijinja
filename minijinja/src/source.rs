use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use memo_map::MemoMap;
use self_cell::self_cell;

use crate::compiler::lexer::SyntaxConfig;
use crate::error::{Error, ErrorKind};
use crate::template::CompiledTemplate;

#[cfg(test)]
use similar_asserts::assert_eq;

type LoadFunc = dyn for<'a> Fn(&'a str) -> Result<String, Error> + Send + Sync;

/// Internal utility for dynamic template loading.
///
/// Because an [`Environment`](crate::Environment) holds a reference to the
/// source lifetime it borrows templates from, it becomes very inconvenient when
/// it is shared. This object provides a solution for such cases. First templates
/// are loaded into the source to decouple the lifetimes from the environment.
#[derive(Clone)]
pub(crate) struct Source {
    backing: SourceBacking,
}

#[derive(Clone)]
enum SourceBacking {
    Dynamic {
        templates: MemoMap<String, Arc<LoadedTemplate>>,
        loader: Arc<LoadFunc>,
        syntax: SyntaxConfig,
    },
    Static {
        templates: BTreeMap<String, Arc<LoadedTemplate>>,
        syntax: SyntaxConfig,
    },
}

impl Default for Source {
    fn default() -> Source {
        Source {
            backing: SourceBacking::Static {
                templates: Default::default(),
                syntax: Default::default(),
            },
        }
    }
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.backing {
            SourceBacking::Dynamic { templates, .. } => f
                .debug_list()
                .entries(templates.iter().map(|x| x.0))
                .finish(),
            SourceBacking::Static { templates, .. } => f
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
    /// Sets the syntax for the source.
    #[cfg(feature = "custom_syntax")]
    pub fn set_syntax(&mut self, new_syntax: crate::custom_syntax::Syntax) -> Result<(), Error> {
        match self.backing {
            SourceBacking::Dynamic { ref mut syntax, .. }
            | SourceBacking::Static { ref mut syntax, .. } => {
                *syntax = ok!(new_syntax.compile());
            }
        }
        Ok(())
    }

    pub(crate) fn _syntax_config(&self) -> &SyntaxConfig {
        match &self.backing {
            SourceBacking::Dynamic { ref syntax, .. }
            | SourceBacking::Static { ref syntax, .. } => syntax,
        }
    }

    /// Reconfigures the source with a new loader.
    pub fn set_loader<F>(&mut self, f: F)
    where
        F: Fn(&str) -> Result<Option<String>, Error> + Send + Sync + 'static,
    {
        // Simple case: we already have dynamic backing, swap out the loader
        if let SourceBacking::Dynamic { ref mut loader, .. } = self.backing {
            *loader = Arc::new(move |name| match ok!(f(name)) {
                Some(rv) => Ok(rv),
                None => Err(Error::new_not_found(name)),
            });

        // complex case: we need to migrate static backing to dynamic backing.
        // This requires some swapping hackery
        } else if let SourceBacking::Static {
            templates: old_templates,
            syntax: old_syntax,
        } = mem::replace(
            &mut self.backing,
            SourceBacking::Dynamic {
                templates: MemoMap::new(),
                loader: Arc::new(move |name| match ok!(f(name)) {
                    Some(rv) => Ok(rv),
                    None => Err(Error::new_not_found(name)),
                }),
                syntax: Default::default(),
            },
        ) {
            if let SourceBacking::Dynamic {
                ref templates,
                ref mut syntax,
                ..
            } = self.backing
            {
                for (key, value) in old_templates.into_iter() {
                    templates.insert(key, value);
                }
                *syntax = old_syntax;
            } else {
                unreachable!();
            }
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
        let tmpl = ok!(LoadedTemplate::try_new(
            owner,
            |(name, source)| -> Result<_, Error> {
                CompiledTemplate::from_name_and_source_with_syntax(
                    name.as_str(),
                    source,
                    self._syntax_config().clone(),
                )
            }
        ));

        match self.backing {
            SourceBacking::Dynamic {
                ref mut templates, ..
            } => {
                templates.replace(name, Arc::new(tmpl));
            }
            SourceBacking::Static {
                ref mut templates, ..
            } => {
                templates.insert(name, Arc::new(tmpl));
            }
        }
        Ok(())
    }

    /// Removes an already loaded template from the source.
    pub fn remove_template(&mut self, name: &str) {
        match &mut self.backing {
            SourceBacking::Dynamic { templates, .. } => templates.remove(name),
            SourceBacking::Static { templates, .. } => templates.remove(name),
        };
    }

    /// Gets a compiled template from the source.
    pub(crate) fn get_compiled_template(&self, name: &str) -> Result<&CompiledTemplate<'_>, Error> {
        match &self.backing {
            SourceBacking::Dynamic {
                templates,
                loader,
                syntax,
            } => Ok(
                ok!(templates.get_or_try_insert(name, || -> Result<_, Error> {
                    let syntax = syntax.clone();
                    let source = ok!(loader(name));
                    let owner = (name.to_owned(), source);
                    let tmpl = ok!(LoadedTemplate::try_new(
                        owner,
                        |(name, source)| -> Result<_, Error> {
                            CompiledTemplate::from_name_and_source_with_syntax(
                                name.as_str(),
                                source,
                                syntax,
                            )
                        }
                    ));
                    Ok(Arc::new(tmpl))
                }))
                .borrow_dependent(),
            ),
            SourceBacking::Static { templates, .. } => templates
                .get(name)
                .map(|value| value.borrow_dependent())
                .ok_or_else(|| Error::new_not_found(name)),
        }
    }

    pub fn clear_templates(&mut self) {
        match &mut self.backing {
            SourceBacking::Dynamic { templates, .. } => {
                templates.clear();
            }
            SourceBacking::Static { templates, .. } => {
                templates.clear();
            }
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

/// Helper to load templates from a given directory.
///
/// This creates a dynamic loader which looks up templates in the
/// given directory.  Templates that start with a dot (`.`) or are contained in
/// a folder starting with a dot cannot be loaded.
///
/// # Example
///
/// ```rust
/// # use minijinja::{path_loader, Environment};
/// fn create_env() -> Environment<'static> {
///     let mut env = Environment::new();
///     env.set_loader(path_loader("path/to/templates"));
///     env
/// }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "loader")))]
pub fn path_loader<'x, P: AsRef<Path> + 'x>(
    dir: P,
) -> impl for<'a> Fn(&'a str) -> Result<Option<String>, Error> + Send + Sync + 'static {
    let dir = dir.as_ref().to_path_buf();
    move |name| {
        let path = match safe_join(&dir, name) {
            Some(path) => path,
            None => return Ok(None),
        };
        match fs::read_to_string(path) {
            Ok(result) => Ok(Some(result)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(
                Error::new(ErrorKind::InvalidOperation, "could not read template").with_source(err),
            ),
        }
    }
}

#[test]
fn test_source_replace_static() {
    let mut env = crate::Environment::new();
    env.add_template_owned("a", "1").unwrap();
    env.add_template_owned("a", "2").unwrap();
    let rv = env.get_template("a").unwrap().render(()).unwrap();
    assert_eq!(rv, "2");
}

#[test]
fn test_source_replace_dynamic() {
    let mut env = crate::Environment::new();
    env.add_template("a", "1").unwrap();
    env.add_template("a", "2").unwrap();
    env.set_loader(|_| Ok(None));
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
