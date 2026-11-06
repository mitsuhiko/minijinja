package minijinja

import (
	"errors"
	"strings"
	"testing"
)

// renderTemplates registers the given templates and renders entry.
func renderTemplates(t *testing.T, templates map[string]string, entry string) (string, error) {
	t.Helper()
	env := NewEnvironment()
	for name, source := range templates {
		if err := env.AddTemplate(name, source); err != nil {
			t.Fatalf("failed to add template %q: %v", name, err)
		}
	}
	tmpl, err := env.GetTemplate(entry)
	if err != nil {
		t.Fatalf("failed to get template %q: %v", entry, err)
	}
	return tmpl.Render(nil)
}

type templatesCase struct {
	name      string
	templates map[string]string
	entry     string
	expected  string
	errKind   ErrorKind // if set, rendering must fail with this kind
	errText   string    // if set, the error must contain this text
}

func runTemplatesCases(t *testing.T, cases []templatesCase) {
	t.Helper()
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			entry := tc.entry
			if entry == "" {
				entry = "t"
			}
			result, err := renderTemplates(t, tc.templates, entry)
			if tc.errKind != 0 || tc.errText != "" {
				if err == nil {
					t.Fatalf("expected an error, got %q", result)
				}
				var mjErr *Error
				if tc.errKind != 0 && (!errors.As(err, &mjErr) || mjErr.Kind != tc.errKind) {
					t.Fatalf("expected a %v error, got %v", tc.errKind, err)
				}
				if !strings.Contains(err.Error(), tc.errText) {
					t.Fatalf("expected error containing %q, got %v", tc.errText, err)
				}
				return
			}
			if err != nil {
				t.Fatalf("render error: %v", err)
			}
			if result != tc.expected {
				t.Errorf("expected %q, got %q", tc.expected, result)
			}
		})
	}
}

func TestCallerScopingLikeRust(t *testing.T) {
	nested := `{% macro outer() %}O({{ caller() }}){% endmacro %}` +
		`{% macro inner() %}I({{ caller() }}){% endmacro %}` +
		`{% macro both() %}{% call outer() %}{% call inner() %}{{ caller() }}{% endcall %}{% endcall %}{% endmacro %}`
	runTemplatesCases(t, []templatesCase{
		// caller in a nested call block refers to the caller of that call
		// block, so both() does not accept a caller.  This used to recurse
		// until the Go stack overflowed.
		{
			name: "nested call blocks",
			templates: map[string]string{
				"m": nested,
				"t": `{% from 'm' import both %}{% call both() %}X{% endcall %}`,
			},
			errKind: ErrTooManyArguments,
		},
		{
			name:      "nested call blocks local",
			templates: map[string]string{"t": nested + `{% call both() %}X{% endcall %}`},
			errKind:   ErrTooManyArguments,
		},
		{
			name: "caller of the call block itself",
			templates: map[string]string{
				"t": `{% macro m() %}[{{ caller() }}]{% endmacro %}{% call m() %}{{ caller() }}{% endcall %}`,
			},
			errText: "callable",
		},
		{
			name: "outer caller in call block of macro",
			templates: map[string]string{
				"t": `{% macro m() %}[{{ caller() }}]{% endmacro %}{% macro x() %}{% call m() %}<{{ caller() }}>{% endcall %}{% endmacro %}{% call x() %}C{% endcall %}`,
			},
			errKind: ErrTooManyArguments,
		},
		{
			name: "outer caller passed through a variable",
			templates: map[string]string{
				"t": `{% macro outer() %}O({{ caller() }}){% endmacro %}{% macro both() %}{% set c = caller %}{% call outer() %}{{ c() }}{% endcall %}{% endmacro %}{% call both() %}X{% endcall %}`,
			},
			expected: "O(X)",
		},
		// the call block body sees the variables where it is defined, not
		// the ones of the macro calling it.
		{
			name: "call block closure",
			templates: map[string]string{
				"t": `{% macro m() %}{% set v = 'macro' %}[{{ caller() }}]{% endmacro %}{% set v = 'outer' %}{% call m() %}{{ v }}{% endcall %}`,
			},
			expected: "[outer]",
		},
		{
			name: "recursion through caller",
			templates: map[string]string{
				"t": `{% macro m() %}{{ caller() }}{% endmacro %}{% macro r() %}{% call m() %}{{ r() }}{% endcall %}{% endmacro %}{{ r() }}`,
			},
			errText: "recursion limit exceeded",
		},
		{
			name: "caller arguments",
			templates: map[string]string{
				"t": `{% macro m() %}{{ caller.name }}|{{ caller.caller }}|{{ caller }}|{{ caller.arguments }}|{{ caller(1) }}|{{ caller(b=3) }}{% endmacro %}{% call(a, b=2) m() %}{{ a }}{{ b }}{% endcall %}`,
			},
			expected: "caller|False|<macro caller>|['a', 'b']|12|3",
		},
		{
			name: "too many caller arguments",
			templates: map[string]string{
				"t": `{% macro m() %}{{ caller(1, 2) }}{% endmacro %}{% call(a) m() %}{{ a }}{% endcall %}`,
			},
			errKind: ErrTooManyArguments,
		},
		{
			name: "caller keyword argument",
			templates: map[string]string{
				"t": `{% macro m() %}[{{ caller() }}]{% endmacro %}{% macro c() %}C{% endmacro %}{{ m(caller=c) }}`,
			},
			expected: "[C]",
		},
		{
			name: "macro without caller",
			templates: map[string]string{
				"t": `{% macro m() %}x{% endmacro %}{% call m() %}C{% endcall %}`,
			},
			errKind: ErrTooManyArguments,
		},
		{
			name: "call block calling caller",
			templates: map[string]string{
				"t": `{% macro m() %}<{% call(x) caller() %}{{ x }}{% endcall %}>{% endmacro %}{% call(c) m() %}[{{ c(1) }}]{% endcall %}`,
			},
			errKind: ErrTooManyArguments,
		},
		{
			name: "call block passing a caller to caller",
			templates: map[string]string{
				"t": `{% macro m() %}<{% call caller() %}Y{% endcall %}>{% endmacro %}{% call m() %}[{{ caller() }}]{% endcall %}`,
			},
			expected: "<[Y]>",
		},
	})
}

