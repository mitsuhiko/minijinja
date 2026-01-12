package testutil

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
)

// TestInput represents a parsed test input file.
type TestInput struct {
	Context  map[string]any // JSON context variables
	Settings *TestSettings  // Optional $settings from context
	Template string         // Template source after ---
}

// TestSettings represents the $settings field in test inputs.
type TestSettings struct {
	KeepTrailingNewline bool      `json:"keep_trailing_newline"`
	LstripBlocks        bool      `json:"lstrip_blocks"`
	TrimBlocks          bool      `json:"trim_blocks"`
	Markers             [6]string `json:"markers"`
	LineStatementPrefix string    `json:"line_statement_prefix"`
	LineCommentPrefix   string    `json:"line_comment_prefix"`
	Undefined           string    `json:"undefined"`
}

// HasMarkers returns true if custom markers are configured.
func (s *TestSettings) HasMarkers() bool {
	if s == nil {
		return false
	}
	for _, m := range s.Markers {
		if m != "" {
			return true
		}
	}
	return false
}

// ParseTestInputFile reads and parses a test input file.
func ParseTestInputFile(path string) (*TestInput, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return ParseTestInput(string(content))
}

// ParseTestInput parses test input content.
// Format: JSON context\n---\ntemplate
func ParseTestInput(content string) (*TestInput, error) {
	input := &TestInput{
		Context: make(map[string]any),
	}

	parts := strings.SplitN(content, "\n---\n", 2)

	// Parse JSON context
	if len(parts) >= 1 && strings.TrimSpace(parts[0]) != "" {
		if err := json.Unmarshal([]byte(parts[0]), &input.Context); err != nil {
			return nil, err
		}

		// Extract $settings if present
		if settingsRaw, ok := input.Context["$settings"]; ok {
			settingsJSON, err := json.Marshal(settingsRaw)
			if err != nil {
				return nil, err
			}
			input.Settings = &TestSettings{}
			if err := json.Unmarshal(settingsJSON, input.Settings); err != nil {
				return nil, err
			}
			// Remove $settings from context (it's not a template variable)
			delete(input.Context, "$settings")
		}
	}

	// Template is the second part
	if len(parts) >= 2 {
		input.Template = parts[1]
	}

	return input, nil
}

// GlobTestInputs finds all test input files matching a pattern.
func GlobTestInputs(pattern string) ([]string, error) {
	return filepath.Glob(pattern)
}

// TestResult represents the result of running a single test.
type TestResult struct {
	Name     string
	Passed   bool
	Skipped  bool
	Error    error
	Expected string
	Actual   string
}

// Diff returns a simple diff between expected and actual output.
func (r *TestResult) Diff() string {
	if r.Expected == r.Actual {
		return ""
	}

	var sb strings.Builder
	sb.WriteString("=== Expected ===\n")
	sb.WriteString(r.Expected)
	if !strings.HasSuffix(r.Expected, "\n") {
		sb.WriteString("⏎\n") // Show missing newline
	}
	sb.WriteString("=== Actual ===\n")
	sb.WriteString(r.Actual)
	if !strings.HasSuffix(r.Actual, "\n") {
		sb.WriteString("⏎\n")
	}
	sb.WriteString("=== End ===\n")
	return sb.String()
}
