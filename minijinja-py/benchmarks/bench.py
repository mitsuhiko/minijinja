"""Micro benchmarks for the Python <-> Rust bridge.

Run with a release build of the extension module:

    maturin develop --release
    python benchmarks/bench.py [filter]

If jinja2 is installed, the same templates are rendered with Jinja2 for
reference.
"""

import sys
import timeit

from minijinja import Environment

try:
    import jinja2
except ImportError:
    jinja2 = None


class User:
    def __init__(self, i):
        self.name = f"user-{i}"
        self.email = f"user{i}@example.com"
        self.active = i % 3 != 0
        self.score = i * 1.5


ROWS = [
    {
        "id": i,
        "name": f"item-{i}",
        "price": i * 1.25,
        "tags": ["a", "b", "c"],
        "in_stock": i % 2 == 0,
    }
    for i in range(500)
]
USERS = [User(i) for i in range(500)]
NUMBERS = list(range(1000))


def shout(value):
    return str(value).upper()


CASES = [
    (
        "hello",
        "Hello {{ name }}!",
        lambda: {"name": "World"},
    ),
    (
        "small_ctx",
        "{{ title }}: {{ count }} {{ ratio }} {% if flag %}yes{% endif %}",
        lambda: {"title": "Report", "count": 42, "ratio": 0.5, "flag": True},
    ),
    (
        "dict_rows",
        "{% for row in rows %}{{ row.id }} {{ row.name }} {{ row.price }}"
        "{% if row.in_stock %}!{% endif %}{% for t in row.tags %}{{ t }}{% endfor %}\n"
        "{% endfor %}",
        lambda: {"rows": ROWS},
    ),
    (
        "obj_attrs",
        "{% for u in users %}{% if u.active %}{{ u.name }} <{{ u.email }}> "
        "{{ u.score }}\n{% endif %}{% endfor %}",
        lambda: {"users": USERS},
    ),
    (
        "int_list",
        "{% for n in numbers %}{{ n }},{% endfor %}",
        lambda: {"numbers": NUMBERS},
    ),
    (
        "int_list_sum",
        "{{ numbers|sum }} {{ numbers|length }} {{ numbers|max }}",
        lambda: {"numbers": NUMBERS},
    ),
    (
        "py_filter",
        "{% for row in rows %}{{ row.name|shout }}{% endfor %}",
        lambda: {"rows": ROWS},
    ),
    (
        "py_function",
        "{% for row in rows %}{{ shout(row.name) }}{% endfor %}",
        lambda: {"rows": ROWS, "shout": shout},
    ),
    (
        "native_loop",
        "{% for n in range(1000) %}{{ n }}{% endfor %}",
        lambda: {},
    ),
]


def bench(fn, min_time=0.3):
    timer = timeit.Timer(fn)
    number, _ = timer.autorange()
    number = max(1, int(number * min_time / 0.2))
    best = min(timer.repeat(repeat=5, number=number))
    return best / number


def fmt(t):
    if t < 1e-3:
        return f"{t * 1e6:9.2f} us"
    return f"{t * 1e3:9.2f} ms"


def main():
    selector = sys.argv[1] if len(sys.argv) > 1 else ""
    env = Environment(filters={"shout": shout})
    if jinja2 is not None:
        jenv = jinja2.Environment()
        jenv.filters["shout"] = shout
        jenv.globals["range"] = range

    print(f"{'case':<14} {'minijinja':>12} {'jinja2':>12}")
    for name, source, make_ctx in CASES:
        if selector not in name:
            continue
        ctx = make_ctx()
        env.add_template(name, source)
        rv = env.render_template(name, **ctx)
        mj = bench(lambda: env.render_template(name, **ctx))
        line = f"{name:<14} {fmt(mj)}"
        if jinja2 is not None:
            jtmpl = jenv.from_string(source)
            assert jtmpl.render(**ctx).strip() != "" or rv.strip() == ""
            j = bench(lambda: jtmpl.render(**ctx))
            line += f" {fmt(j)}  ({j / mj:.2f}x)"
        print(line)


if __name__ == "__main__":
    main()
