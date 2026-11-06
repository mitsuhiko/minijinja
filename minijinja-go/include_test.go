package minijinja

import "testing"

func TestIncludedTemplateBlocksLikeRust(t *testing.T) {
	runTemplatesCases(t, []templatesCase{
		{
			name: "block of the same name",
			templates: map[string]string{
				"i": `{% block a %}I{% endblock %}`,
				"t": `{% block a %}A{% endblock %}{% include 'i' %}`,
			},
			expected: "AI",
		},
		{
			name: "block overridden by child template",
			templates: map[string]string{
				"i": `{% block a %}I{% endblock %}`,
				"p": `{% block a %}P{% endblock %}{% block b %}{% endblock %}`,
				"c": `{% extends 'p' %}{% block a %}C{% endblock %}{% block b %}{% include 'i' %}{% endblock %}`,
			},
			entry:    "c",
			expected: "CI",
		},
		{
			name: "self in included template",
			templates: map[string]string{
				"i": `{% block b %}IB{% endblock %}{{ self.b() }}`,
				"p": `{% block a %}P{% endblock %}{% block b %}PB{% endblock %}`,
				"c": `{% extends 'p' %}{% block a %}{% include 'i' %}{% endblock %}{% block b %}CB{% endblock %}`,
			},
			entry:    "c",
			expected: "IBIBCB",
		},
		{
			name: "included template extending another",
			templates: map[string]string{
				"ip": `{% block a %}IP{% endblock %}`,
				"i":  `{% extends 'ip' %}{% block a %}I{{ super() }}{% endblock %}`,
				"t":  `{% block a %}A{% endblock %}{% include 'i' %}{{ self.a() }}`,
			},
			expected: "AIIPA",
		},
		{
			name: "block of the includer",
			templates: map[string]string{
				"i": `{{ self.a() }}`,
				"t": `{% block a %}A{% endblock %}{% include 'i' %}`,
			},
			errKind: ErrBadInclude,
		},
	})
}
