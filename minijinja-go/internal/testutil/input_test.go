package testutil

import "testing"

func TestParseTestInputRejectsOutOfRangeJSONNumber(t *testing.T) {
	_, err := ParseTestInput("{\"n\": 1e400}\n---\n{{ n }}")
	if err == nil {
		t.Fatal("expected an out-of-range JSON number to be rejected")
	}
}
