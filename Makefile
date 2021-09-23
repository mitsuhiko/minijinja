all: test

build:
	@cargo build

doc:
	@cargo doc

test:
	@echo "CARGO TESTS"
	@rustup component add rustfmt 2> /dev/null
	@cargo test
	@cargo test --all-features

format:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all

format-check:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all -- --check

lint:
	@rustup component add clippy 2> /dev/null
	@cargo clippy

.PHONY: all doc test format format-check lint
