use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::vendor::self_cell::self_cell;

use crate::compiler::instructions::Instructions;
use crate::error::{Error, ErrorKind};
use crate::template::CompiledTemplate;
use crate::template::TemplateConfig;

type LoadFunc = dyn for<'a> Fn(&'a str) -> Result<Option<TemplateSource>, Error> + Send + Sync;
type UptodateFunc = dyn Fn() -> bool + Send + Sync;

/// The source of a template as returned by a loader.
///
/// A loader registered with [`Environment::set_loader`](crate::Environment::set_loader)
/// can either return the template source as a [`String`] or as a
/// `TemplateSource`.  The latter allows attaching an up-to-date check
/// with [`with_uptodate_check`](Self::with_uptodate_check) which is used
/// to automatically reload templates when
/// [auto reloading](crate::Environment::set_auto_reload) is enabled.
///
/// ```
/// # use minijinja::{Environment, TemplateSource};
/// # use std::sync::Arc;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// let changed = Arc::new(AtomicBool::new(false));
/// let mut env = Environment::new();
/// env.set_loader({
///     let changed = changed.clone();
///     move |name| {
///         let changed = changed.clone();
///         Ok(Some(
///             TemplateSource::new(format!("Hello from {name}!"))
///                 .with_uptodate_check(move || !changed.swap(false, Ordering::Relaxed)),
///         ))
///     }
/// });
/// ```
pub struct TemplateSource {
    source: String,
    uptodate: Option<Box<UptodateFunc>>,
}

impl TemplateSource {
    /// Creates a new template source from a string.
    pub fn new<S: Into<String>>(source: S) -> TemplateSource {
        TemplateSource {
            source: source.into(),
            uptodate: None,
        }
    }

    /// Attaches a check that reports if the loaded template is still up to date.
    ///
    /// When [auto reloading](crate::Environment::set_auto_reload) is enabled
    /// (the default) the environment invokes this callback every time the template is
    /// looked up.  If the callback returns `false` the loader is invoked again
    /// to load a fresh version of the template.
    pub fn with_uptodate_check<F>(mut self, f: F) -> TemplateSource
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.uptodate = Some(Box::new(f));
        self
    }

    /// Returns the source of the template.
    pub fn source(&self) -> &str {
        &self.source
    }
}

impl fmt::Debug for TemplateSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TemplateSource")
            .field("source", &self.source)
            .field("has_uptodate_check", &self.uptodate.is_some())
            .finish()
    }
}

impl From<String> for TemplateSource {
    fn from(source: String) -> TemplateSource {
        TemplateSource::new(source)
    }
}

impl From<&str> for TemplateSource {
    fn from(source: &str) -> TemplateSource {
        TemplateSource::new(source)
    }
}

impl From<Box<str>> for TemplateSource {
    fn from(source: Box<str>) -> TemplateSource {
        TemplateSource::new(source)
    }
}

impl From<Cow<'_, str>> for TemplateSource {
    fn from(source: Cow<'_, str>) -> TemplateSource {
        TemplateSource::new(source)
    }
}

/// Internal utility for dynamic template loading.
///
/// Because an [`Environment`](crate::Environment) holds a reference to the
/// source lifetime it borrows templates from, it becomes very inconvenient when
/// it is shared. This object provides a solution for such cases. First templates
/// are loaded into the source to decouple the lifetimes from the environment.
#[derive(Clone)]
pub(crate) struct LoaderStore<'source> {
    pub template_config: TemplateConfig,
    pub auto_reload: bool,
    loader: Option<Arc<LoadFunc>>,
    owned_templates: OwnedTemplates,
    borrowed_templates: BTreeMap<&'source str, Arc<CompiledTemplate<'source>>>,
}

impl fmt::Debug for LoaderStore<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut l = f.debug_list();
        let owned = self.owned_templates.current.lock().unwrap();
        for key in owned.keys() {
            l.entry(key);
        }
        for key in self.borrowed_templates.keys() {
            if !owned.contains_key(*key) {
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

self_cell! {
    pub(crate) struct OwnedInstructions {
        owner: Box<str>,
        #[covariant]
        dependent: Instructions,
    }
}

impl fmt::Debug for LoadedTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.borrow_dependent(), f)
    }
}

/// An owned template together with its optional up-to-date check.
struct CachedTemplate {
    template: LoadedTemplate,
    uptodate: Option<Box<UptodateFunc>>,
}

impl CachedTemplate {
    fn is_uptodate(&self) -> bool {
        match self.uptodate {
            Some(ref f) => f(),
            None => true,
        }
    }