func TestSuperInMacroLikeRust(t *testing.T) {
	parent := `{% block b %}parent{% endblock %}`
	runTemplatesCases(t, []templatesCase{
		{
			name: "macro defined in block",
			templates: map[string]string{
				"p": parent,
				"c": `{% extends 'p' %}{% block b %}{% macro m() %}<P{{ super() }}>{% endmacro %}{{ m() }}child{% endblock %}`,
			},
			entry:   "c",
			errKind: ErrInvalidOperation,
		},
		{
			name: "macro defined outside of block",
			templates: map[string]string{
				"p": parent,
				"c": `{% extends 'p' %}{% macro m() %}<P{{ super() }}>{% endmacro %}{% block b %}{{ m() }}child{% endblock %}`,
			},
			entry:   "c",
			errKind: ErrInvalidOperation,
		},
		{
			name: "call block body",
			templates: map[string]string{
				"p": parent,
				"c": `{% extends 'p' %}{% macro m() %}[{{ caller() }}]{% endmacro %}{% block b %}{% call m() %}{{ super() }}{% endcall %}child{% endblock %}`,
			},
			entry:   "c",
			errKind: ErrInvalidOperation,
		},
		{
			name: "super next to a macro call",
			templates: map[string]string{
				"p": parent,
				"c": `{% extends 'p' %}{% macro m() %}M{% endmacro %}{% block b %}{{ m() }}{{ super() }}{% endblock %}`,
			},
			entry:    "c",
			expected: "Mparent",
		},
	})
}

func TestMacroAutoEscapeLikeRust(t *testing.T) {
	runTemplatesCases(t, []templatesCase{
		// macros use the auto escaping of the place they are called from,
		// not of the template that defines them.
		{
			name: "html macro called from txt",
			templates: map[string]string{
				"m.html": `{% macro m(x) %}<{{ x }}>{% endmacro %}`,
				"t.txt":  `{% from 'm.html' import m %}{{ m('&') }}`,
			},
			entry:    "t.txt",
			expected: "<&>",
		},
		{
			name: "html module macro called from txt",
			templates: map[string]string{
				"m.html": `{% macro m() %}{{ '<' }}{% endmacro %}{% set v = m() %}`,
				"t.txt":  `{% import 'm.html' as mm %}{{ mm.v }}|{{ mm.m() }}`,
			},
			entry:    "t.txt",
			expected: "&lt;|<",
		},
		{
			name: "txt macro called from html",
			templates: map[string]string{
				"m.txt":  `{% macro m(x) %}<{{ x }}>{{ caller() }}{% endmacro %}`,
				"t.html": `{% from 'm.txt' import m %}{% call m('&') %}{{ '&' }}{% endcall %}`,
			},
			entry:    "t.html",
			expected: "<&amp;>&amp;",
		},
		{
			name: "html macro with caller called from txt",
			templates: map[string]string{
				"m.html": `{% macro m(x) %}<{{ x }}>{{ caller() }}{% endmacro %}`,
				"t.txt":  `{% from 'm.html' import m %}{% call m('&') %}{{ '&' }}{% endcall %}`,
			},
			entry:    "t.txt",
			expected: "<&>&",
		},
		{
			name: "macro defined with auto escaping disabled",
			templates: map[string]string{
				"t.html": `{% autoescape false %}{% macro m(x) %}<{{ x }}>{% endmacro %}{% endautoescape %}{{ m('&') }}`,
			},
			entry:    "t.html",
			expected: "<&amp;>",
		},
		{
			name: "macro called with auto escaping disabled",
			templates: map[string]string{
				"t.html": `{% macro m(x) %}<{{ x }}>{% endmacro %}{% autoescape false %}{{ m('&') }}{% endautoescape %}`,
			},
			entry:    "t.html",
			expected: "<&>",
		},
		// without auto escaping, the result of a macro is not safe.
		{
			name:      "escaping macro output without auto escaping",
			templates: map[string]string{"t.txt": `{% macro m() %}<{% endmacro %}{{ m()|escape }}`},
			entry:     "t.txt",
			expected:  "&lt;",
		},
		{
			name:      "escaping macro output with auto escaping disabled",
			templates: map[string]string{"t.html": `{% macro m() %}<{% endmacro %}{% autoescape false %}{{ m()|escape }}{% endautoescape %}`},
			entry:     "t.html",
			expected:  "&lt;",
		},
	})
}
