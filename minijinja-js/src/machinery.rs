//! Unstable access to the engine internals.
//!
//! This is only compiled with the `unstable_machinery` feature which is used
//! by the playground.  The output format follows the serde representation of
//! the internal types and can change at any time.
use std::collections::BTreeMap;
use std::sync::Arc;

use minijinja::machinery::{self, CompiledTemplate, Instructions, TemplateConfig};
use minijinja::value::{Serde, Value};
use minijinja::AutoEscape;
use wasm_bindgen::prelude::*;

use crate::error::to_js_error;
use crate::value::value_to_js;
use crate::Environment;

fn to_js(value: Value) -> Result<JsValue, JsValue> {
    value_to_js(&value).map_err(to_js_error)
}

fn collect_instructions<'a, 'source>(
    instructions: &'a Instructions<'source>,
) -> Vec<&'a machinery::Instruction<'source>> {
    (0..).map_while(|idx| instructions.get(idx)).collect()
}

#[wasm_bindgen]
impl Environment {
    /// Tokenizes a template source into `[token, span]` pairs.
    ///
    /// This is an unstable API and only available in builds with the
    /// `unstable_machinery` feature.
    pub fn unstableTokenize(&self, source: &str) -> Result<JsValue, JsValue> {
        let syntax = self.env()?.syntax().clone();
        let tokens = machinery::tokenize(source, false, syntax)
            .collect::<Result<Vec<_>, _>>()
            .map_err(to_js_error)?;
        to_js(Value::from(Serde(tokens)))
    }

    /// Parses a template source into its AST.
    ///
    /// This is an unstable API and only available in builds with the
    /// `unstable_machinery` feature.
    pub fn unstableParse(&self, source: &str, name: Option<String>) -> Result<JsValue, JsValue> {
        let syntax = self.env()?.syntax().clone();
        let name = name.as_deref().unwrap_or("<string>");
        let ast = machinery::parse(source, name, syntax).map_err(to_js_error)?;
        to_js(Value::from(Serde(ast)))
    }

    /// Compiles a template source and returns the instructions of the root
    /// and all blocks (keyed by block name, the root is `"<root>"`).
    ///
    /// This is an unstable API and only available in builds with the
    /// `unstable_machinery` feature.
    pub fn unstableCompile(&self, source: &str, name: Option<String>) -> Result<JsValue, JsValue> {
        let syntax = self.env()?.syntax().clone();
        let name = name.as_deref().unwrap_or("<string>");
        let compiled = CompiledTemplate::new(
            name,
            source,
            &TemplateConfig {
                syntax_config: syntax,
                default_auto_escape: Arc::new(|_| AutoEscape::None),
            },
        )
        .map_err(to_js_error)?;
        // "<root>" sorts before all block names
        let mut rv = BTreeMap::new();
        rv.insert("<root>", collect_instructions(&compiled.instructions));
        for (block_name, instructions) in compiled.blocks.iter() {
            rv.insert(block_name, collect_instructions(instructions));
        }
        to_js(Value::from(Serde(rv)))
    }
}
