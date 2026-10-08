import { DEFAULT_CONFIG } from "./defaultConfig";
import { DEFAULT_SETTINGS, type PlaygroundState } from "./state";

export interface Example {
  id: string;
  title: string;
  state: PlaygroundState;
}

function json(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

export const EXAMPLES: Example[] = [
  {
    id: "hello",
    title: "Hello World",
    state: {
      files: [
        {
          name: "index.html",
          source: `<nav>
  <ul>
    {%- for item in nav %}
    <li><a href="{{ item.href }}">{{ item.title }}</a></li>
    {%- endfor %}
  </ul>
</nav>
<main>
  Hello {{ name }}!
</main>
`,
        },
      ],
      entry: "index.html",
      context: json({
        name: "World",
        nav: [
          { href: "/", title: "Index" },
          { href: "/help", title: "Help" },
          { href: "/about", title: "About" },
        ],
      }),
      config: DEFAULT_CONFIG,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "inheritance",
    title: "Template Inheritance",
    state: {
      files: [
        {
          name: "index.html",
          source: `{% extends "layout.html" %}

{% block title %}{{ page.title }}{% endblock %}

{% block body %}
  <h1>{{ page.title }}</h1>
  {{ super() }}
  <p>{{ page.body }}</p>
{% endblock %}
`,
        },
        {
          name: "layout.html",
          source: `<!doctype html>
<html>
  <head>
    <title>{% block title %}{% endblock %} | {{ site_name }}</title>
  </head>
  <body>
    {% block body %}
    <p>This paragraph comes from the layout.</p>
    {% endblock %}
  </body>
</html>
`,
        },
      ],
      entry: "index.html",
      context: json({
        site_name: "My Site",
        page: {
          title: "Welcome",
          body: "Inheritance lets templates share a layout.",
        },
      }),
      config: DEFAULT_CONFIG,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "macros",
    title: "Macros and Imports",
    state: {
      files: [
        {
          name: "index.html",
          source: `{% from "forms.html" import input, button %}
<form method="post">
  {{ input("username", label="Username") }}
  {{ input("password", type="password", label="Password") }}
  {{ button("Sign in") }}
</form>
`,
        },
        {
          name: "forms.html",
          source: `{% macro input(name, type="text", label=none) -%}
  <label>
    {%- if label %}{{ label }}: {% endif -%}
    <input type="{{ type }}" name="{{ name }}">
  </label>
{%- endmacro %}

{% macro button(text) -%}
  <button type="submit">{{ text }}</button>
{%- endmacro %}
`,
        },
      ],
      entry: "index.html",
      context: json({}),
      config: DEFAULT_CONFIG,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "filters",
    title: "Loops and Filters",
    state: {
      files: [
        {
          name: "report.txt",
          source: `Report for {{ team|title }}
{{ "=" * (11 + team|length) }}

{% for group in members|groupby("role") -%}
{{ group.grouper|upper }}:
{% for member in group.list|sort(attribute="name") -%}
  {{ loop.index }}. {{ member.name }} ({{ member.commits }} commits)
{% endfor %}
{% endfor -%}
Total commits: {{ members|map(attribute="commits")|sum }}
Most active: {{ (members|sort(attribute="commits", reverse=true)|first).name }}
`,
        },
      ],
      entry: "report.txt",
      context: json({
        team: "platform team",
        members: [
          { name: "Peter", role: "engineer", commits: 42 },
          { name: "Anna", role: "designer", commits: 7 },
          { name: "Jane", role: "engineer", commits: 108 },
          { name: "Bob", role: "designer", commits: 12 },
        ],
      }),
      config: DEFAULT_CONFIG,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "whitespace",
    title: "Whitespace Control",
    state: {
      files: [
        {
          name: "config.txt",
          source: `# Toggle trim blocks and lstrip blocks in the settings
# and enable "Show whitespace" in the output.
{% for server in servers %}
  {% if server.enabled %}
server {{ server.name }} {
    listen {{ server.port }};
}
  {% endif %}
{% endfor %}
`,
        },
      ],
      entry: "config.txt",
      context: json({
        servers: [
          { name: "alpha", port: 8080, enabled: true },
          { name: "beta", port: 8081, enabled: false },
          { name: "gamma", port: 8082, enabled: true },
        ],
      }),
      config: DEFAULT_CONFIG,
      settings: { ...DEFAULT_SETTINGS, trimBlocks: true, lstripBlocks: true },
    },
  },
  {
    id: "pycompat",
    title: "Python Compatibility",
    state: {
      files: [
        {
          name: "index.txt",
          source: `{# Python methods are available with pycompat enabled #}
{% for key, value in config.items() -%}
{{ key.upper() }} = {{ value }}
{% endfor -%}
{{ "hello world".title().replace("World", "MiniJinja") }}
{{ names|join(", ") if "Anna" in names else "nobody" }}
`,
        },
      ],
      entry: "index.txt",
      context: json({
        config: { debug: true, workers: 4, host: "localhost" },
        names: ["Anna", "Peter"],
      }),
      config: DEFAULT_CONFIG,
      settings: { ...DEFAULT_SETTINGS, pycompat: true },
    },
  },
  {
    id: "json",
    title: "JSON Output",
    state: {
      files: [
        {
          name: "data.json",
          source: `{# .json templates escape values as JSON #}
{
  "user": {{ user.name }},
  "tags": {{ user.tags }},
  "summary": {{ "%s has %d tags"|format(user.name, user.tags|length) }}
}
`,
        },
      ],
      entry: "data.json",
      context: json({
        user: { name: 'Peter "Pete" Parker', tags: ["admin", "dev"] },
      }),
      config: DEFAULT_CONFIG,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "custom-filters",
    title: "Custom Filters",
    state: {
      files: [
        {
          name: "index.html",
          source: `{# The filters, tests and functions are defined in the Config tab -#}
<h1>{{ title|slugify }}</h1>
<ul>
{%- for item in items %}
  <li>{{ item.name }}: {{ item.price|currency(code="EUR") }}
    {%- if item.price is expensive %} (expensive){% endif %}</li>
{%- endfor %}
</ul>
{{ badge("new") }}
<p>{{ greet("World") }}</p>
`,
        },
      ],
      entry: "index.html",
      context: json({
        title: "Hello Custom Filters!",
        greeting: "Servus",
        items: [
          { name: "Coffee", price: 3.5 },
          { name: "Espresso machine", price: 899 },
        ],
      }),
      config: `// The config runs before every render with the environment as \`env\`.
// Also available: passState, markSafe, SafeString and TemplateError.

env.addFilter("slugify", (value) =>
  String(value).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""));

// keyword arguments are passed as trailing object
env.addFilter("currency", (value, { code = "USD" } = {}) =>
  new Intl.NumberFormat("en-US", { style: "currency", currency: code }).format(value));

env.addTest("expensive", (value) => value > 100);

// markSafe stops the output from being escaped
env.addFunction("badge", (text) =>
  markSafe(\`<span class="badge">\${text}</span>\`));

// passState gives access to the template state, here to look up a variable
env.addFunction("greet", passState((state, name) =>
  \`\${state.lookup("greeting") ?? "Hello"} \${name}!\`));
`,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "datetime",
    title: "Dates and Times",
    state: {
      files: [
        {
          name: "events.txt",
          source: `{# Formats default to DATETIME_FORMAT, DATE_FORMAT and TIME_FORMAT,
   the timezone to TIMEZONE (all set in the context). -#}
Generated {{ now()|datetimeformat(format="long") }} ({{ TIMEZONE }})

{% for event in events -%}
* {{ event.title }}
  {{ event.start|dateformat(format="full") }} at {{ event.start|timeformat }}
  In New York: {{ event.start|datetimeformat(tz="America/New_York") }}
  Custom: {{ event.start|datetimeformat(format="%a %-d.%-m. %I:%M %p %Z") }}
{% endfor %}
`,
        },
      ],
      entry: "events.txt",
      context: json({
        TIMEZONE: "Europe/Vienna",
        TIME_FORMAT: "short",
        events: [
          { title: "Release party", start: "2025-06-24T16:30:00Z" },
          { title: "Planning", start: 1767261600 },
          { title: "Retro", start: "2026-03-29T09:00:00[Europe/London]" },
        ],
      }),
      config: DEFAULT_CONFIG,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "random",
    title: "Random Values",
    state: {
      files: [
        {
          name: "index.html",
          source: `{# Set RAND_SEED in the context for repeatable output -#}
<h1>{{ ["Hello", "Hi", "Welcome"]|random }}, {{ name }}!</h1>
<p>Your lucky number is {{ randrange(1, 100) }}.</p>
{{ lipsum(2, html=true) }}
`,
        },
      ],
      entry: "index.html",
      context: json({ name: "World" }),
      config: DEFAULT_CONFIG,
      settings: DEFAULT_SETTINGS,
    },
  },
  {
    id: "line-statements",
    title: "Custom Syntax",
    state: {
      files: [
        {
          name: "Makefile",
          source: `## Line statements and comments are enabled in the settings
# for target in targets
\${ target.name }:
\t\${ target.command }

# endfor
`,
        },
      ],
      entry: "Makefile",
      context: json({
        targets: [
          { name: "build", command: "cargo build" },
          { name: "test", command: "cargo test" },
        ],
      }),
      config: DEFAULT_CONFIG,
      settings: {
        ...DEFAULT_SETTINGS,
        trimBlocks: true,
        syntax: {
          variableStart: "${",
          variableEnd: "}",
          lineStatementPrefix: "#",
          lineCommentPrefix: "##",
        },
      },
    },
  },
];

export const DEFAULT_STATE = EXAMPLES[0].state;
