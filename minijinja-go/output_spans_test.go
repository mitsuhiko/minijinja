package minijinja

import (
	"fmt"
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/syntax"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// renderSpans renders a template and returns a description of the recorded
// spans in the same format as the Rust tests in test_output_spans.rs.
//
// This also checks that the spans cover the entire output without gaps and
// that raw spans point to the exact source that was written.
func renderSpans(t *testing.T, env *Environment, name string, ctx any) string {
	t.Helper()
	tmpl, err := env.GetTemplate(name)
	if err != nil {
		t.Fatal(err)
	}
	output, spans, err := tmpl.RenderWithSpans(ctx)
	if err != nil {
		t.Fatal(err)
	}
	expected, err := tmpl.Render(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if output != expected {
		t.Fatalf("output differs from Render:\n%q\n%q", output, expected)
	}

	pos := 0
	var rv strings.Builder
	for _, span := range spans {
		if span.OutputStart != pos {
			t.Fatalf("gap or overlap in output spans at %d: %+v", pos, span)
		}
		if span.OutputEnd <= span.OutputStart {
			t.Fatalf("empty output span: %+v", span)
		}
		pos = span.OutputEnd

		spanTmpl, err := env.GetTemplate(span.Name)
		if err != nil {
			t.Fatal(err)
		}
		source := spanTmpl.Source()
		start, end := span.SourceRange()
		sourceText := source[start:end]
		outputText := output[span.OutputStart:span.OutputEnd]
		line := strings.Count(source[:start], "\n") + 1
		if span.Line() != line {
			t.Fatalf("line %d does not match source range (line %d): %+v", span.Line(), line, span)
		}
		if span.Kind == OutputSpanRaw && outputText != sourceText {
			t.Fatalf("raw output %q does not match source %q", outputText, sourceText)
		}
		fmt.Fprintf(&rv, "%s:%d %s %q <- %q\n", span.Name, span.Line(), span.Kind, outputText, sourceText)
	}
	if pos != len(output) {
		t.Fatalf("output not fully covered by spans: %d of %d", pos, len(output))
	}
	return rv.String()
}

func assertSpans(t *testing.T, got, expected string) {
	t.Helper()
	expected = strings.TrimLeft(expected, "\n")
	if got != expected {
		t.Errorf("unexpected spans:\n--- got ---\n%s--- expected ---\n%s", got, expected)
	}
}

func TestOutputSpansBasic(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("t.txt", "Hello {{ name }}!\n{% for item in items %}- {{ item|upper }}\n{% endfor %}{{ empty }}{{\n  name\n}}")
	assertSpans(t, renderSpans(t, env, "t.txt", map[string]any{"name": "World", "items": []string{"a", "b"}, "empty": ""}), `
t.txt:1 Raw "Hello " <- "Hello "
t.txt:1 Expression "World" <- "name"
t.txt:1 Raw "!\n" <- "!\n"
t.txt:2 Raw "- " <- "- "
t.txt:2 Expression "A" <- "item|upper"
t.txt:2 Raw "\n" <- "\n"
t.txt:2 Raw "- " <- "- "
t.txt:2 Expression "B" <- "item|upper"
t.txt:2 Raw "\n" <- "\n"
t.txt:4 Expression "World" <- "name"
`)
}

func TestOutputSpansWhitespaceControl(t *testing.T) {
	env := NewEnvironment()
	env.SetWhitespace(syntax.WhitespaceConfig{TrimBlocks: true, LstripBlocks: true})
	env.AddTemplate("t.txt", "<ul>\n  {% for x in seq %}\n  <li>{{- x -}}  </li>\n  {% endfor %}\n</ul>\n{% raw %}\n  {{ raw }}  {%- endraw %}!")
	assertSpans(t, renderSpans(t, env, "t.txt", map[string]any{"seq": []int{1, 2}}), `
t.txt:1 Raw "<ul>\n" <- "<ul>\n"
t.txt:3 Raw "  <li>" <- "  <li>"
t.txt:3 Expression "1" <- "x"
t.txt:3 Raw "</li>\n" <- "</li>\n"
t.txt:3 Raw "  <li>" <- "  <li>"
t.txt:3 Expression "2" <- "x"
t.txt:3 Raw "</li>\n" <- "</li>\n"
t.txt:5 Raw "</ul>\n" <- "</ul>\n"
t.txt:7 Raw "  {{ raw }}" <- "  {{ raw }}"
t.txt:7 Raw "!" <- "!"
`)
}

func TestOutputSpansCapturedOutput(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("t.txt", "{% macro m(x) %}[{{ x }}{{ caller() if caller }}]{% endmacro %}"+
		"{{ m(1) }}|{% call m(2) %}c{% endcall %}|"+
		"{% filter upper|trim %} a{{ 'b' }} {% endfilter %}|"+
		"{% set x %}captured{% endset %}{{ x }}")
	assertSpans(t, renderSpans(t, env, "t.txt", nil), `
t.txt:1 Expression "[1]" <- "m(1)"
t.txt:1 Raw "|" <- "|"
t.txt:1 Expression "[2c]" <- "m(2)"
t.txt:1 Raw "|" <- "|"
t.txt:1 Expression "AB" <- "upper|trim"
t.txt:1 Raw "|" <- "|"
t.txt:1 Expression "captured" <- "x"
`)
}

func TestOutputSpansRecursiveLoop(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("t.txt", "{% for item in tree recursive %}({{ item.name }}{{ loop(item.children) }}){% endfor %}")
	tree := map[string]any{"tree": []any{
		map[string]any{"name": "a", "children": []any{map[string]any{"name": "b", "children": []any{}}}},
	}}
	assertSpans(t, renderSpans(t, env, "t.txt", tree), `
t.txt:1 Raw "(" <- "("
t.txt:1 Expression "a" <- "item.name"
t.txt:1 Raw "(" <- "("
t.txt:1 Expression "b" <- "item.name"
t.txt:1 Raw ")" <- ")"
t.txt:1 Raw ")" <- ")"
`)
}

func TestOutputSpansRecursiveLoopFromInclude(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("i", "|{{ loop(x[1:] if x is sequence else []) }}")
	env.AddTemplate("t", "{% for x in [[1,[2]]] recursive %}<{{ x }}{% include 'i' %}>{% endfor %}!")
	assertSpans(t, renderSpans(t, env, "t", nil), `
t:1 Raw "<" <- "<"
t:1 Expression "[1, [2]]" <- "x"
i:1 Raw "|" <- "|"
t:1 Raw "<" <- "<"
t:1 Expression "[2]" <- "x"
i:1 Raw "|" <- "|"
t:1 Raw ">" <- ">"
t:1 Raw ">" <- ">"
t:1 Raw "!" <- "!"
`)
}

// loop() in an included template writes to the current output of the
// included template, even if that is captured.
func TestRecursiveLoopFromIncludeInCapture(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("if", "{% filter upper %}[{{ loop(x[1:] if x is sequence else []) }}]{% endfilter %}")
	env.AddTemplate("is", "{% set c %}[{{ loop(x[1:] if x is sequence else []) }}]{% endset %}({{ c }})")
	env.AddTemplate("tf", "{% for x in [['a',['b']]] recursive %}<{{ x[0] }}{% include 'if' %}>{% endfor %}")
	env.AddTemplate("ts", "{% for x in [['a',['b']]] recursive %}<{{ x[0] }}{% include 'is' %}>{% endfor %}")
	for name, expected := range map[string]string{"tf": "<a[<B[]>]>", "ts": "<a([<b([])>])>"} {
		tmpl, _ := env.GetTemplate(name)
		out, _, err := tmpl.RenderWithSpans(nil)
		if err != nil {
			t.Fatal(err)
		}
		rendered, err := tmpl.Render(nil)
		if err != nil {
			t.Fatal(err)
		}
		if out != expected || rendered != expected {
			t.Errorf("%s: got %q / %q, expected %q", name, out, rendered, expected)
		}
	}
}

func TestOutputSpansMultiTemplate(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("layout.html", "<title>{% block title %}Default{% endblock %}</title>\n{% block body %}{% endblock %}")
	env.AddTemplate("item.html", "<li>{{ item }}</li>")
	env.AddTemplate("page.html", "{% extends \"layout.html\" %}ignored"+
		"{% block title %}{{ title }} - {{ super() }}{% endblock %}"+
		"{% block body %}<ul>{% for item in items %}{% include \"item.html\" %}{% endfor %}</ul>{% endblock %}")
	assertSpans(t, renderSpans(t, env, "page.html", map[string]any{"title": "Page", "items": []int{1, 2}}), `
layout.html:1 Raw "<title>" <- "<title>"
page.html:1 Expression "Page" <- "title"
page.html:1 Raw " - " <- " - "
layout.html:1 Raw "Default" <- "Default"
layout.html:1 Raw "</title>\n" <- "</title>\n"
page.html:1 Raw "<ul>" <- "<ul>"
item.html:1 Raw "<li>" <- "<li>"
item.html:1 Expression "1" <- "item"
item.html:1 Raw "</li>" <- "</li>"
item.html:1 Raw "<li>" <- "<li>"
item.html:1 Expression "2" <- "item"
item.html:1 Raw "</li>" <- "</li>"
page.html:1 Raw "</ul>" <- "</ul>"
`)
}

func TestOutputSpansNestedExtendsAndSelf(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("base.txt", "[{% block a %}base{% endblock %}]{{ self.a() }}")
	env.AddTemplate("middle.txt", "{% extends 'base.txt' %}{% block a %}middle {{ super() }}{% endblock %}")
	env.AddTemplate("child.txt", "{% extends 'middle.txt' %}{% block a %}child {{ super() }}{% endblock %}")
	assertSpans(t, renderSpans(t, env, "child.txt", nil), `
base.txt:1 Raw "[" <- "["
child.txt:1 Raw "child " <- "child "
middle.txt:1 Raw "middle " <- "middle "
base.txt:1 Raw "base" <- "base"
base.txt:1 Raw "]" <- "]"
child.txt:1 Raw "child " <- "child "
middle.txt:1 Raw "middle " <- "middle "
base.txt:1 Raw "base" <- "base"
`)
}

func TestOutputSpansCustomFormatterAndEscaping(t *testing.T) {
	env := NewEnvironment()
	env.AddTemplate("t.html", "<p>{{ value }}</p>")
	assertSpans(t, renderSpans(t, env, "t.html", map[string]any{"value": "<b>"}), `
t.html:1 Raw "<p>" <- "<p>"
t.html:1 Expression "&lt;b&gt;" <- "value"
t.html:1 Raw "</p>" <- "</p>"
`)

	env.SetFormatter(func(state *State, val value.Value, escape func(string) string) string {
		return "<" + val.String() + ">"
	})
	assertSpans(t, renderSpans(t, env, "t.html", map[string]any{"value": 42}), `
t.html:1 Raw "<p>" <- "<p>"
t.html:1 Expression "<42>" <- "value"
t.html:1 Raw "</p>" <- "</p>"
`)
}

func TestOutputSpansError(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("a{{ 1 / 0 }}")
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := tmpl.RenderWithSpans(nil); err == nil {
		t.Fatal("expected an error")
	}
}

// Templates rendered through extends and blocks execute in the context of
// the template that defines them, like in Rust.
func TestStateNameInParentTemplate(t *testing.T) {
	env := NewEnvironment()
	env.SetDebug(true)
	env.AddFunction("name", func(state *State, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		return value.FromString(state.Name()), nil
	})
	env.AddTemplate("parent", "{{ name() }}{% block b %}{{ name() }}{% endblock %}{% block c %}{{ name() }}{% endblock %}")
	env.AddTemplate("child", "{% extends 'parent' %}{% block b %}[{{ name() }}|{{ super() }}]{% endblock %}")
	tmpl, _ := env.GetTemplate("child")
	out, err := tmpl.Render(nil)
	if err != nil {
		t.Fatal(err)
	}
	if out != "parent[child|parent]parent" {
		t.Errorf("unexpected output: %q", out)
	}

	env.AddTemplate("broken_parent", "a\n{% block b %}{{ 1 + none }}{% endblock %}")
	env.AddTemplate("broken_child", "{% extends 'broken_parent' %}")
	tmpl, _ = env.GetTemplate("broken_child")
	_, err = tmpl.Render(nil)
	templErr, ok := err.(*Error)
	if !ok {
		t.Fatalf("expected template error, got %v", err)
	}
	if templErr.Name != "broken_parent" || templErr.Span == nil || templErr.Span.StartLine != 2 {
		t.Errorf("error points to the wrong location: %v", err)
	}
}

// Macros and call block bodies execute in the context of the template that
// defines them, like in Rust.
func TestStateNameInMacros(t *testing.T) {
	env := NewEnvironment()
	env.SetDebug(true)
	env.AddFunction("name", func(state *State, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		return value.FromString(state.Name()), nil
	})
	env.AddTemplate("parent", "{% macro pm() %}{{ name() }}{% endmacro %}{% macro wrap() %}{{ caller() }}{% endmacro %}{{ cm() }}|{% block b %}{% endblock %}")
	env.AddTemplate("child", "{% extends 'parent' %}{% macro cm() %}{{ name() }}{% endmacro %}{% block b %}{{ pm() }}|{% call wrap() %}{{ name() }}{% endcall %}{% endblock %}")
	tmpl, _ := env.GetTemplate("child")
	out, err := tmpl.Render(nil)
	if err != nil {
		t.Fatal(err)
	}
	if out != "child|parent|child" {
		t.Errorf("unexpected output: %q", out)
	}

	env.AddTemplate("parent2", "line1\nline2\n{{ cm() }}{% block b %}{% endblock %}")
	env.AddTemplate("child2", "{% extends 'parent2' %}{% macro cm() %}\n\n\n\n{{ 1 / 0 }}{% endmacro %}")
	tmpl, _ = env.GetTemplate("child2")
	_, err = tmpl.Render(nil)
	templErr, ok := err.(*Error)
	if !ok {
		t.Fatalf("expected template error, got %v", err)
	}
	if templErr.Name != "child2" || templErr.Span == nil || templErr.Span.StartLine != 5 {
		t.Errorf("error points to the wrong location: %v", err)
	}
}
