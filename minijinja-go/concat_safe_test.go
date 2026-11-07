package minijinja

import (
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

func TestSafeConcatEscapesForAutoEscapeMode(t *testing.T) {
	ctx := map[string]any{"s": value.FromSafeString("<s>"), "a": "<a>"}
	source := "{{ s ~ a }}|{{ a + s }}|{{ s ~ s }}|{{ a ~ a }}"
	for _, tc := range []struct {
		name     string
		expected string
	}{
		{"index.html", "<s>&lt;a&gt;|&lt;a&gt;<s>|<s><s>|&lt;a&gt;&lt;a&gt;"},
		// without auto escaping nothing is escaped
		{"index.txt", "<s><a>|<a><s>|<s><s>|<a><a>"},
		// the unsafe side is formatted for the active auto escape mode
		{"index.js", `<s>"<a>"|"<a>"<s>|<s><s>|"<a><a>"`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			env := NewEnvironment()
			tmpl, err := env.TemplateFromNamedString(tc.name, source)
			if err != nil {
				t.Fatal(err)
			}
			rv, err := tmpl.Render(ctx)
			if err != nil {
				t.Fatal(err)
			}
			if rv != tc.expected {
				t.Fatalf("got %q, expected %q", rv, tc.expected)
			}
		})
	}
}

func TestSafeConcatStrictUndefined(t *testing.T) {
	env := NewEnvironment()
	env.SetUndefinedBehavior(UndefinedStrict)
	tmpl, err := env.TemplateFromNamedString("index.html", "{{ s ~ missing }}")
	if err != nil {
		t.Fatal(err)
	}
	_, err = tmpl.Render(map[string]any{"s": value.FromSafeString("<s>")})
	if err == nil {
		t.Fatal("expected undefined error")
	}
}

func TestSafeConcatUsesFormatter(t *testing.T) {
	env := NewEnvironment()
	env.SetAutoEscapeFunc(func(string) AutoEscape { return AutoEscapeCustom("markdown") })
	env.SetFormatter(func(state *State, val value.Value, _ func(string) string) string {
		if !val.IsSafe() && state.AutoEscape().IsCustom() {
			return strings.ReplaceAll(val.String(), "*", `\*`)
		}
		return val.String()
	})
	tmpl, err := env.TemplateFromString("{{ ('**'|safe) ~ a ~ ('**'|safe) }}|{{ ('*'|safe) + a }}|{{ a ~ a }}")
	if err != nil {
		t.Fatal(err)
	}
	rv, err := tmpl.Render(map[string]any{"a": "*x*"})
	if err != nil {
		t.Fatal(err)
	}
	if expected := `**\*x\***|*\*x\*|\*x\*\*x\*`; rv != expected {
		t.Fatalf("got %q, expected %q", rv, expected)
	}
}

func TestValueConcatSafety(t *testing.T) {
	safe := value.FromSafeString("<s>")
	plain := value.FromString("<a>")
	if !safe.Concat(safe).IsSafe() {
		t.Fatal("concatenating two safe strings should be safe")
	}
	if safe.Concat(plain).IsSafe() || plain.Concat(safe).IsSafe() {
		t.Fatal("concatenating with an unsafe string must not be safe")
	}
	if rv, _ := safe.Add(safe); !rv.IsSafe() {
		t.Fatal("adding two safe strings should be safe")
	}
	for _, pair := range [][2]value.Value{{safe, plain}, {plain, safe}} {
		if rv, _ := pair[0].Add(pair[1]); rv.IsSafe() {
			t.Fatal("adding an unsafe string must not be safe")
		}
	}
}
