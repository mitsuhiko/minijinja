package minijinja

import (
	"strings"
	"testing"
)

func TestVersion(t *testing.T) {
	if Version == "" {
		t.Error("Version should not be empty")
	}
}

func TestBasicRender(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("Hello {{ name }}!")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"name": "World"})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "Hello World!" {
		t.Errorf("expected 'Hello World!', got %q", result)
	}
}

func TestVariableTypes(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{{ str }} {{ num }} {{ float }} {{ bool }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"str":   "hello",
		"num":   42,
		"float": 3.14,
		"bool":  true,
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "hello 42 3.14 true" {
		t.Errorf("expected 'hello 42 3.14 true', got %q", result)
	}
}

func TestForLoop(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% for item in items %}{{ item }}{% endfor %}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"items": []string{"a", "b", "c"},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "abc" {
		t.Errorf("expected 'abc', got %q", result)
	}
}

func TestForLoopWithIndex(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% for item in items %}{{ loop.index }}:{{ item }} {% endfor %}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"items": []string{"a", "b", "c"},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "1:a 2:b 3:c " {
		t.Errorf("expected '1:a 2:b 3:c ', got %q", result)
	}
}

func TestForLoopElse(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% for item in items %}{{ item }}{% else %}empty{% endfor %}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"items": []string{},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "empty" {
		t.Errorf("expected 'empty', got %q", result)
	}
}

func TestIfCondition(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% if show %}visible{% endif %}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"show": true})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "visible" {
		t.Errorf("expected 'visible', got %q", result)
	}

	result, err = tmpl.Render(map[string]any{"show": false})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "" {
		t.Errorf("expected '', got %q", result)
	}
}

func TestIfElse(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% if show %}yes{% else %}no{% endif %}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"show": true})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "yes" {
		t.Errorf("expected 'yes', got %q", result)
	}

	result, err = tmpl.Render(map[string]any{"show": false})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "no" {
		t.Errorf("expected 'no', got %q", result)
	}
}

func TestIfElif(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% if x == 1 %}one{% elif x == 2 %}two{% else %}other{% endif %}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	tests := []struct {
		x        int
		expected string
	}{
		{1, "one"},
		{2, "two"},
		{3, "other"},
	}

	for _, test := range tests {
		result, err := tmpl.Render(map[string]any{"x": test.x})
		if err != nil {
			t.Fatalf("render error: %v", err)
		}
		if result != test.expected {
			t.Errorf("x=%d: expected %q, got %q", test.x, test.expected, result)
		}
	}
}

