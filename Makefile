all: test

build:
	@cargo build --all

doc:
	@cargo doc --all

test:
	@echo "CARGO TESTS"
	@rustup component add rustfmt 2> /dev/null
	@cd minijinja; cargo test
	@cd minijinja; cargo test --all-features
	@cd minijinja; cargo check --no-default-features

format:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all

format-check:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all -- --check

lint:
	@rustup component add clippy 2> /dev/null
	@cargo clippy --all -- -F clippy::dbg-macro

.PHONY: all doc test format format-check lint
