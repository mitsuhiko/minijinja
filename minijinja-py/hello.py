from minijinja_py import Environment

INDEX = """{% extends "layout.html" %}
{% block title %}{{ page.title }}{% endblock %}
{% block body %}
  <ul>
  {%- for item in items %}
    <li>{{ item }}
  {%- endfor %}
  </ul>
{% endblock %}
"""
LAYOUT = """<!doctype html>
<title>{% block title %}{% endblock %}</title>
<body>
  {% block body %}{% endblock %}
</body>
"""

env = Environment()
env.set_loader({
    "index.html": INDEX,
    "layout.html": LAYOUT,
}.get)

print(env.render_template(
    'index.html',
    page={"title": "The Page Title"},
    items=["Peter", "Paul", "Mary"]
))
