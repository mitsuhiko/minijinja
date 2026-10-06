package errors

import (
	"fmt"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/syntax"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

func TestLimitedRepr(t *testing.T) {
	seq := make([]value.Value, 25)
	for i := range seq {
		seq[i] = value.FromInt(int64(i))
	}
	env := make(map[string]value.Value)
	for _, key := range []string{"a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"} {
		env[key] = value.FromString(key)
	}
	nested := value.FromInt(1)
	for _, key := range []string{"e", "d", "c", "b", "a"} {
		nested = value.FromMap(map[string]value.Value{key: nested})
	}

	tests := []struct {
		name string
		val  value.Value
		want string
	}{
		{"short seq", value.FromSlice([]value.Value{value.FromInt(1), value.FromString("x")}), "[1, 'x']"},
		{"long seq", value.FromSlice(seq), "[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, ... 15 more]"},
		{"tuple", value.FromTuple([]value.Value{value.FromInt(1)}), "(1,)"},
		{"long map", value.FromMap(env), "{'a': 'a', 'b': 'b', 'c': 'c', 'd': 'd', 'e': 'e', 'f': 'f', 'g': 'g', 'h': 'h', 'i': 'i', 'j': 'j', ...: 2 more}"},
		{"deep", nested, "{'a': {'b': {'c': {'d': {...}}}}}"},
		{"long string", value.FromString(strings.Repeat("é", 150)), "'" + strings.Repeat("é", 100) + "'..."},
		{"short string", value.FromString("hello"), "'hello'"},
		{"number", value.FromInt(42), "42"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := limitedRepr(tt.val, 0); got != tt.want {
				t.Errorf("limitedRepr() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestRenderDebugInfo(t *testing.T) {
	render := func(span *syntax.Span, source string) string {
		err := NewError(ErrTemplateNotFound, "missing")
		err.Name = "templates/test.html"
		err.Span = span
		err.DebugInfo = &DebugInfo{TemplateSource: source}
		return fmt.Sprintf("%+v", err)
	}
	rule := strings.Repeat("~", 79) + "\n"

	t.Run("tabs", func(t *testing.T) {
		got := render(&syntax.Span{StartLine: 2, StartCol: 6, EndLine: 2, EndCol: 9}, "<ul>\n\t\t<li>{{ x }}</li>\n</ul>")
		want := "   1 | <ul>\n" +
			"   2 > \t\t<li>{{ x }}</li>\n" +
			"     i \t\t    ^^^ template not found\n" +
			"   3 | </ul>\n" + rule
		if !strings.Contains(got, want) {
			t.Errorf("unexpected output:\n%s", got)
		}
	})

	t.Run("multi-line span", func(t *testing.T) {
		got := render(&syntax.Span{StartLine: 1, StartCol: 3, EndLine: 2, EndCol: 5}, "{% include\n  'x' %}")
		want := "   1 > {% include\n" +
			"     i    ^^^^^^^ template not found\n" +
			"   2 |   'x' %}\n" + rule
		if !strings.Contains(got, want) {
			t.Errorf("unexpected output:\n%s", got)
		}
	})

	t.Run("no location", func(t *testing.T) {
		got := render(nil, "{{ x }}")
		if strings.Contains(got, "{{ x }}") || strings.Contains(got, "~~~") {
			t.Errorf("unexpected source excerpt:\n%s", got)
		}
		if !strings.Contains(got, "No referenced variables") {
			t.Errorf("missing referenced variables:\n%s", got)
		}
	})
}
