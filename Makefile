DOC_FEATURES=source,json,urlencode
TEST_FEATURES=unstable_machinery,builtins,source,json,urlencode,debug,internal_debug,macros,multi-template

all: test

build:
	@cargo build --all

doc:
	@cd minijinja; RUSTC_BOOTSTRAP=1 RUSTDOCFLAGS="--cfg=docsrs --html-in-header doc-header.html" cargo doc -p minijinja -p minijinja-autoreload -p minijinja-stack-ref --no-deps --features=$(DOC_FEATURES)

test:
	@$(MAKE) run-tests FEATURES=$(TEST_FEATURES)
	@$(MAKE) run-tests FEATURES=$(TEST_FEATURES),preserve_order,key_interning,unicode
	@echo "CARGO TEST ALL FEATURES"
	@cd minijinja; cargo test --all-features

wasi-test:
	@cd minijinja; cargo test --all-features --target=wasm32-wasi -- --nocapture

snapshot-tests:
	@cd minijinja; cargo insta test --all-features --review

run-tests:
	@rustup component add rustfmt 2> /dev/null
	@echo "CARGO TESTS"
	@cd minijinja; cargo test --features=json,urlencode,internal_debug
	@echo "CARGO TEST SPEEDUPS"
	@cd minijinja; cargo test --no-default-features --features=speedups,$(FEATURES)
	@echo "CARGO CHECK NO_DEFAULT_FEATURES"
	@cd minijinja; cargo check --no-default-features --features=debug

check:
	@echo "check no default features:"
	@cd minijinja; cargo check --no-default-features
	@echo "check all features:"
	@cd minijinja; cargo check --all-features
	@echo "check macro only:"
	@cd minijinja; cargo check --no-default-features --features macros
	@echo "check multi-template only:"
	@cd minijinja; cargo check --no-default-features --features multi-template
	@echo "check minijinja-autoreload:"
	@cd minijinja-autoreload; cargo check
	@cd minijinja-autoreload; cargo check --no-default-features

format:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all

format-check:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all -- --check

lint:
	@rustup component add clippy 2> /dev/null
	@cargo clippy --all -- -F clippy::dbg-macro -D warnings

.PHONY: all doc test wasi-test run-tests format format-check lint check snapshot-tests