    /// Returns the compiled template with an unbounded lifetime.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the `Arc` holding this template is kept
    /// alive for as long as the returned reference is used.
    unsafe fn compiled_unbounded<'a>(this: &Arc<CachedTemplate>) -> &'a CompiledTemplate<'a> {
        let ptr: *const CachedTemplate = Arc::as_ptr(this);
        (*ptr).template.borrow_dependent()
    }
}

/// Storage for owned templates.
///
/// References to templates in this storage are handed out with the lifetime
/// of a shared borrow of the store.  To allow templates to be replaced (eg:
/// for auto reloading) through a shared reference, replaced templates are
/// moved to the `stale` list rather than dropped.  The invariant is that a
/// template which was ever placed into `current` is only dropped through
/// `&mut self` as at that point no borrows can exist.
struct OwnedTemplates {
    current: Mutex<HashMap<Arc<str>, Arc<CachedTemplate>>>,
    stale: Mutex<Vec<Arc<CachedTemplate>>>,
}

impl Default for OwnedTemplates {
    fn default() -> OwnedTemplates {
        OwnedTemplates {
            current: Mutex::new(HashMap::new()),
            stale: Mutex::new(Vec::new()),
        }
    }
}

impl Clone for OwnedTemplates {
    fn clone(&self) -> OwnedTemplates {
        // stale templates are only kept alive for borrows of the original
        // store, so the clone does not need them.
        OwnedTemplates {
            current: Mutex::new(self.current.lock().unwrap().clone()),
            stale: Mutex::new(Vec::new()),
        }
    }
}

impl OwnedTemplates {
    fn current_mut(&mut self) -> &mut HashMap<Arc<str>, Arc<CachedTemplate>> {
        // we have exclusive access, so nothing can borrow stale templates.
        self.stale.get_mut().unwrap().clear();
        self.current.get_mut().unwrap()
    }

    fn get(&self, name: &str) -> Option<Arc<CachedTemplate>> {
        self.current.lock().unwrap().get(name).cloned()
    }

    /// Removes `expected` from the current templates if it's still the current one.
    fn retire(&self, name: &str, expected: &Arc<CachedTemplate>) {
        let mut current = self.current.lock().unwrap();
        if current.get(name).is_some_and(|x| Arc::ptr_eq(x, expected)) {
            let old = current.remove(name).unwrap();
            self.stale.lock().unwrap().push(old);
        }
    }

    /// Installs a freshly loaded template replacing `expected`.
    ///
    /// If another thread already replaced `expected` in the meantime, the
    /// template installed by that thread is returned instead.
    fn install(
        &self,
        name: Arc<str>,
        expected: Option<&Arc<CachedTemplate>>,
        new: Arc<CachedTemplate>,
    ) -> Arc<CachedTemplate> {
        let mut current = self.current.lock().unwrap();
        match current.entry(name) {
            Entry::Vacant(entry) => entry.insert(new).clone(),
            Entry::Occupied(mut entry) => {
                if expected.is_some_and(|x| Arc::ptr_eq(x, entry.get())) {
                    let old = entry.insert(new.clone());
                    self.stale.lock().unwrap().push(old);
                    new
                } else {
                    // another thread won the race.  Our template was never
                    // handed out so it can be dropped.
                    entry.get().clone()
                }
            }
        }
    }
}

