package minijinja

import (
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

func TestLoopChangedWithPullIterator(t *testing.T) {
	iter := value.MakeOneShotIterator(func(yield func(value.Value) bool) {
		for _, item := range []int64{1, 1, 2} {
			if !yield(value.FromInt(item)) {
				return
			}
		}
	})
	tmpl, err := NewEnvironment().TemplateFromString(
		`{% for item in iter %}{{ loop.changed(item) }} {% endfor %}`,
	)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(map[string]value.Value{"iter": iter})
	if err != nil {
		t.Fatal(err)
	}
	if result != "True False True " {
		t.Fatalf("unexpected result %q", result)
	}
}
