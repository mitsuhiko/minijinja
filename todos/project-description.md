# Project: MiniJinja
A powerful Jinja2-compatible template engine for Rust with minimal dependencies.

## Features
- Jinja2-compatible syntax and semantics
- Template inheritance, macros, and imports
- Built-in and custom filters, functions, and tests
- Serde integration for any serializable type
- Multiple language bindings (Python, JavaScript, C)
- CLI tool for template rendering
- Auto-escaping for secure HTML generation
- Bytecode compilation for performance

## Tech Stack
- Rust (core implementation)
- Python/PyO3 (Python bindings)
- JavaScript/wasm-pack (WASM bindings)
- Cargo (build and dependency management)
- Make (build orchestration)
- insta (snapshot testing)

## Structure
- minijinja/ - Core template engine library
- minijinja-cli/ - Command-line interface
- minijinja-contrib/ - Additional filters and functions
- minijinja-autoreload/ - Auto-reloading functionality
- minijinja-py/ - Python bindings
- minijinja-js/ - JavaScript/WASM bindings
- examples/ - Usage examples
- benchmarks/ - Performance benchmarks

## Architecture
Template processing flow: Lexing → Parsing → Compilation → VM Execution
- compiler/ - Lexer, parser, AST, bytecode generation
- vm/ - Virtual machine for executing bytecode
- value/ - Value system with serde integration
- Environment manages templates, filters, and configuration

## Commands
- Build: make build
- Test: make test
- Lint: make lint
- Dev/Run: cargo run -p minijinja-cli -- template.j2 data.json

## Testing
Create tests in minijinja/tests/inputs/ with format:
```
{JSON context}
---
Template content
```
Run with `cargo test -p minijinja --all-features`