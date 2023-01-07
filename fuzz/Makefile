
.PHONY: fuzz-add-template
fuzz-add-template:
	@mkdir -p corpus/add_template
	@RUSTC_BOOTSTRAP=1 cargo fuzz run --jobs 8 add_template corpus/add_template seeds/add_template

.PHONY: fuzz-render
fuzz-render:
	@mkdir -p corpus/render
	@RUSTC_BOOTSTRAP=1 cargo fuzz run --jobs 8 render corpus/render seeds/render

.PHONY: clean-corpus
clean-corpus:
	@rm corpus/*/*
