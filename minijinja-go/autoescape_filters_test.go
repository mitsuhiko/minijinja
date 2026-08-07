package minijinja

import (
	"bytes"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

func filterTestState(t *testing.T, env *Environment, name string) *State {
	t.Helper()
	tmpl, err := env.TemplateFromNamedString(name, "")
	if err != nil {
		t.Fatal(err)
	}
	state, err := tmpl.EvalToState(nil)
	if err != nil {
		t.Fatal(err)
	}
	return state
}

func TestStringFiltersPreserveSafety(t *testing.T) {
	env := NewEnvironment()
	state := filterTestState(t, env, "test.txt")
	safe := value.FromSafeString(" Hello\nWorld ")
	plain := value.FromString(" Hello\nWorld ")

	for _, filter := range []string{"upper", "lower", "capitalize", "reverse", "trim", "last", "indent"} {
		t.Run(filter, func(t *testing.T) {
			result, err := state.ApplyFilter(filter, safe, nil, nil)
			if err != nil {
				t.Fatal(err)
			}
			if !result.IsSafe() {
				t.Fatalf("%s discarded safety", filter)
			}

			result, err = state.ApplyFilter(filter, plain, nil, nil)
			if err != nil {
				t.Fatal(err)
			}
			if result.IsSafe() {
				t.Fatalf("%s marked a plain string safe", filter)
			}
		})
	}

	for _, filter := range []string{"split", "lines"} {
		t.Run(filter, func(t *testing.T) {
			result, err := state.ApplyFilter(filter, safe, nil, nil)
			if err != nil {
				t.Fatal(err)
			}
			for _, item := range result.Iter() {
				if !item.IsSafe() {
					t.Fatalf("%s discarded item safety", filter)
				}
			}

			result, err = state.ApplyFilter(filter, plain, nil, nil)
			if err != nil {
				t.Fatal(err)
			}
			for _, item := range result.Iter() {
				if item.IsSafe() {
					t.Fatalf("%s marked a plain item safe", filter)
				}
			}
		})
	}
}

func TestByteStringFilterSemantics(t *testing.T) {
	env := NewEnvironment()
	state := filterTestState(t, env, "test.txt")

	reversed, err := state.ApplyFilter("reverse", value.FromBytes([]byte("éa")), nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	if got, ok := reversed.Raw().([]byte); !ok || !bytes.Equal(got, []byte{'a', 0xa9, 0xc3}) {
		t.Fatalf("unexpected reversed bytes: %#v", reversed.Raw())
	}

	input := value.FromBytes([]byte{'a', 0xff, 'b', '\n', 'c'})
	for _, filter := range []string{"split", "lines"} {
		result, err := state.ApplyFilter(filter, input, nil, nil)
		if err != nil {
			t.Fatal(err)
		}
		items := result.Iter()
		if len(items) != 2 || items[0].String() != "a�b" || items[1].String() != "c" {
			t.Fatalf("unexpected %s result: %s", filter, result.Repr())
		}
	}
}

func TestComposingFiltersEscapeUnsafeFragments(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromNamedString(
		"test.html",
		"{{ source|replace('x', replacement) }}|{{ values|join(delimiter) }}|{{ format_string|format(user) }}",
	)
	if err != nil {
		t.Fatal(err)
	}

	output, err := tmpl.Render(map[string]any{
		"source":      value.FromSafeString("<b>x</b>"),
		"replacement": "<i>y</i>",
		"values": []value.Value{
			value.FromSafeString("<b>a</b>"),
			value.FromString("<i>b</i>"),
		},
		"delimiter":     "<hr>",
		"format_string": value.FromSafeString("<b>%s</b>"),
		"user":          "<i>x</i>",
	})
	if err != nil {
		t.Fatal(err)
	}
	expected := "<b>&lt;i&gt;y&lt;&#x2f;i&gt;</b>|<b>a</b>&lt;hr&gt;&lt;i&gt;b&lt;&#x2f;i&gt;|<b>&lt;i&gt;x&lt;&#x2f;i&gt;</b>"
	if output != expected {
		t.Fatalf("unexpected output:\nwant %q\n got %q", expected, output)
	}

	state := filterTestState(t, env, "state.html")
	result, err := state.ApplyFilter("replace", value.FromString("<b>TEXT</b>"), []value.Value{
		value.FromString("TEXT"),
		value.FromSafeString("<i>y</i>"),
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !result.IsSafe() || result.String() != "&lt;b&gt;<i>y</i>&lt;&#x2f;b&gt;" {
		t.Fatalf("unexpected safe replace result: %s", result.Repr())
	}

	result, err = state.ApplyFilter("join", value.FromSlice([]value.Value{
		value.FromString("<b>a</b>"),
		value.FromString("<i>b</i>"),
	}), []value.Value{value.FromSafeString("<hr>")}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !result.IsSafe() || result.String() != "&lt;b&gt;a&lt;&#x2f;b&gt;<hr>&lt;i&gt;b&lt;&#x2f;i&gt;" {
		t.Fatalf("unexpected safe join result: %s", result.Repr())
	}
}

func TestSafeFormatEscapesWithoutAutoEscape(t *testing.T) {
	env := NewEnvironment()
	state := filterTestState(t, env, "test.txt")

	result, err := state.ApplyFilter("format", value.FromSafeString("<b>%(user)s</b>"), []value.Value{
		value.FromMap(map[string]value.Value{"user": value.FromString("<i>x</i>")}),
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !result.IsSafe() || result.String() != "<b>&lt;i&gt;x&lt;&#x2f;i&gt;</b>" {
		t.Fatalf("unexpected safe format result: %s", result.Repr())
	}

	result, err = state.ApplyFilter("format", value.FromSafeString("%d"), []value.Value{value.FromInt(42)}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !result.IsSafe() || result.String() != "42" {
		t.Fatalf("unexpected safe numeric result: %s", result.Repr())
	}

	result, err = state.ApplyFilter("format", value.FromSafeString("%.5s"), []value.Value{value.FromString("<é")}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !result.IsSafe() || result.String() != "&lt;é" {
		t.Fatalf("unexpected safe precision result: %s", result.Repr())
	}

	_, err = state.ApplyFilter("format", value.FromSafeString("%c"), []value.Value{value.FromInt(60)}, nil)
	if err == nil || !strings.Contains(err.Error(), "character formatting is not supported for safe format strings") {
		t.Fatalf("unexpected character format error: %v", err)
	}

	plainResults := []struct {
		name string
		val  value.Value
		args []value.Value
	}{
		{
			name: "replace",
			val:  value.FromString("<b>x</b>"),
			args: []value.Value{value.FromString("x"), value.FromSafeString("y")},
		},
		{
			name: "join",
			val:  value.FromSlice([]value.Value{value.FromSafeString("<b>x</b>")}),
			args: []value.Value{value.FromString(",")},
		},
		{
			name: "format",
			val:  value.FromString("<b>%s</b>"),
			args: []value.Value{value.FromSafeString("<i>x</i>")},
		},
	}
	for _, test := range plainResults {
		result, err := state.ApplyFilter(test.name, test.val, test.args, nil)
		if err != nil {
			t.Fatal(err)
		}
		if result.IsSafe() {
			t.Fatalf("%s unexpectedly produced a safe value without auto-escape", test.name)
		}
	}
}

func TestFilterSafetyUsesCustomFormatter(t *testing.T) {
	env := NewEnvironment()
	env.SetAutoEscapeFunc(func(string) AutoEscape { return AutoEscapeCustom("stars") })
	env.SetFormatter(func(_ *State, val value.Value, _ func(string) string) string {
		if val.IsSafe() {
			return val.String()
		}
		return strings.ReplaceAll(val.String(), "*", `\*`)
	})

	tmpl, err := env.TemplateFromString("{{ '%s'|safe|format(value) }}|{{ values|join('*') }}")
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(map[string]any{
		"value": "*",
		"values": []value.Value{
			value.FromSafeString("ok"),
			value.FromString("end"),
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if result != `\*|ok\*end` {
		t.Fatalf("unexpected custom-formatted result: %q", result)
	}
}

func TestIndentPreservesSafeFilterBlocks(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromNamedString(
		"test.html",
		"{% filter indent(2) %}<p>one</p>\n<p>two</p>{% endfilter %}",
	)
	if err != nil {
		t.Fatal(err)
	}
	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatal(err)
	}
	if result != "<p>one</p>\n  <p>two</p>" {
		t.Fatalf("unexpected indented filter block: %q", result)
	}
}
