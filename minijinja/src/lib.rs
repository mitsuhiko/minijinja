//! <div align=center>
//!   <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
//!   <p><strong>MiniJinja: a powerful template engine for Rust with minimal dependencies</strong></p>
//! </div>
//!
//! MiniJinja is a simple [Jinja2](https://jinja.palletsprojects.com/) inspired
//! template engine based on [serde](https://serde.rs/). It's light in features
//! and on dependencies but implements a pretty sizable feature set from Jinja2.
//! It attempts to stay largely compatible in Syntax and behavior:
//!
//! ```jinja
//! {% for user in users %}
//!   <li>{{ user.name }}</li>
//! {% endfor %}
//! ```
//!
//! # Why MiniJinja
//!
//! Rust already has quite a selection of template engines and there are in fact
//! already a handful of engines which are inspired by Jinja2 including
//! [Tera](https://crates.io/crates/tera) and
//! [Askama](https://crates.io/crates/askama) but they are very heavy in terms of
//! dependencies and usage. MiniJinja by its name does not try to become a
//! replacement for these, but it wants to be a good default choice if you need a
//! little bit of templating with minimal dependencies.
//!
//! MiniJinja tries to juggle these three goals:
//!
//! 1. aim for a high level of compatibility with Jinja2 templates
//! 2. provide template rendering and expression evaluation functionality
//! 3. achieve above functionality with the lest amount of dependencies possible
//!
//! # Template Usage
//!
//! To use MiniJinja one needs to create an [`Environment`] and populate it with templates.
//! Afterwards templates can be loaded and rendered.  To pass data one can pass any serde
//! serializable value:
//!
//! ```
//! use minijinja::{Environment, context};
//!
//! let mut env = Environment::new();
//! env.add_template("hello", "Hello {{ name }}!").unwrap();
//! let tmpl = env.get_template("hello").unwrap();
//! println!("{}", tmpl.render(context!(name => "John")).unwrap());
//! ```
//!
//! ```plain
//! Hello John!
//! ```
//!
//! # Expression Usage
//!
//! MiniJinja — like Jinja2 — allows to be used as expression language.  This can be
//! useful to express logic in configuration files or similar things.  For this
//! purpose the [`Environment::compile_expression`] method can be used.  It returns
//! an expression object that can then be evaluated, returning the result:
//!
//! ```
//! use minijinja::{Environment, context};
//!
//! let env = Environment::new();
//! let expr = env.compile_expression("number < 42").unwrap();
//! let result = expr.eval(context!(number => 23)).unwrap();
//! assert_eq!(result.is_true(), true);
//! ```
//!
//! # Learn more
//!
//! - [`syntax`]: documentation of the template engine syntax.
//! - [`filters`]: for how to write custom filters and to see the list of built-in filters.
//! - [`tests`]: for how to write custom test functions and to see the list of built-in tests.
//! - [`functions`]: for how to write custom functions and to see the list of built-in functions.
//! - [`value`]: for information about the runtime value object.
//! - [`Environment`]: the main API entry point.
//! - [`Template`]: the template object API.
//!
//! # Optional Features
//!
//! There are some additional features that can be enabled:
//!
//! - `source`: enables the `Source` type which helps with dynamic loading of templates.
//! - `memchr`: enables the `memchr` dependency which provides performance improvements
//!   for the parser.
//! - `v_htmlescape`: enables the `v_htmlescape` dependency which implements a faster HTML
//!   escaping algorithm.
//! - `unstable_machinery`: provides access to the internal machinery of the engine.  This
//!   is a forever unstable API which mainly exists to aid debugging complex issues.
//! - `json`: When enabled the `tojson` filter is added as builtin filter.
//! - `urlencode`: When enabled the `urlencode` filter is added as builtin filter.
//! - `preserve_order`: When enable the internal value implementation uses an indexmap
//!   which preserves the original order of maps and structs.
//!
//! Additionally to cut down on size of the engine some default
//! functionality can be removed:
//!
//! - `builtin_filters`: if this feature is removed the default filters are
//!   not implemented.
//! - `builtin_tests`: if this feature is removed the default tests are
//!   not implemented.
//! - `builtin_functions`: if this feature is removed the default functions are
//!   not implemented.
//! - `sync`: this feature makes MiniJinja's type `Send` and `Sync`.  If this feature
//!   is disabled sending types across threads is often not possible.  Thread bounds
//!   of things like callbacks however are not changing which means code that uses
//!   MiniJinja still needs to be threadsafe.
//! - `debug`: if this feature is removed some debug functionality of the engine is
//!   removed as well.  This mainly affects the quality of error reporting.
#![allow(clippy::cognitive_complexity)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_logo_url = "https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo-square.png")]
mod key;

mod ast;
mod compiler;
mod context;
mod environment;
mod error;
mod instructions;
mod lexer;
mod parser;
mod tokens;
mod utils;
mod vm;

pub mod filters;
pub mod functions;
pub mod syntax;
pub mod tests;
pub mod value;

#[cfg(feature = "source")]
mod source;

pub use self::environment::{Environment, Expression, Template};
pub use self::error::{Error, ErrorKind};
pub use self::utils::{AutoEscape, HtmlEscape};

#[cfg(feature = "debug")]
pub use self::error::DebugInfo;

#[cfg(feature = "source")]
pub use self::source::Source;

pub use self::context::*;
pub use self::vm::State;

/// This module gives access to the low level machinery.
///
/// This module is only provided by the `unstable_machinery` feature and does not
/// have a stable interface.  It mostly exists for internal testing purposes and
/// for debugging.
#[cfg(feature = "unstable_machinery")]
#[cfg_attr(docsrs, doc(cfg(feature = "unstable_machinery")))]
pub mod machinery {
    /// The AST nodes.
    pub mod ast {
        pub use crate::ast::*;
    }
    pub use crate::compiler::Compiler;
    pub use crate::instructions::{Instruction, Instructions};
    pub use crate::lexer::tokenize;
    pub use crate::parser::parse;
    pub use crate::tokens::{Span, Token};
    pub use crate::vm::{simple_eval, Vm};
}
