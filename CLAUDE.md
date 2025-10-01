# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MiniJinja is a powerful Jinja2-compatible template engine for Rust with minimal dependencies. The project is organized as a Cargo workspace with multiple crates:

- **minijinja** - Core template engine library
- **minijinja-cli** - Command-line interface for template rendering
- **minijinja-contrib** - Additional filters and functions
- **minijinja-autoreload** - Auto-reloading functionality for development
- **minijinja-py** - Python bindings (requires Python development libraries)
- **minijinja-js** - JavaScript/WASM bindings
- **examples/** - Various usage examples and demos
- **benchmarks/** - Performance benchmarks

It's quite likely that this project is checked out as `minijinja`.  Do not confuse `minijinja` with `minijinja/minijinja` in that case.

## Essential Commands

### Building and Testing

```bash
# Build entire workspace
make build

# Run comprehensive test suite (slow)
make test

# Run tests for core library only (always test all features!)
cargo test -p minijinja --all-features
```

### Code Quality
```bash
# Format code
make format

# Run linting with clippy
make lint

# Check various feature combinations
make check
```

### CLI Usage

```bash
# Build and run CLI
cd minijinja-cli
cargo run -- template.j2 data.json

# Example usage
cargo run -- examples/hello.j2 examples/hello.json
echo "Hello {{ name }}" | cargo run -- - -Dname=World
```

## Architecture Overview

### Core Library Structure (`minijinja/src/`)
- **compiler/** - Lexer, parser, AST, and bytecode generation
  - `lexer.rs` - Tokenizes template source code
  - `parser.rs` - Builds AST from tokens
  - `codegen.rs` - Generates bytecode from AST
- **vm/** - Virtual machine for executing compiled templates
  - `state.rs` - Execution state and context management
  - `*_object.rs` - Runtime objects (loops, macros, etc.)
- **value/** - Value system and type conversions
  - `mod.rs` - Core Value enum and operations
  - `object.rs` - Dynamic object trait for custom types
  - `serialize.rs`/`deserialize.rs` - Serde integration
- **environment.rs** - Main API entry point for template loading
- **template.rs** - Compiled template representation
- **filters.rs**, **functions.rs**, **tests.rs** - Built-in functionality

### Template Processing Flow
1. **Lexing** - Source code → tokens
2. **Parsing** - Tokens → AST
3. **Compilation** - AST → bytecode instructions
4. **Execution** - VM interprets bytecode with runtime context

### Key Design Patterns
- **Feature-gated compilation** - Different features can be enabled/disabled via Cargo features
- **Zero-copy where possible** - Uses `Cow<str>` and borrowed data structures
- **Serde integration** - All value types work with serde serialization
- **Error handling** - Rich error messages with source location tracking

## Testing Strategy

### Test Organization
- `/minijinja/tests/` - Integration tests with snapshot testing
- `/minijinja/tests/inputs/` - Template files for testing
- `/minijinja/tests/snapshots/` - Expected outputs (insta snapshots)
- Unit tests are embedded in source files

### Feature Testing
The project extensively tests different feature combinations:
```bash
# Test minimal feature set
cd minijinja && cargo test --no-default-features --features=debug

# Test with performance optimizations
cd minijinja && cargo test --no-default-features --features=speedups

# Test specific feature combinations
cd minijinja && cargo test --features=json,urlencode,custom_syntax
```

### Snapshot Testing
Uses the `insta` crate for snapshot testing. When tests fail due to output changes:

```bash
cargo insta test --accept  # accept changes
cargo insta test --reject  # reject changes
```

## Important Development Notes

### Feature Flags
The project uses extensive feature gating. Key features:
- `builtins`, `macros`, `multi_template` - Core functionality
- `json`, `urlencode` - Additional filters
- `loader` - Template loading from filesystem
- `custom_syntax` - Custom delimiters
- `speedups` - Performance optimizations
- `debug` - Debug functionality

### MSRV (Minimum Supported Rust Version)
Currently Rust 1.70+. The CI tests against this version.

### CLI Data Format Support
The CLI supports multiple data formats:
- JSON (.json, .json5)
- YAML (.yaml, .yml)
- TOML (.toml)
- Query strings (.qs)
- INI files (.ini)
- CBOR (.cbor)

### Platform Support
- Standard Rust targets
- WebAssembly (WASI)
- Python bindings (via PyO3)
- JavaScript/Node.js bindings (via wasm-bindgen)

## Common Development Workflows

### Adding New Filters/Functions
1. Add implementation in `filters.rs` or `functions.rs`
2. Add tests in the same file
3. Update documentation
4. Add snapshot tests if needed

### Modifying Parser/Compiler
1. Update lexer tokens if needed (`compiler/tokens.rs`)
2. Modify parser for new syntax (`compiler/parser.rs`)
3. Update AST definitions (`compiler/ast.rs`)
4. Add bytecode generation (`compiler/codegen.rs`)
5. Implement VM execution (`vm/mod.rs`)

### Running Specific Tests
```bash
# Run specific test files
cd minijinja && cargo test --test test_filters
cd minijinja && cargo test --test test_templates

# Run specific test by name
cd minijinja && cargo test test_function_name -- --nocapture
```

### Commit Guidelines
- Follow conventional commit format
- Always run `make lint` and `make format` before committing
- Ensure tests pass with `make test`

### For New Releases
- Make sure the next release is mentioned in CHANGELOG.md
- Use `scripts/bump-version.sh VERSION` to update all references to the next version
- Create a commit for that release and push the tags

## Warnings and Recommendations

- **Workflow Recommendations**:
  - Please do not use `cargo insta review`. it fucks you up because it prompts.