impl<'source> LoaderStore<'source> {
    pub fn new(template_config: TemplateConfig) -> LoaderStore<'source> {
        LoaderStore {
            template_config,
            auto_reload: true,
            loader: None,
            owned_templates: OwnedTemplates::default(),
            borrowed_templates: BTreeMap::default(),
        }
    }

    pub fn insert(&mut self, name: &'source str, source: &'source str) -> Result<(), Error> {
        self.insert_cow(Cow::Borrowed(name), Cow::Borrowed(source))
    }

    pub fn insert_cow(
        &mut self,
        name: Cow<'source, str>,
        source: Cow<'source, str>,
    ) -> Result<(), Error> {
        match (source, name) {
            (Cow::Borrowed(source), Cow::Borrowed(name)) => {
                let compiled = ok!(CompiledTemplate::new(name, source, &self.template_config));
                self.owned_templates.current_mut().remove(name);
                self.borrowed_templates.insert(name, Arc::new(compiled));
            }
            (source, name) => {
                let name: Arc<str> = name.into();
                let template = ok!(self
                    .make_owned_template(name.clone(), TemplateSource::new(source.into_owned())));
                self.borrowed_templates.remove(&name as &str);
                self.owned_templates
                    .current_mut()
                    .insert(name, Arc::new(template));
            }
        }

        Ok(())
    }

    pub fn remove(&mut self, name: &str) {
        self.borrowed_templates.remove(name);
        self.owned_templates.current_mut().remove(name);
    }

    pub fn clear(&mut self) {
        self.borrowed_templates.clear();
        self.owned_templates.current_mut().clear();
    }

    pub fn get(&self, name: &str) -> Result<&CompiledTemplate<'_>, Error> {
        if let Some(rv) = self.borrowed_templates.get(name) {
            return Ok(&**rv);
        }

        let existing = self.owned_templates.get(name);
        if let Some(ref existing) = existing {
            if !self.auto_reload || existing.is_uptodate() {
                // SAFETY: the template is (or was) in `current`, so it is only
                // dropped through `&mut self`.
                return Ok(unsafe { CachedTemplate::compiled_unbounded(existing) });
            }
        }

        // The loader and compilation are intentionally invoked without holding
        // a lock so that slow loaders do not block lookups of other templates.
        let source = match self.loader {
            Some(ref loader) => ok!(loader(name)),
            None => None,
        };
        let Some(source) = source else {
            if let Some(ref existing) = existing {
                self.owned_templates.retire(name, existing);
            }
            return Err(Error::new_not_found(name));
        };
        let name: Arc<str> = name.into();
        let template = ok!(self.make_owned_template(name.clone(), source));
        let rv = self
            .owned_templates
            .install(name, existing.as_ref(), Arc::new(template));
        // SAFETY: `install` returns the template that is now in `current`.
        Ok(unsafe { CachedTemplate::compiled_unbounded(&rv) })
    }

    pub fn set_loader<F, R>(&mut self, f: F)
    where
        F: Fn(&str) -> Result<Option<R>, Error> + Send + Sync + 'static,
        R: Into<TemplateSource>,
    {
        self.loader = Some(Arc::new(move |name| f(name).map(|x| x.map(Into::into))));
    }

    fn make_owned_template(
        &self,
        name: Arc<str>,
        source: TemplateSource,
    ) -> Result<CachedTemplate, Error> {
        let template = ok!(LoadedTemplate::try_new(
            (name, source.source.into_boxed_str()),
            |(name, source)| -> Result<_, Error> {
                CompiledTemplate::new(name, source, &self.template_config)
            },
        ));
        Ok(CachedTemplate {
            template,
            uptodate: source.uptodate,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &CompiledTemplate<'_>)> {
        let borrowed = self
            .borrowed_templates
            .iter()
            .map(|(name, template)| (*name, &**template));

        let owned = self
            .owned_templates
            .current
            .lock()
            .unwrap()
            .values()
            .map(|template| {
                // SAFETY: the template is in `current`, so it is only dropped
                // through `&mut self`.
                let compiled = unsafe { CachedTemplate::compiled_unbounded(template) };
                (compiled.instructions.name(), compiled)
            })
            .collect::<Vec<_>>();

        borrowed.chain(owned)
    }
}

/// Safely joins two paths.
pub fn safe_join(base: &Path, template: &str) -> Option<PathBuf> {
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
/// The loaded templates carry an up-to-date check based on the modification
/// time and size of the file.  Because [auto reloading](crate::Environment::set_auto_reload)
/// is enabled by default, templates are reloaded automatically when they change
/// on disk.  This costs a `stat` call per template lookup which can be avoided by
/// disabling auto reloading.
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
pub fn path_loader<'x, P: AsRef<Path> + 'x>(
    dir: P,
) -> impl for<'a> Fn(&'a str) -> Result<Option<TemplateSource>, Error> + Send + Sync + 'static {
    let dir = dir.as_ref().to_path_buf();
    move |name| {
        let Some(path) = safe_join(&dir, name) else {
            return Ok(None);
        };
        // stat before reading so that a change racing with the read is
        // detected by the up-to-date check.
        let stamp = file_stamp(&path);
        match fs::read_to_string(&path) {
            Ok(result) => {
                let mut rv = TemplateSource::new(result);
                if let Some(stamp) = stamp {
                    rv = rv.with_uptodate_check(move || file_stamp(&path) == Some(stamp));
                }
                Ok(Some(rv))
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(
                Error::new(ErrorKind::InvalidOperation, "could not read template").with_source(err),
            ),
        }
    }
}

fn file_stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use similar_asserts::assert_eq;

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
}
