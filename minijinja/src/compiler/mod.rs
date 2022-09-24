#![allow(missing_docs)]
/// This module contains the internals of the compiler.
pub mod ast;
pub mod codegen;
pub mod instructions;
pub mod lexer;
#[cfg(feature = "macros")]
pub mod meta;
pub mod parser;
pub mod tokens;
