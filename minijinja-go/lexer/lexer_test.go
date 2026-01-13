package lexer

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/testutil"
)

const (
	// Paths relative to lexer package directory (minijinja-go/lexer/)
	rustTestsDir  = "../../minijinja/tests"
	lexerInputDir = "../../minijinja/tests/lexer-inputs"
	snapshotDir   = "../../minijinja/tests/snapshots"
	skipListFile  = "../skiplist.txt"
)

func TestLexer(t *testing.T) {
	// Load skip list
	skipList, err := testutil.LoadSkipList(skipListFile)
	if err != nil {
		t.Fatalf("failed to load skip list: %v", err)
	}

	// Find all lexer input files
	inputs, err := filepath.Glob(filepath.Join(lexerInputDir, "*.txt"))
	if err != nil {
		t.Fatalf("failed to glob inputs: %v", err)
	}

	if len(inputs) == 0 {
		t.Fatalf("no input files found in %s", lexerInputDir)
	}

	for _, inputPath := range inputs {
		inputName := filepath.Base(inputPath)
		testName := "lexer@" + inputName

		t.Run(inputName, func(t *testing.T) {
			// Check skip list
			if skipList[testName] || skipList[inputName] {
				t.Skipf("skipped via skiplist.txt")
			}

			// Parse input file
			input, err := testutil.ParseTestInputFile(inputPath)
			if err != nil {
				t.Fatalf("failed to parse input: %v", err)
			}

			// Build config from settings
			syntaxCfg := DefaultSyntax()
			whitespaceCfg := DefaultWhitespace()

			if input.Settings != nil {
				if input.Settings.HasMarkers() {
					syntaxCfg.BlockStart = input.Settings.Markers[0]
					syntaxCfg.BlockEnd = input.Settings.Markers[1]
					syntaxCfg.VarStart = input.Settings.Markers[2]
					syntaxCfg.VarEnd = input.Settings.Markers[3]
					syntaxCfg.CommentStart = input.Settings.Markers[4]
					syntaxCfg.CommentEnd = input.Settings.Markers[5]
				}
				if input.Settings.LineStatementPrefix != "" {
					syntaxCfg.LineStatementPrefix = input.Settings.LineStatementPrefix
				}
				if input.Settings.LineCommentPrefix != "" {
					syntaxCfg.LineCommentPrefix = input.Settings.LineCommentPrefix
				}
				whitespaceCfg.KeepTrailingNewline = input.Settings.KeepTrailingNewline
				whitespaceCfg.LstripBlocks = input.Settings.LstripBlocks
				whitespaceCfg.TrimBlocks = input.Settings.TrimBlocks
			}

			// Tokenize
			tokens, err := Tokenize(input.Template, syntaxCfg, whitespaceCfg)
			if err != nil {
				t.Fatalf("lexer error: %v", err)
			}

			// Format output like Rust's stringify_tokens
			actual := stringifyTokens(tokens, input.Template)

			// Load expected snapshot
			snapshotPath := testutil.FindSnapshotFile(snapshotDir, "test_lexer__lexer", inputPath)
			snapshot, err := testutil.ParseSnapshotFile(snapshotPath)
			if err != nil {
				if os.IsNotExist(err) {
					t.Fatalf("snapshot not found: %s\nActual output:\n%s", snapshotPath, actual)
				}
				t.Fatalf("failed to parse snapshot: %v", err)
			}

			// Compare - normalize trailing newlines
			// Insta snapshots may have trailing blank line, our output may not
			expected := strings.TrimRight(snapshot.Expected, "\n") + "\n"
			actualNorm := strings.TrimRight(actual, "\n") + "\n"
			if actualNorm != expected {
				t.Errorf("output mismatch\n%s", diffStrings(expected, actualNorm))
			}
		})
	}
}

// stringifyTokens formats tokens the way Rust's test does.
func stringifyTokens(tokens []Token, source string) string {
	var sb strings.Builder
	for _, tok := range tokens {
		sb.WriteString(tok.FormatForSnapshot(source))
		sb.WriteString("\n")
	}
	return sb.String()
}

// diffStrings returns a simple diff for debugging.
func diffStrings(expected, actual string) string {
	var sb strings.Builder
	sb.WriteString("=== EXPECTED ===\n")
	sb.WriteString(expected)
	sb.WriteString("=== ACTUAL ===\n")
	sb.WriteString(actual)
	sb.WriteString("=== END ===\n")

	// Show first difference
	expectedLines := strings.Split(expected, "\n")
	actualLines := strings.Split(actual, "\n")

	for i := 0; i < len(expectedLines) || i < len(actualLines); i++ {
		var expLine, actLine string
		if i < len(expectedLines) {
			expLine = expectedLines[i]
		}
		if i < len(actualLines) {
			actLine = actualLines[i]
		}
		if expLine != actLine {
			sb.WriteString(fmt.Sprintf("\nFirst diff at line %d:\n", i+1))
			sb.WriteString(fmt.Sprintf("  expected: %q\n", expLine))
			sb.WriteString(fmt.Sprintf("  actual:   %q\n", actLine))
			break
		}
	}

	return sb.String()
}

// TestLexerBasic is a simple sanity check.
func TestLexerBasic(t *testing.T) {
	input := "Hello {{ name }}!"
	tokens, err := Tokenize(input, DefaultSyntax(), DefaultWhitespace())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := []struct {
		typ   TokenType
		value string
	}{
		{TokenTemplateData, "Hello "},
		{TokenVariableStart, "{{"},
		{TokenIdent, "name"},
		{TokenVariableEnd, "}}"},
		{TokenTemplateData, "!"},
	}

	if len(tokens) != len(expected) {
		t.Fatalf("expected %d tokens, got %d", len(expected), len(tokens))
	}

	for i, exp := range expected {
		if tokens[i].Type != exp.typ || tokens[i].Value != exp.value {
			t.Errorf("token %d: expected %s(%q), got %s(%q)",
				i, exp.typ, exp.value, tokens[i].Type, tokens[i].Value)
		}
	}
}
