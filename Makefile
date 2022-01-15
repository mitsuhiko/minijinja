DOC_FEATURES=source,json,urlencode
TEST_FEATURES=unstable_machinery,builtins,source,json,urlencode,debug,internal_debug

all: test

build:
	@cargo build --all

doc:
	@RUSTDOCFLAGS="--cfg=docsrs --html-in-header doc-header.html" cargo +nightly doc --no-deps --all --features=$(DOC_FEATURES)

test:
	@$(MAKE) run-tests FEATURES=$(TEST_FEATURES)
	@$(MAKE) run-tests FEATURES=$(TEST_FEATURES),preserve_order,key_interning
	@echo "CARGO TEST ALL FEATURES"
	@cd minijinja; cargo test --all-features

test-142:
	@$(MAKE) run-tests FEATURES=$(TEST_FEATURES)
	@$(MAKE) run-tests FEATURES=$(TEST_FEATURES),key_interning
	@echo "CARGO TEST ALL FEATURES"
	@cd minijinja; cargo test --all-features

run-tests:
	@rustup component add rustfmt 2> /dev/null
	@echo "CARGO TESTS"
	@cd minijinja; cargo test --features=json,urlencode,internal_debug
	@echo "CARGO TEST SPEEDUPS"
	@cd minijinja; cargo test --no-default-features --features=speedups,$(FEATURES)
	@echo "CARGO CHECK NO_DEFAULT_FEATURES"
	@cd minijinja; cargo check --no-default-features

check:
	@echo "check no default features:"
	@cd minijinja; cargo check --no-default-features
	@echo "check all features:"
	@cd minijinja; cargo check --all-features

format:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all

format-check:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all -- --check

lint:
	@rustup component add clippy 2> /dev/null
	@cargo clippy --all -- -F clippy::dbg-macro

.PHONY: all doc test run-tests format format-check lint check