func TestArithmetic(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		expected string
	}{
		{"{{ 1 + 2 }}", "3"},
		{"{{ 10 - 3 }}", "7"},
		{"{{ 4 * 5 }}", "20"},
		{"{{ 10 / 4 }}", "2.5"},
		{"{{ 10 // 4 }}", "2"},
		{"{{ 10 % 3 }}", "1"},
		{"{{ 2 ** 3 }}", "8"},
		{"{{ -5 }}", "-5"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(nil)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestComparisons(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		expected string
	}{
		{"{{ 1 == 1 }}", "true"},
		{"{{ 1 != 2 }}", "true"},
		{"{{ 1 < 2 }}", "true"},
		{"{{ 2 > 1 }}", "true"},
		{"{{ 1 <= 1 }}", "true"},
		{"{{ 2 >= 2 }}", "true"},
		{"{{ 1 == 2 }}", "false"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(nil)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestLogicalOperators(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		expected string
	}{
		{"{{ true and true }}", "true"},
		{"{{ true and false }}", "false"},
		{"{{ false or true }}", "true"},
		{"{{ false or false }}", "false"},
		{"{{ not true }}", "false"},
		{"{{ not false }}", "true"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(nil)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestStringConcat(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{{ 'hello' ~ ' ' ~ 'world' }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "hello world" {
		t.Errorf("expected 'hello world', got %q", result)
	}
}

func TestFilters(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		expected string
	}{
		{"{{ 'hello'|upper }}", "HELLO"},
		{"{{ 'HELLO'|lower }}", "hello"},
		{"{{ 'hello'|capitalize }}", "Hello"},
		{"{{ '  hello  '|trim }}", "hello"},
		{"{{ [1,2,3]|length }}", "3"},
		{"{{ [1,2,3]|first }}", "1"},
		{"{{ [1,2,3]|last }}", "3"},
		{"{{ [3,1,2]|sort|join(',') }}", "1,2,3"},
		{"{{ [1,2,3]|join('-') }}", "1-2-3"},
		{"{{ 'hello'|replace('l','x') }}", "hexxo"},
		{"{{ 5|abs }}", "5"},
		{"{{ -5|abs }}", "5"},
		{"{{ 3.7|int }}", "3"},
		{"{{ 3|float }}", "3.0"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(nil)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestDefaultFilter(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		ctx      map[string]any
		expected string
	}{
		{"{{ x|default('fallback') }}", nil, "fallback"},
		{"{{ x|default('fallback') }}", map[string]any{"x": "value"}, "value"},
		{"{{ x|d('fallback') }}", nil, "fallback"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(test.ctx)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestTests(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		ctx      map[string]any
		expected string
	}{
		{"{{ x is defined }}", map[string]any{"x": 1}, "true"},
		{"{{ y is defined }}", map[string]any{"x": 1}, "false"},
		{"{{ x is undefined }}", map[string]any{"x": 1}, "false"},
		{"{{ y is undefined }}", map[string]any{"x": 1}, "true"},
		{"{{ none is none }}", nil, "true"},
		{"{{ 1 is none }}", nil, "false"},
		{"{{ 3 is odd }}", nil, "true"},
		{"{{ 4 is even }}", nil, "true"},
		{"{{ 10 is divisibleby(5) }}", nil, "true"},
		{"{{ 10 is divisibleby(3) }}", nil, "false"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(test.ctx)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestInOperator(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		ctx      map[string]any
		expected string
	}{
		{"{{ 'a' in 'abc' }}", nil, "true"},
		{"{{ 'd' in 'abc' }}", nil, "false"},
		{"{{ 1 in [1,2,3] }}", nil, "true"},
		{"{{ 4 in [1,2,3] }}", nil, "false"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(test.ctx)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestAttributeAccess(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{{ user.name }} is {{ user.age }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"user": map[string]any{
			"name": "Alice",
			"age":  30,
		},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "Alice is 30" {
		t.Errorf("expected 'Alice is 30', got %q", result)
	}
}

func TestIndexAccess(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{{ items[0] }} {{ items[1] }} {{ items[-1] }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"items": []string{"a", "b", "c"},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "a b c" {
		t.Errorf("expected 'a b c', got %q", result)
	}
}

func TestRangeFunction(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		expected string
	}{
		{"{% for i in range(3) %}{{ i }}{% endfor %}", "012"},
		{"{% for i in range(1, 4) %}{{ i }}{% endfor %}", "123"},
		{"{% for i in range(0, 6, 2) %}{{ i }}{% endfor %}", "024"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(nil)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestSetStatement(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% set x = 5 %}{{ x }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "5" {
		t.Errorf("expected '5', got %q", result)
	}
}

func TestWithBlock(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{% with x = 5, y = 10 %}{{ x + y }}{% endwith %}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "15" {
		t.Errorf("expected '15', got %q", result)
	}
}

func TestTernaryExpression(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString("{{ 'yes' if x else 'no' }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"x": true})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "yes" {
		t.Errorf("expected 'yes', got %q", result)
	}

	result, err = tmpl.Render(map[string]any{"x": false})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}
	if result != "no" {
		t.Errorf("expected 'no', got %q", result)
	}
}

func TestHTMLEscaping(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromNamedString("test.html", "{{ content }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"content": "<script>alert('xss')</script>"})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if !strings.Contains(result, "&lt;") {
		t.Errorf("expected HTML escaping, got %q", result)
	}
}

func TestSafeFilter(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromNamedString("test.html", "{{ content|safe }}")
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"content": "<b>bold</b>"})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "<b>bold</b>" {
		t.Errorf("expected '<b>bold</b>', got %q", result)
	}
}

func TestMacro(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{% macro greet(name) %}Hello {{ name }}!{% endmacro %}{{ greet("World") }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "Hello World!" {
		t.Errorf("expected 'Hello World!', got %q", result)
	}
}

func TestMacroWithDefault(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{% macro greet(name="Guest") %}Hello {{ name }}!{% endmacro %}{{ greet() }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "Hello Guest!" {
		t.Errorf("expected 'Hello Guest!', got %q", result)
	}
}

func TestInclude(t *testing.T) {
	env := NewEnvironment()
	err := env.AddTemplate("header.html", "Header: {{ title }}")
	if err != nil {
		t.Fatalf("add template error: %v", err)
	}

	tmpl, err := env.TemplateFromString(`{% include "header.html" %}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{"title": "Welcome"})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "Header: Welcome" {
		t.Errorf("expected 'Header: Welcome', got %q", result)
	}
}

func TestSlicing(t *testing.T) {
	env := NewEnvironment()

	tests := []struct {
		template string
		expected string
	}{
		{"{{ 'hello'[1:3] }}", "el"},
		{"{{ 'hello'[:3] }}", "hel"},
		{"{{ 'hello'[2:] }}", "llo"},
		{"{{ [1,2,3,4,5][1:4] }}", "[2, 3, 4]"},
	}

	for _, test := range tests {
		tmpl, err := env.TemplateFromString(test.template)
		if err != nil {
			t.Fatalf("parse error for %q: %v", test.template, err)
		}
		result, err := tmpl.Render(nil)
		if err != nil {
			t.Fatalf("render error for %q: %v", test.template, err)
		}
		if result != test.expected {
			t.Errorf("%q: expected %q, got %q", test.template, test.expected, result)
		}
	}
}

func TestDictLiteral(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{{ {'a': 1, 'b': 2}.a }}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(nil)
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "1" {
		t.Errorf("expected '1', got %q", result)
	}
}

func TestNestedForLoop(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{% for row in rows %}{% for col in row %}{{ col }}{% endfor %},{% endfor %}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"rows": [][]int{{1, 2}, {3, 4}},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "12,34," {
		t.Errorf("expected '12,34,', got %q", result)
	}
}

func TestForLoopFilter(t *testing.T) {
	env := NewEnvironment()
	tmpl, err := env.TemplateFromString(`{% for x in items if x > 2 %}{{ x }}{% endfor %}`)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}

	result, err := tmpl.Render(map[string]any{
		"items": []int{1, 2, 3, 4, 5},
	})
	if err != nil {
		t.Fatalf("render error: %v", err)
	}

	if result != "345" {
		t.Errorf("expected '345', got %q", result)
	}
}
