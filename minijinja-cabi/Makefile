.PHONY: hello test clean

hello:
	$(MAKE) -C example hello

test:
	cargo build -p minijinja-cabi
	$(MAKE) -C tests test

clean:
	$(MAKE) -C tests clean
