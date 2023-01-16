.PHONY: all
all: develop test

.venv:
	python3 -mvenv .venv
	pip install --upgrade pip
	pip install maturin pytest

.PHONY: test
develop: .venv
	.venv/bin/maturin develop

.PHONY: test
test: .venv
	.venv/bin/pytest
