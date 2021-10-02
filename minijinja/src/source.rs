use std::collections::BTreeMap;
use std::fmt;

use self_cell::self_cell;

use crate::environment::CompiledTemplate;
use crate::error::Error;
use crate::utils::RcType;

/// Utility for dynamic template loading.
///
/// Because an [`Environment`](crate::Environment) holds a reference
/// to the source it borrows templates from it becomes very inconvenient
/// when it should be passed around.  This object provides a solution for
/// such cases.  First templates are loaded into the source, then it can
/// be converted into an environment.
///
/// In the process the lifetime to the source is eliminated.
#[derive(Clone, Default)]
pub struct Source {
    templates: BTreeMap<String, RcType<LoadedTemplate>>,
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.templates, f)
    }
}

self_cell! {
    struct LoadedTemplate {
        owner: String,
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

    /// Adds a new template into the source.
    pub fn add_template<N: Into<String>, S: Into<String>>(
        &mut self,
        name: N,
        source: S,
    ) -> Result<(), Error> {
        let source = source.into();
        let name = name.into();
        let tmpl = LoadedTemplate::try_new(source, |source| -> Result<_, Error> {
            CompiledTemplate::from_name_and_source(name.as_str(), source)
        })?;
        self.templates.insert(name, RcType::new(tmpl));
        Ok(())
    }

    /// Removes a template from the source.
    pub fn remove_template(&mut self, name: &str) {
        self.templates.remove(name);
    }

    /// Gets a compiled template from the source.
    pub(crate) fn get_compiled_template(
        &self,
        name: &str,
    ) -> Option<(&str, &CompiledTemplate<'_>)> {
        self.templates
            .get_key_value(name)
            .map(|(key, value)| (key.as_str(), value.borrow_dependent()))
    }
}
