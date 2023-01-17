.PHONY: all
all: develop test

.venv:
	python3 -mvenv .venv
	pip install --upgrade pip
	pip install maturin pytest markupsafe black

.PHONY: test
develop: .venv
	.venv/bin/maturin develop

.PHONY: develop-release
develop-release: .venv
	.venv/bin/maturin develop --release

.PHONY: test
test: .venv
	.venv/bin/pytest

.PHONY: format
format:
	.venv/bin/black tests python
