package minijinja

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/internal/testutil"
	"github.com/mitsuhiko/minijinja/minijinja-go/lexer"
)

const (
	rustInputDir     = "../minijinja/tests/inputs"
	rustRefsDir      = "../minijinja/tests/inputs/refs"
	rustSnapshotDir  = "../minijinja/tests/snapshots"
	templateSkipList = "skiplist-templates.txt"
)

func TestTemplates(t *testing.T) {
	// Load skip list
	skipList, err := testutil.LoadSkipList(templateSkipList)
	if err != nil {
		t.Fatalf("failed to load skip list: %v", err)
	}

	// Load reference templates (shared templates used by other tests)
	refTemplates := make(map[string]string)
	refFiles, _ := filepath.Glob(filepath.Join(rustRefsDir, "*"))
	for _, refPath := range refFiles {
		name := filepath.Base(refPath)
		content, err := os.ReadFile(refPath)
		if err != nil {
			t.Fatalf("failed to read ref template %s: %v", name, err)
		}
		refTemplates[name] = string(content)
	}

	// Find all input files
	inputs, err := filepath.Glob(filepath.Join(rustInputDir, "*.txt"))
	if err != nil {
		t.Fatalf("failed to glob inputs: %v", err)
	}
	htmlInputs, _ := filepath.Glob(filepath.Join(rustInputDir, "*.html"))
	inputs = append(inputs, htmlInputs...)

	if len(inputs) == 0 {
		t.Fatalf("no input files found in %s", rustInputDir)
	}

	for _, inputPath := range inputs {
		inputName := filepath.Base(inputPath)
		testName := "vm@" + inputName

		t.Run(inputName, func(t *testing.T) {
			// Check skip list
			if skipList[testName] || skipList[inputName] {
				t.Skipf("skipped via skiplist-templates.txt")
			}

			// Parse input file
			input, err := testutil.ParseTestInputFile(inputPath)
			if err != nil {
				t.Fatalf("failed to parse input: %v", err)
			}

			// Create environment
			env := NewEnvironment()

			// Configure environment from settings
			if input.Settings != nil {
				if input.Settings.HasMarkers() {
					env.SetSyntax(lexer.SyntaxConfig{
						BlockStart:          input.Settings.Markers[0],
						BlockEnd:            input.Settings.Markers[1],
						VarStart:            input.Settings.Markers[2],
						VarEnd:              input.Settings.Markers[3],
						CommentStart:        input.Settings.Markers[4],
						CommentEnd:          input.Settings.Markers[5],
						LineStatementPrefix: input.Settings.LineStatementPrefix,
						LineCommentPrefix:   input.Settings.LineCommentPrefix,
					})
				} else if input.Settings.LineStatementPrefix != "" || input.Settings.LineCommentPrefix != "" {
					syntax := lexer.DefaultSyntax()
					syntax.LineStatementPrefix = input.Settings.LineStatementPrefix
					syntax.LineCommentPrefix = input.Settings.LineCommentPrefix
					env.SetSyntax(syntax)
				}

				env.SetWhitespace(lexer.WhitespaceConfig{
					KeepTrailingNewline: input.Settings.KeepTrailingNewline,
					LstripBlocks:        input.Settings.LstripBlocks,
					TrimBlocks:          input.Settings.TrimBlocks,
				})
			}

			// Add reference templates
			for name, source := range refTemplates {
				if err := env.AddTemplate(name, source); err != nil {
					t.Fatalf("failed to add ref template %s: %v", name, err)
				}
			}

			// Add get_args function (used by some tests)
			env.AddFunction("get_args", func(state *State, args []Value, kwargs map[string]Value) (Value, error) {
				return FromSlice(args), nil
			})

			// Try to add and render the template
			var rendered string
			if err := env.AddTemplate(inputName, input.Template); err != nil {
				// Syntax error
				rendered = formatSyntaxError(err)
			} else {
				tmpl, err := env.GetTemplate(inputName)
				if err != nil {
					rendered = formatError(err)
				} else {
					result, err := tmpl.Render(input.Context)
					if err != nil {
						rendered = formatError(err)
					} else {
						rendered = result + "\n"
					}
				}
			}

			// Load expected snapshot
			snapshotPath := filepath.Join(rustSnapshotDir, "test_templates__vm@"+inputName+".snap")
			snapshot, err := testutil.ParseSnapshotFile(snapshotPath)
			if err != nil {
				if os.IsNotExist(err) {
					t.Fatalf("snapshot not found: %s\nActual output:\n%s", snapshotPath, rendered)
				}
				t.Fatalf("failed to parse snapshot: %v", err)
			}

			// Compare
			expected := snapshot.Expected
			if !compareOutput(expected, rendered) {
				t.Errorf("output mismatch\n%s", diffStrings(expected, rendered))
			}
		})
	}
}

// formatSyntaxError formats a syntax error like Rust does.
func formatSyntaxError(err error) string {
	var sb strings.Builder
	sb.WriteString("!!!SYNTAX ERROR!!!\n\n")
	// Format debug representation
	sb.WriteString(fmt.Sprintf("%#v", err))
	sb.WriteString("\n\n")
	// Format display representation
	sb.WriteString(err.Error())
	sb.WriteString("\n")
	return sb.String()
}

// formatError formats a runtime error like Rust does.
func formatError(err error) string {
	var sb strings.Builder
	sb.WriteString("!!!ERROR!!!\n\n")
	// Format debug representation
	sb.WriteString(fmt.Sprintf("%#v", err))
	sb.WriteString("\n\n")
	// Format display representation
	sb.WriteString(err.Error())
	sb.WriteString("\n")
	return sb.String()
}

// compareOutput compares expected and actual output with normalization.
func compareOutput(expected, actual string) bool {
	// Normalize trailing whitespace
	expected = strings.TrimRight(expected, "\n") + "\n"
	actual = strings.TrimRight(actual, "\n") + "\n"
	return expected == actual
}

// diffStrings returns a diff for debugging.
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
