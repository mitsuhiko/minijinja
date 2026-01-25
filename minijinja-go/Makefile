.PHONY: test
test:
	@go test -v ./... 2>&1 | grep -E '(^ok|^FAIL|^\s+--- )'

.PHONY: test-all
test-all:
	@go test -v ./...

.PHONY: summary
summary:
	@echo "=== MiniJinja-Go Test Summary ==="
	@echo ""
	@echo "Lexer tests:"
	@go test -v ./internal/lexer/... 2>&1 | grep -E '^\s+--- ' | sort | uniq -c
	@echo ""
	@echo "Skip list entries:"
	@grep -v '^#' skiplist.txt | grep -v '^$$' | wc -l | xargs echo "  "

.PHONY: lint
lint:
	@go vet ./...
	@test -z "$$(gofmt -l .)" || (echo "Files need formatting:" && gofmt -l . && exit 1)

.PHONY: format
format:
	@gofmt -w .

.PHONY: doc
doc:
	@go doc -all .

.PHONY: doc-open
doc-open:
	@go doc -http
