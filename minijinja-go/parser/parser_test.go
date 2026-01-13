package parser

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/internal/testutil"
)

const (
	rustTestsDir   = "../../minijinja/tests"
	parserInputDir = "../../minijinja/tests/parser-inputs"
	snapshotDir    = "../../minijinja/tests/snapshots"
	skipListFile   = "../skiplist-parser.txt"
)

func TestParser(t *testing.T) {
	// Load skip list
	skipList, err := testutil.LoadSkipList(skipListFile)
	if err != nil {
		t.Fatalf("failed to load skip list: %v", err)
	}

	// Find all parser input files
	inputs, err := filepath.Glob(filepath.Join(parserInputDir, "*.txt"))
	if err != nil {
		t.Fatalf("failed to glob inputs: %v", err)
	}

	if len(inputs) == 0 {
		t.Fatalf("no input files found in %s", parserInputDir)
	}

	for _, inputPath := range inputs {
		inputName := filepath.Base(inputPath)
		testName := "parser@" + inputName

		t.Run(inputName, func(t *testing.T) {
			// Check skip list
			if skipList[testName] || skipList[inputName] {
				t.Skipf("skipped via skiplist-parser.txt")
			}

			// Read input file (raw template, no JSON)
			content, err := os.ReadFile(inputPath)
			if err != nil {
				t.Fatalf("failed to read input: %v", err)
			}
			template := string(content)

			// Parse
			result := ParseDefault(template, inputName)

			// Format output
			actual := FormatResult(result)

			// Load expected snapshot
			snapshotPath := testutil.FindSnapshotFile(snapshotDir, "test_parser__parser", inputPath)
			snapshot, err := testutil.ParseSnapshotFile(snapshotPath)
			if err != nil {
				if os.IsNotExist(err) {
					t.Fatalf("snapshot not found: %s\nActual output:\n%s", snapshotPath, actual)
				}
				t.Fatalf("failed to parse snapshot: %v", err)
			}

			// Compare
			expected := strings.TrimRight(snapshot.Expected, "\n") + "\n"
			actualNorm := strings.TrimRight(actual, "\n") + "\n"
			if actualNorm != expected {
				t.Errorf("output mismatch\n%s", diffStrings(expected, actualNorm))
			}
		})
	}
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

// TestParserBasic is a simple sanity check.
func TestParserBasic(t *testing.T) {
	result := ParseDefault("Hello {{ name }}!", "test.html")
	if result.Err != nil {
		t.Fatalf("unexpected error: %v", result.Err)
	}

	tmpl := result.Template
	if len(tmpl.Children) != 3 {
		t.Fatalf("expected 3 children, got %d", len(tmpl.Children))
	}

	// First child: EmitRaw "Hello "
	if raw, ok := tmpl.Children[0].(*EmitRaw); !ok || raw.Raw != "Hello " {
		t.Errorf("expected EmitRaw 'Hello ', got %T %v", tmpl.Children[0], tmpl.Children[0])
	}

	// Second child: EmitExpr with Var "name"
	if emit, ok := tmpl.Children[1].(*EmitExpr); !ok {
		t.Errorf("expected EmitExpr, got %T", tmpl.Children[1])
	} else if v, ok := emit.Expr.(*Var); !ok || v.ID != "name" {
		t.Errorf("expected Var 'name', got %T %v", emit.Expr, emit.Expr)
	}

	// Third child: EmitRaw "!"
	if raw, ok := tmpl.Children[2].(*EmitRaw); !ok || raw.Raw != "!" {
		t.Errorf("expected EmitRaw '!', got %T %v", tmpl.Children[2], tmpl.Children[2])
	}
}
