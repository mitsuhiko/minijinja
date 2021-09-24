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
//! use std::collections::BTreeMap;
//! use minijinja::Environment;
//!
//! let mut env = Environment::new();
//! env.add_template("hello", "Hello {{ name }}!").unwrap();
//! let mut ctx = BTreeMap::new();
//! ctx.insert("name", "John");
//! println!("{}", env.get_template("hello").unwrap().render(&ctx).unwrap());
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
//! use std::collections::BTreeMap;
//! use minijinja::Environment;
//!
//! let env = Environment::new();
//! let expr = env.compile_expression("23 < 42").unwrap();
//! let result = expr.eval(&()).unwrap();
//! assert_eq!(result.is_true(), true);
//! ```
//!
//! # Learn more
//!
//! - [`syntax`]: documentation of the template engine syntax.
//! - [`filters`]: for how to write custom filters and list of built-in filters.
//! - [`tests`]: for how to write custom test functions and list of built-in tests.
//! - [`value`]: for information about the runtime value object.
//! - [`Environment`]: the main API entry point.
//! - [`Template`]: the template object API.
//!
//! # Optional Features
//!
//! There are some additional features that can be enabled:
//!
//! - `memchr`: enables the `memchr` dependency which provides performance improvements
//!   for the parser.
//! - `unstable_machinery`: provides access to the internal machinery of the engine.  This
//!   is a forever unstable API which mainly exists to aid debugging complex issues.
mod key;

mod ast;
mod compiler;
mod environment;
mod error;
mod instructions;
mod lexer;
mod parser;
mod tokens;
mod utils;
mod vm;

pub mod filters;
pub mod syntax;
pub mod tests;
pub mod value;

pub use self::environment::{Environment, Expression, Template};
pub use self::error::{Error, ErrorKind};
pub use self::utils::AutoEscape;

/// This module gives access to the low level machinery.
///
/// This module is only provided by the `unstable_machinery` feature and does not
/// have a stable interface.  It mostly exists for internal testing purposes and
/// for debugging.
#[cfg(feature = "unstable_machinery")]
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
