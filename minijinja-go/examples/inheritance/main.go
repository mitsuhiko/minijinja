// Example: Template inheritance and macros
//
// This example demonstrates Jinja2's powerful template inheritance system
// and macro functionality.
package main

import (
	"fmt"
	"log"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

func main() {
	env := minijinja.NewEnvironment()

	// =========================================
	// Template Inheritance
	// =========================================

	// Base template with blocks that can be overridden
	err := env.AddTemplate("base.html", `<!DOCTYPE html>
<html>
<head>
    <title>{% block title %}My Site{% endblock %}</title>
    {% block head %}
    <link rel="stylesheet" href="style.css">
    {% endblock %}
</head>
<body>
    <nav>{% block nav %}Home | About | Contact{% endblock %}</nav>
    
    <main>
        {% block content %}
        <p>Default content</p>
        {% endblock %}
    </main>
    
    <footer>{% block footer %}© 2024{% endblock %}</footer>
</body>
</html>
`)
	if err != nil {
		log.Fatal(err)
	}

	// Child template that extends the base
	err = env.AddTemplate("page.html", `{% extends "base.html" %}

{% block title %}{{ page_title }} - My Site{% endblock %}

{% block head %}
{{ super() }}
<script src="page.js"></script>
{% endblock %}

{% block content %}
<h1>{{ page_title }}</h1>
<p>{{ content }}</p>
{% endblock %}
`)
	if err != nil {
		log.Fatal(err)
	}

	// =========================================
	// Macros
	// =========================================

	// Template with reusable macros
	err = env.AddTemplate("macros.html", `{# Macro for rendering a form field #}
{% macro field(name, label, type="text", value="") %}
<div class="form-field">
    <label for="{{ name }}">{{ label }}</label>
    <input type="{{ type }}" id="{{ name }}" name="{{ name }}" value="{{ value }}">
</div>
{% endmacro %}

{# Macro for rendering a user card #}
{% macro user_card(user) %}
<div class="user-card">
    <h3>{{ user.name }}</h3>
    <p>Email: {{ user.email }}</p>
    {% if user.admin %}
    <span class="badge">Admin</span>
    {% endif %}
</div>
{% endmacro %}

{# Macro that uses caller() for content injection #}
{% macro card(title) %}
<div class="card">
    <div class="card-header">{{ title }}</div>
    <div class="card-body">
        {{ caller() }}
    </div>
</div>
{% endmacro %}
`)
	if err != nil {
		log.Fatal(err)
	}

	// Template that uses macros
	err = env.AddTemplate("form.html", `{% from "macros.html" import field, user_card, card %}

<h2>Registration Form</h2>
<form>
    {{ field("username", "Username") }}
    {{ field("email", "Email Address", type="email") }}
    {{ field("password", "Password", type="password") }}
</form>

<h2>Team Members</h2>
{% for user in users %}
    {{ user_card(user) }}
{% endfor %}

<h2>Using call blocks</h2>
{% call card("Important Notice") %}
<p>This content is injected via caller()!</p>
<p>You can put any content here.</p>
{% endcall %}
`)
	if err != nil {
		log.Fatal(err)
	}

	// =========================================
	// Render examples
	// =========================================

	fmt.Println("=== Template Inheritance ===")
	tmpl, _ := env.GetTemplate("page.html")
	result, err := tmpl.Render(map[string]any{
		"page_title": "Welcome",
		"content":    "This is the welcome page content.",
	})
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(result)

	fmt.Println("\n=== Macros ===")
	tmpl2, _ := env.GetTemplate("form.html")
	result2, err := tmpl2.Render(map[string]any{
		"users": []map[string]any{
			{"name": "Alice", "email": "alice@example.com", "admin": true},
			{"name": "Bob", "email": "bob@example.com", "admin": false},
		},
	})
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(result2)
}
