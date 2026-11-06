package minijinja

import "testing"

func TestLoopRecursionLikeRust(t *testing.T) {
	runTemplatesCases(t, []templatesCase{
		{
			name:      "recursive loop",
			templates: map[string]string{"t": `{% for x in [[1], [2, [3]]] recursive %}[{{ loop(x[1:]) if x is sequence else x }}]{% endfor %}`},
			expected:  "[][[]]",
		},
		// loop() refers to the innermost loop, which is not recursive.
		{
			name:      "nested loop",
			templates: map[string]string{"t": `{% for x in [[1, [2]]] recursive %}{% for y in [1] %}{{ loop(x[1:]) }}{% endfor %}{% endfor %}`},
			errText:   "cannot recurse outside of recursive loop",
		},
		{
			name:      "nested loop over iterator",
			templates: map[string]string{"t": `{% for x in [[1, [2]]] recursive %}{% for y in range(1) %}{{ loop(x[1:]) }}{% endfor %}{% endfor %}`},
			errText:   "cannot recurse outside of recursive loop",
		},
		{
			name:      "loop that is not recursive",
			templates: map[string]string{"t": `{% for x in [1] %}{% set l = loop %}{{ l([]) }}{% endfor %}`},
			errText:   "cannot recurse outside of recursive loop",
		},
		{
			name:      "outer loop through a variable",
			templates: map[string]string{"t": `{% for x in [[1, [2]]] recursive %}{% set outer = loop %}{% for y in [1] %}{{ outer(x[1:] if x is sequence else []) }}{% endfor %}[{{ x }}]{% endfor %}`},
			expected:  "[[2]][[1, [2]]]",
		},
		{
			name:      "outer loop in iterable and else",
			templates: map[string]string{"t": `{% for x in [[1, [2]]] recursive %}<{{ x }}>{% for y in [loop(x[1:] if x is sequence else [])] %}{{ y }}{% endfor %}{% for y in [] %}{% else %}{{ loop(x[1:] if x is sequence else []) }}{% endfor %}{% endfor %}`},
			expected:  "<[1, [2]]><[2]><[2]>",
		},
		// macros do not see the loop they are called from.
		{
			name:      "macro called in loop",
			templates: map[string]string{"t": `{% macro m() %}{{ loop([]) }}{% endmacro %}{% for x in [1] recursive %}{{ m() }}{% endfor %}`},
			errKind:   ErrUnknownFunction,
		},
	})
}
