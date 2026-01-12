// Package testutil provides testing utilities for MiniJinja-Go.
package testutil

import (
	"bufio"
	"os"
	"path/filepath"
	"strings"
)

// Snapshot represents a parsed Insta snapshot file.
type Snapshot struct {
	Source      string            // source file that generated snapshot
	Description string            // template description
	InputFile   string            // original input file path
	Info        map[string]any    // additional metadata
	Expected    string            // expected output
	RawMeta     map[string]string // raw metadata fields
}

// ParseSnapshotFile parses an Insta .snap file.
func ParseSnapshotFile(path string) (*Snapshot, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return ParseSnapshot(string(content))
}

// ParseSnapshot parses the content of an Insta .snap file.
func ParseSnapshot(content string) (*Snapshot, error) {
	snap := &Snapshot{
		RawMeta: make(map[string]string),
	}

	// Split by the YAML frontmatter delimiter
	// Format: ---\n<yaml metadata>\n---\n<expected output>
	parts := strings.SplitN(content, "\n---\n", 2)
	if len(parts) < 2 {
		// Might be just "---" at the start
		content = strings.TrimPrefix(content, "---\n")
		parts = strings.SplitN(content, "\n---\n", 2)
	}

	if len(parts) == 2 {
		// Parse YAML-like metadata (simple key: value format)
		scanner := bufio.NewScanner(strings.NewReader(parts[0]))
		var currentKey string
		var multilineValue strings.Builder

		for scanner.Scan() {
			line := scanner.Text()

			// Check if this is a new key
			if !strings.HasPrefix(line, " ") && !strings.HasPrefix(line, "\t") && strings.Contains(line, ":") {
				// Save previous key if exists
				if currentKey != "" {
					snap.RawMeta[currentKey] = strings.TrimSpace(multilineValue.String())
				}

				idx := strings.Index(line, ":")
				currentKey = line[:idx]
				value := strings.TrimSpace(line[idx+1:])
				multilineValue.Reset()

				// Handle quoted strings
				if strings.HasPrefix(value, "\"") {
					value = parseQuotedString(value)
				}
				multilineValue.WriteString(value)
			} else if currentKey != "" {
				// Continuation of multiline value
				if multilineValue.Len() > 0 {
					multilineValue.WriteString("\n")
				}
				multilineValue.WriteString(line)
			}
		}

		// Save last key
		if currentKey != "" {
			snap.RawMeta[currentKey] = strings.TrimSpace(multilineValue.String())
		}

		// Extract known fields
		snap.Source = snap.RawMeta["source"]
		snap.Description = snap.RawMeta["description"]
		snap.InputFile = snap.RawMeta["input_file"]

		// Expected output is everything after the second ---
		snap.Expected = parts[1]
	} else {
		// No metadata, entire content is expected output
		snap.Expected = content
	}

	return snap, nil
}

// parseQuotedString handles escaped characters in quoted strings.
func parseQuotedString(s string) string {
	if len(s) < 2 {
		return s
	}

	// Remove surrounding quotes
	if strings.HasPrefix(s, "\"") && strings.HasSuffix(s, "\"") {
		s = s[1 : len(s)-1]
	}

	// Handle escape sequences
	s = strings.ReplaceAll(s, "\\n", "\n")
	s = strings.ReplaceAll(s, "\\t", "\t")
	s = strings.ReplaceAll(s, "\\\"", "\"")
	s = strings.ReplaceAll(s, "\\\\", "\\")

	return s
}

// LoadSkipList loads a skip list file (one test name per line, # for comments).
func LoadSkipList(path string) (map[string]bool, error) {
	skipList := make(map[string]bool)

	content, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return skipList, nil
	}
	if err != nil {
		return nil, err
	}

	scanner := bufio.NewScanner(strings.NewReader(string(content)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		skipList[line] = true
	}

	return skipList, nil
}

// FindSnapshotFile finds the snapshot file for a given test.
func FindSnapshotFile(snapshotDir, testPrefix, inputFile string) string {
	// Insta naming convention: test_name__subtest@inputfile.snap
	base := filepath.Base(inputFile)
	snapName := testPrefix + "@" + base + ".snap"
	return filepath.Join(snapshotDir, snapName)
}
