use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
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

type LoadFunc = dyn for<'a> Fn(&'a str) -> Result<Option<String>, Error> + Send + Sync;

/// Internal utility for dynamic template loading.
///
/// Because an [`Environment`](crate::Environment) holds a reference to the
/// source lifetime it borrows templates from, it becomes very inconvenient when
/// it is shared. This object provides a solution for such cases. First templates
/// are loaded into the source to decouple the lifetimes from the environment.
#[derive(Clone, Default)]
pub(crate) struct LoaderSource<'source> {
    pub syntax_config: SyntaxConfig,
    loader: Option<Arc<LoadFunc>>,
    owned_templates: MemoMap<Arc<str>, Arc<LoadedTemplate>>,
    borrowed_templates: BTreeMap<&'source str, Arc<CompiledTemplate<'source>>>,
}

impl<'source> fmt::Debug for LoaderSource<'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut l = f.debug_list();
        for key in self.owned_templates.keys() {
            l.entry(key);
        }
        for key in self.borrowed_templates.keys() {
            if !self.owned_templates.contains_key(*key) {
                l.entry(key);
            }
        }
        l.finish()
    }
}

self_cell! {
    struct LoadedTemplate {
        owner: (Arc<str>, Box<str>),
        #[covariant]
        dependent: CompiledTemplate,
    }
}

impl fmt::Debug for LoadedTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.borrow_dependent(), f)
    }
}

impl<'source> LoaderSource<'source> {
    pub fn set_loader<F>(&mut self, f: F)
    where
        F: Fn(&str) -> Result<Option<String>, Error> + Send + Sync + 'static,
    {
        self.loader = Some(Arc::new(f));
    }

    pub fn add_template<N, S>(&mut self, name: N, source: S) -> Result<(), Error>
    where
        N: Into<Cow<'source, str>>,
        S: Into<Cow<'source, str>>,
    {
        match (source.into(), name.into()) {
            (Cow::Borrowed(source), Cow::Borrowed(name)) => {
                self.owned_templates.remove(name);
                self.borrowed_templates.insert(
                    name,
                    Arc::new(ok!(CompiledTemplate::from_name_and_source_with_syntax(
                        name,
                        source,
                        self.syntax_config.clone()
                    ))),
                );
            }
            (source, name) => {
                self.borrowed_templates.remove(&name as &str);
                let name: Arc<str> = name.into();
                self.owned_templates.replace(
                    name.clone(),
                    ok!(self.make_owned_template(name, source.to_string())),
                );
            }
        }

        Ok(())
    }

    fn make_owned_template(
        &self,
        name: Arc<str>,
        source: String,
    ) -> Result<Arc<LoadedTemplate>, Error> {
        LoadedTemplate::try_new(
            (name, source.into_boxed_str()),
            |(name, source)| -> Result<_, Error> {
                CompiledTemplate::from_name_and_source_with_syntax(
                    name,
                    source,
                    self.syntax_config.clone(),
                )
            },
        )
        .map(Arc::new)
    }

    pub fn remove_template(&mut self, name: &str) {
        self.borrowed_templates.remove(name);
        self.owned_templates.remove(name);
    }

    pub fn clear_templates(&mut self) {
        self.borrowed_templates.clear();
        self.owned_templates.clear();
    }

    pub fn get_compiled_template(&self, name: &str) -> Result<&CompiledTemplate<'_>, Error> {
        if let Some(rv) = self.borrowed_templates.get(name) {
            Ok(&**rv)
        } else {
            let name: Arc<str> = name.into();
            self.owned_templates
                .get_or_try_insert(&name.clone(), || -> Result<_, Error> {
                    let loader_result = match self.loader {
                        Some(ref loader) => ok!(loader(&name)),
                        None => None,
                    }
                    .ok_or_else(|| Error::new_not_found(&name));
                    self.make_owned_template(name, ok!(loader_result))
                })
                .map(|x| x.borrow_dependent())
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
