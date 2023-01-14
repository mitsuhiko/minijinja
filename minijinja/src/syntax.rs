//! Documents the syntax for templates.
//!
//! <details><summary><strong style="cursor: pointer">Table of Contents</strong></summary>
//!
//! - [Synopsis](#synopsis)
//! - [Trailing Newlines](#trailing-newlines)
//! - [Expressions](#expressions)
//!   - [Literals](#literals)
//!   - [Math](#math)
//!   - [Comparisons](#comparisons)
//!   - [Logic](#logic)
//!   - [Other Operators](#other-operators)
//!   - [If Expressions](#if-expressions)
//! - [Tags](#tags)
//!   - [`{% for %}`](#-for-)
//!   - [`{% if %}`](#-if-)
//!   - [`{% extends %}`](#-extends-)
//!   - [`{% block %}`](#-block-)
//!   - [`{% include %}`](#-include-)
//!   - [`{% import %}`](#-import-)
//!   - [`{% with %}`](#-with-)
//!   - [`{% set %}`](#-set-)
//!   - [`{% filter %}`](#-filter-)
//!   - [`{% macro %}`](#-macro-)
//!   - [`{% call %}`](#-call-)
//!   - [`{% do %}`](#-do-)
//!   - [`{% autoescape %}`](#-autoescape-)
//!   - [`{% raw %}`](#-raw-)
//!
//! </details>
//!
//! # Synopsis
//!
//! A MiniJinja template is simply a text file.  MiniJinja can generate any text-based
//! format (HTML, XML, CSV, LaTeX, etc.).  A template doesn’t need to have a specific extension
//! and in fact MiniJinja does not understand much about the file system.  However the default
//! configuration for [auto escaping](crate::Environment::set_auto_escape_callback) uses file
//! extensions to configure the initial behavior.
//!
//! A template contains [**expressions**](#expressions), which get replaced with values when a
//! template is rendered; and [**tags**](#tags), which control the logic of the template.  The
//! template syntax is heavily inspired by Jinja2, Django and Python.
//!
//! This is a minimal template that illustrates a few basics:
//!
//! ```jinja
//! <!doctype html>
//! <title>{% block title %}My Website{% endblock %}</title>
//! <ul id="navigation">
//! {% for item in navigation %}
//!     <li><a href="{{ item.href }}">{{ item.caption }}</a></li>
//! {% endfor %}
//! </ul>
//!
//! <h1>My Webpage</h1>
//! {% block body %}{% endblock %}
//!
//! {# a comment #}
//! ```
//!
//! # Trailing Newlines
//!
//! MiniJinja, like Jinja2, will remove one trailing newline from the end of the file automatically
//! on parsing.  This lets templates produce a consistent output no matter if the editor adds a
//! trailing newline or not.  If one wants a trailing newline an extra newline can be added or the
//! code rendering it adds it manually.
//!
//! # Expressions
//!
//! MiniJinja allows basic expressions everywhere. These work largely as you expect from Jinja2.
//! Even if you have not used Jinja2 you should feel comfortable with it.  To output the result
//! of an expression wrap it in `{{ .. }}`.
//!
//! ## Literals
//!
//! The simplest form of expressions are literals. Literals are representations for
//! objects such as strings and numbers. The following literals exist:
//!
//! - `"Hello World"`: Everything between two double or single quotes is a string. They are
//!   useful whenever you need a string in the template (e.g. as arguments to function calls
//!   and filters, or just to extend or include a template).
//! - `42`: Integers are whole numbers without a decimal part.
//! - `42.0`: Floating point numbers can be written using a `.` as a decimal mark.
//! - `['list', 'of', 'objects']`: Everything between two brackets is a list. Lists are useful
//!   for storing sequential data to be iterated over.
//!   for compatibility with Jinja2 `('list', 'of', 'objects')` is also allowed.
//! - `{'map': 'of', 'key': 'and', 'value': 'pairs'}`: A map is a structure that combines keys
//!   and values. Keys must be unique and always have exactly one value. Maps are rarely
//!   created in templates.
//! - `true` / `false` / `none`: boolean values and the special `none` value which maps to the
//!   unit type in Rust.
//!
//! ## Math
//!
//! MiniJinja allows you to calculate with values.  The following operators are supported:
//!
//! - ``+``: Adds two numbers up. ``{{ 1 + 1 }}`` is ``2``.
//! - ``-``: Subtract the second number from the first one.  ``{{ 3 - 2 }}`` is ``1``.
//! - ``/``: Divide two numbers. ``{{ 1 / 2 }}`` is ``0.5``.  See note on divisions below.
//! - ``//``: Integer divide two numbers. ``{{ 5 // 3 }}`` is ``1``.  See note on divisions below.
//! - ``%``: Calculate the remainder of an integer division.  ``{{ 11 % 7 }}`` is ``4``.
//! - ``*``: Multiply the left operand with the right one.  ``{{ 2 * 2 }}`` would return ``4``.
//! - ``**``: Raise the left operand to the power of the right operand.  ``{{ 2**3 }}``
//!   would return ``8``.
//!
//! Note on divisions: divisions in Jinja2 are flooring, divisions in MiniJinja
//! are at present using euclidean division.  They are almost the same but not quite.
//!
//! ## Comparisons
//!  
//! - ``==``: Compares two objects for equality.
//! - ``!=``: Compares two objects for inequality.
//! - ``>``: ``true`` if the left hand side is greater than the right hand side.
//! - ``>=``: ``true`` if the left hand side is greater or equal to the right hand side.
//! - ``<``:``true`` if the left hand side is less than the right hand side.
//! - ``<=``: ``true`` if the left hand side is less or equal to the right hand side.
//!
//! ## Logic
//!
//! For ``if`` statements it can be useful to combine multiple expressions:
//!
//! - ``and``: Return true if the left and the right operand are true.
//! - ``or``: Return true if the left or the right operand are true.
//! - ``not``: negate a statement (see below).
//! - ``(expr)``: Parentheses group an expression.
//!
//! ## Other Operators
//!
//! The following operators are very useful but don't fit into any of the other
//! two categories:
//!
//! - ``is``/``is not``: Performs a [test](crate::tests).
//! - ``in``/``not in``: Performs a containment check.
//! - ``|`` (pipe, vertical bar): Applies a [filter](crate::filters).
//! - ``~`` (tilde): Converts all operands into strings and concatenates them.
//!   ``{{ "Hello " ~ name ~ "!" }}`` would return (assuming `name` is set
//!   to ``'John'``) ``Hello John!``.
//! - ``()``: Call a callable: ``{{ super() }}``.  Inside of the parentheses you
//!   can use positional arguments.  Additionally keyword arguments are supported
//!   which are treated like a dict syntax.  Eg: `foo(a=1, b=2)` is the same as
//!   `foo({"a": 1, "b": 2})`.
//! - ``.`` / ``[]``: Get an attribute of an object.
//! - ``[start:stop]`` / ``[start:stop:step]``: slices a list or string.  All three expressions
//!   are optional (`start`, `stop`, `step`).  For instance ``"Hello World"[:5]`` will return
//!   just `"Hello"`.  Likewise ``"Hello"[1:-1]`` will return `"ell"`.  The step component can
//!   be used to change the step size.  `"12345"[::2]` will return `"135"`.
//!
//! ### If Expressions
//!
//! It is also possible to use inline _if_ expressions. These are useful in some situations.
//! For example, you can use this to extend from one template if a variable is defined,
//! otherwise from the default layout template:
//!
//! ```jinja
//! {% extends layout_template if layout_template is defined else 'default.html' %}
//! ```
//!
//! The `else` part is optional. If not provided, the else block implicitly evaluates
//! into an undefined value:
//!
//! ```jinja
//! {{ title|upper if title }}
//! ```
//!
//! # Tags
//!
//! Tags control logic in templates.  The following tags exist:
//!
//! ## `{% for %}`
//!
//! The for tag loops over each item in a sequence.  For example, to display a list
//! of users provided in a variable called `users`:
//!
//! ```jinja
//! <h1>Members</h1>
//! <ul>
//! {% for user in users %}
//!   <li>{{ user.username }}</li>
//! {% endfor %}
//! </ul>
//! ```
//!
//! It's also possible to unpack tuples while iterating:
//!
//! ```jinja
//! <h1>Members</h1>
//! <ul>
//! {% for (key, value) in list_of_pairs %}
//!   <li>{{ key }}: {{ value }}</li>
//! {% endfor %}
//! </ul>
//! ```
//!
//! Inside of the for block you can access some special variables:
//!
//! - `loop.index`: The current iteration of the loop. (1 indexed)
//! - `loop.index0`: The current iteration of the loop. (0 indexed)
//! - `loop.revindex`: The number of iterations from the end of the loop (1 indexed)
//! - `loop.revindex0`: The number of iterations from the end of the loop (0 indexed)
//! - `loop.first`: True if this is the first iteration.
//! - `loop.last`: True if this is the last iteration.
//! - `loop.length`: The number of items in the sequence.
//! - `loop.cycle`: A helper function to cycle between a list of sequences. See the explanation below.
//! - `loop.depth`: Indicates how deep in a recursive loop the rendering currently is. Starts at level 1
//! - `loop.depth0`: Indicates how deep in a recursive loop the rendering currently is. Starts at level 0
//! - `loop.previtem`: The item from the previous iteration of the loop. `Undefined` during the first iteration.
//! - `loop.nextitem`: The item from the previous iteration of the loop. `Undefined` during the last iteration.
//! - `loop.changed(...args)`: Returns true if the passed values have changed since the last time it was called with the same arguments.
//! - `loop.cycle(...args)`: Returns a value from the passed sequence in a cycle.
//!
//! Within a for-loop, it’s possible to cycle among a list of strings/variables each time through
//! the loop by using the special `loop.cycle` helper:
//!
//! ```jinja
//! {% for row in rows %}
//!   <li class="{{ loop.cycle('odd', 'even') }}">{{ row }}</li>
//! {% endfor %}
//! ```
//!
//! A `loop.changed()` helper is also available which can be used to detect when
//! a value changes between the last iteration and the current one.  The method
//! takes one or more arguments that are all compared.
//!
//! ```jinja
//! {% for entry in entries %}
//!   {% if loop.changed(entry.category) %}
//!     <h2>{{ entry.category }}</h2>
//!   {% endif %}
//!   <p>{{ entry.message }}</p>
//! {% endfor %}
//! ```
//!
//! Unlike in Rust or Python, it’s not possible to break or continue in a loop. You can,
//! however, filter the sequence during iteration, which allows you to skip items.  The
//! following example skips all the users which are hidden:
//!
//! ```jinja
//! {% for user in users if not user.hidden %}
//!   <li>{{ user.username }}</li>
//! {% endfor %}
//! ```
//!
//! If no iteration took place because the sequence was empty or the filtering
//! removed all the items from the sequence, you can render a default block by
//! using else:
//!
//! ```jinja
//! <ul>
//! {% for user in users %}
//!   <li>{{ user.username }}</li>
//! {% else %}
//!   <li><em>no users found</em></li>
//! {% endfor %}
//! </ul>
//! ```
//!
//! It is also possible to use loops recursively. This is useful if you are
//! dealing with recursive data such as sitemaps. To use loops recursively, you
//! basically have to add the ``recursive`` modifier to the loop definition and
//! call the loop variable with the new iterable where you want to recurse.
//!
//! ```jinja
//! <ul class="menu">
//! {% for item in menu recursive %}
//!   <li><a href="{{ item.href }}">{{ item.title }}</a>
//!   {% if item.children %}
//!     <ul class="submenu">{{ loop(item.children) }}</ul>
//!   {% endif %}</li>
//! {% endfor %}
//! </ul>
//! ```
//!
//! **Special note:** the `previtem` and `nextitem` attributes are available by default
//! but can be disabled by removing the `adjacent-loop-items` crate feature.  Removing
//! these attributes can provide meaningful speedups for templates with a lot of loops.
//!
//! ## `{% if %}`
//!
//! The `if` statement is comparable with the Python if statement. In the simplest form,
//! you can use it to test if a variable is defined, not empty and not false:
//!
//! ```jinja
//! {% if users %}
//!   <ul>
//!   {% for user in users %}
//!     <li>{{ user.username }}</li>
//!   {% endfor %}
//!   </ul>
//! {% endif %}
//! ```
//!
//! For multiple branches, `elif` and `else` can be used like in Python.  You can use more
//! complex expressions there too:
//!
//! ```jinja
//! {% if kenny.sick %}
//!   Kenny is sick.
//! {% elif kenny.dead %}
//!   You killed Kenny!  You bastard!!!
//! {% else %}
//!   Kenny looks okay --- so far
//! {% endif %}
//! ```
//!
//! ## `{% extends %}`
//!
//! **Feature:** `multi-template` (included by default)
//!
//! The `extends` tag can be used to extend one template from another.  You can have multiple
//! `extends` tags in a file, but only one of them may be executed at a time.  For more
//! information see [block](#-block-).
//!
//! ## `{% block %}`
//!
//! Blocks are used for inheritance and act as both placeholders and replacements at the
//! same time:
//!
//! The most powerful part of MiniJinja is template inheritance. Template inheritance allows
//! you to build a base "skeleton" template that contains all the common elements of your
//! site and defines **blocks** that child templates can override.
//!
//! **Base Template:**
//!
//! This template, which we'll call ``base.html``, defines a simple HTML skeleton
//! document that you might use for a simple two-column page. It's the job of
//! "child" templates to fill the empty blocks with content:
//!
//! ```jinja
//! <!doctype html>
//! {% block head %}
//! <title>{% block title %}{% endblock %}</title>
//! {% endblock %}
//! {% block body %}{% endblock %}
//! ```
//!
//! **Child Template:**
//!
//! ```jinja
//! {% extends "base.html" %}
//! {% block title %}Index{% endblock %}
//! {% block head %}
//!   {{ super() }}
//!   <style type="text/css">
//!     .important { color: #336699; }
//!   </style>
//! {% endblock %}
//! {% block body %}
//!   <h1>Index</h1>
//!   <p class="important">
//!     Welcome to my awesome homepage.
//!   </p>
//! {% endblock %}
//! ```
//!
//! The ``{% extends %}`` tag is the key here. It tells the template engine that
//! this template "extends" another template.  When the template system evaluates
//! this template, it first locates the parent.  The extends tag should be the
//! first tag in the template.
//!
//! As you can see it's also possible to render the contents of the parent block by calling
//! ``super()``. You can’t define multiple ``{% block %}`` tags with the same name in
//! the same template. This limitation exists because a block tag works in “both”
//! directions. That is, a block tag doesn’t just provide a placeholder to fill -
//! it also defines the content that fills the placeholder in the parent. If
//! there were two similarly-named ``{% block %}`` tags in a template, that
//! template’s parent wouldn’t know which one of the blocks’ content to use.
//!
//! If you want to print a block multiple times, you can, however, use the
//! special self variable and call the block with that name:
//!
//! ```jinja
//! <title>{% block title %}{% endblock %}</title>
//! <h1>{{ self.title() }}</h1>
//! {% block body %}{% endblock %}
//! ```
//!
//! MiniJinja allows you to put the name of the block after the end tag for better
//! readability:
//!
//! ```jinja
//! {% block sidebar %}
//!   {% block inner_sidebar %}
//!     ...
//!   {% endblock inner_sidebar %}
//! {% endblock sidebar %}
//! ```
//!
//! However, the name after the `endblock` word must match the block name.
//!
//! ## `{% include %}`
//!
//! **Feature:** `multi-template` (included by default)
//!  
//! The `include` tag is useful to include a template and return the rendered contents of that file
//! into the current namespace::
//!  
//! ```jinja
//! {% include 'header.html' %}
//!   Body
//! {% include 'footer.html' %}
//! ```
//!
//! Optionally `ignore missing` can be added in which case non existing templates
//! are silently ignored.
//!
//! ```jinja
//! {% include 'customization.html' ignore missing %}
//! ```
//!
//! You can also provide a list of templates that are checked for existence
//! before inclusion. The first template that exists will be included. If `ignore
//! missing` is set, it will fall back to rendering nothing if none of the
//! templates exist, otherwise it will fail with an error.
//!
//! ```jinja
//! {% include ['page_detailed.html', 'page.html'] %}
//! {% include ['special_sidebar.html', 'sidebar.html'] ignore missing %}
//! ```
//!  
//! Included templates have access to the variables of the active context.
//!
//! ## `{% import %}`
//!
//! **Feature:** `multi-template` (included by default)
//!
//! MiniJinja supports the `{% import %}` and `{% from ... import ... %}`
//! syntax.  With it variables or macros can be included from other templates:
//!
//! ```jinja
//! {% from "my_template.html" import my_macro, my_variable %}
//! ```
//!
//! Imports can also be aliased:
//!
//! ```jinja
//! {% from "my_template.html" import my_macro as other_name %}
//! {{ other_name() }}
//! ```
//!
//! Full modules can be imported with `{% import ... as ... %}`:
//!
//! ```jinja
//! {% import "my_template.html" as helpers %}
//! {{ helpers.my_macro() }}
//! ```
//!
//! Note that unlike Jinja2, exported modules do not contain any template code.  Only
//! variables and macros that are defined can be imported.  Also imports unlike in Jinja2
//! are not cached and they get access to the full template context.
//!
//! ## `{% with %}`
//!
//! The with statement makes it possible to create a new inner scope.  Variables set within
//! this scope are not visible outside of the scope:
//!
//! ```jinja
//! {% with foo = 42 %}
//!   {{ foo }}           foo is 42 here
//! {% endwith %}
//! foo is not visible here any longer
//! ```
//!
//! Multiple variables can be set at once and unpacking is supported:
//!
//! ```jinja
//! {% with a = 1, (b, c) = [2, 3] %}
//!   {{ a }}, {{ b }}, {{ c }}  (outputs 1, 2, 3)
//! {% endwith %}
//! ```
//!
//! ## `{% set %}`
//!
//! The `set` statement can be used to assign to variables on the same scope.  This is
//! similar to how `with` works but it won't introduce a new scope.
//!
//! ```jinja
//! {% set navigation = [('index.html', 'Index'), ('about.html', 'About')] %}
//! ```
//!
//! Please keep in mind that it is not possible to set variables inside a block
//! and have them show up outside of it.  This also applies to loops.  The only
//! exception to that rule are if statements which do not introduce a scope.
//!
//! It's also possible to capture blocks of template code into a variable by using
//! the `set` statement as a block.   In that case, instead of using an equals sign
//! and a value, you just write the variable name and then everything until
//! `{% endset %}` is captured.
//!
//! ```jinja
//! {% set navigation %}
//! <li><a href="/">Index</a>
//! <li><a href="/downloads">Downloads</a>
//! {% endset %}
//! ```
//!
//! The `navigation` variable then contains the navigation HTML source.
//!
//! This can also be combined with applying a filter:
//!
//! ```jinja
//! {% set title | upper %}Title of the page{% endset %}
//! ```
//!
//! ## `{% filter %}`
//!
//! Filter sections allow you to apply regular [filters](crate::filters) on a
//! block of template data. Just wrap the code in the special filter block:
//!
//! ```jinja
//! {% filter upper %}
//!   This text becomes uppercase
//! {% endfilter %}
//! ```
//!
//! ## `{% macro %}`
//!
//! **Feature:** `macros` (included by default)
//!
//! MiniJinja has limited support for macros.  They allow you to write reusable
//! template functions.  They are useful to put often used idioms into reusable
//! functions so you don't repeat yourself (“DRY”).
//!
//! Here’s a small example of a macro that renders a form element:
//!
//! ```jinja
//! {% macro input(name, value="", type="text") -%}
//! <input type="{{ type }}" name="{{ name }}" value="{{ value }}">
//! {%- endmacro %}
//! ```
//!
//! The macro can then be called like a function in the namespace:
//!
//! ```jinja
//! <p>{{ input('username') }}</p>
//! <p>{{ input('password', type='password') }}</p>
//! ```
//!
//! The behavior of macros with regards to undefined variables is that they capture
//! them at macro declaration time (eg: they use a closure).
//!
//! Macros can be imported via `{% import %}` or `{% from ... import %}`.
//!
//! Macros also accept a hidden `caller` keyword argument for the use with
//! `{% call %}`.
//!
//! ## `{% call %}`
//!
//! **Feature:** `macros` (included by default)
//!
//! This tag functions similar to a macro that is passed to another macro.  You can
//! think of it as a way to declare an anonymous macro and pass it to another macro
//! with the `caller` keyword argument.  The following example shows a macro that
//! takes advantage of the call functionality and how it can be used:
//!
//! ```jinja
//! {% macro dialog(title) %}
//!   <div class="dialog">
//!     <h3>{{ title }}</h3>
//!     <div class="contents">{{ caller() }}</div>
//!   </div>
//! {% endmacro %}
//!
//! {% call dialog(title="Hello World") %}
//!   This is the dialog body.
//! {% endcall %}
//! ```
//!
//! <details><summary><strong style="cursor: pointer">Macro Alternative:</strong></summary>
//!
//! The above example is more or less equivalent with the following:
//!
//! ```jinja
//! {% macro dialog(title) %}
//!   <div class="dialog">
//!     <h3>{{ title }}</h3>
//!     <div class="contents">{{ caller() }}</div>
//!   </div>
//! {% endmacro %}
//!
//! {% macro caller() %}
//!   This is the dialog body.
//! {% endmacro %}
//! {{ dialog(title="Hello World", caller=caller) }}
//! ```
//!
//! </details>
//!
//! It’s also possible to pass arguments back to the call block.  This makes it
//! useful to build macros that behave like if statements or loops.  Arguments
//! are placed surrounded in parentheses right after the `call` keyword and
//! is followed by the macro to be called.
//!
//! ```jinja
//! {% macro render_user_list(users) %}
//! <ul>
//! {% for user in users %}
//!   <li><p>{{ user.username }}</p>{{ caller(user) }}</li>
//! {% endfor %}
//! </ul>
//! {% endmacro %}
//!
//! {% call(user) render_user_list(list_of_user) %}
//! <dl>
//!   <dt>Name</dt>
//!   <dd>{{ user.name }}</dd>
//!   <dt>E-Mail</dt>
//!   <dd>{{ user.email }}</dd>
//! </dl>
//! {% endcall %}
//! ```
//!
//! ## `{% do %}`
//!
//! The do tag has the same functionality as regular template tags (`{{ ... }}`);
//! except it doesn't output anything when called.
//!
//! This is useful if you have a function or macro that has a side-effect, and
//! you don’t want to display output in the template. The following example
//! shows a macro that uses the do functionality, and how it can be used:
//!
//! ```jinja
//! {% macro dialog(title) %}
//!   Dialog is {{ title }}
//! {% endmacro %}
//!
//! {% do dialog(title="Hello World") %} <- does not output anything
//! ```
//!
//! The above example will not output anything when using the `do` tag.
//!
//! This tag exists for consistency with Jinja2 and can be useful if you have
//! custom functionality in templates that uses side-effects.  For instance if
//! you expose a function to templates that can be used to log warnings:
//!
//! ```jinja
//! {% for user in users %}
//!   {% if user.deleted %}
//!     {% log warn("Found unexpected deleted user in template") %}
//!   {% endif %}
//!   ...
//! {% endfor %}
//! ```
//!
//! ## `{% autoescape %}`
//!
//! If you want you can activate and deactivate the autoescaping from within
//! the templates.
//!
//! Example:
//!
//! ```jinja
//! {% autoescape true %}
//!   Autoescaping is active within this block
//! {% endautoescape %}
//!
//! {% autoescape false %}
//!   Autoescaping is inactive within this block
//! {% endautoescape %}
//! ```
//!
//! After an `endautoescape` the behavior is reverted to what it was before.
//!
//! The exact auto escaping behavior is determined by the value of
//! [`AutoEscape`](crate::AutoEscape) set to the template.
//!
//! ## `{% raw %}`
//!
//! A raw block is a special construct that lets you ignore the embedded template
//! syntax.  This is particularly useful if a segment of template code would
//! otherwise require constant escaping with things like `{{ "{{" }}`:
//!
//! Example:
//!
//! ```jinja
//! {% raw %}
//! <ul>
//! {% for item in seq %}
//!     <li>{{ item }}</li>
//! {% endfor %}
//! </ul>
//! {% endraw %}
//! ```

// this is just for docs
