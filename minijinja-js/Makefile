.PHONY: all

all: install build test

.PHONY: install
install:
	npm install

.PHONY: build
build:
	npm run build

.PHONY: test
test: build
	npm test
