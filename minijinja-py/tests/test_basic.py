from _pytest.unraisableexception import catch_unraisable_exception
from minijinja import Environment, TemplateError, safe


def test_expression():
    env = Environment()
    rv = env.eval_expr("1 + b", b=42)
    assert rv == 43
    rv = env.eval_expr("range(n)", n=10)
    assert rv == list(range(10))


def test_pass_callable():
    def magic():
        return [1, 2, 3]

    env = Environment()
    rv = env.eval_expr("x()", x=magic)
    assert rv == [1, 2, 3]


def test_callable_attrs():
    def hmm():
        pass

    hmm.public_attr = 42
    env = Environment()
    rv = env.eval_expr("[hmm.public_attr, hmm.__module__]", hmm=hmm)
    assert rv == [42, None]


def test_generator():
    def hmm():
        # this implicity gets converted into a list. it's not a real iterator
        yield 1
        yield 2
        yield 3

    hmm.public_attr = 42
    env = Environment()
    rv = env.eval_expr("values|list", values=hmm())
    assert rv == [1, 2, 3]


def test_method_calling():
    class MyClass(object):
        def my_method(self):
            return 23

        def __repr__(self):
            return "This is X"

    env = Environment()
    rv = env.eval_expr("[x ~ '', x.my_method()]", x=MyClass())
    assert rv == ["This is X", 23]


def test_custom_filter():
    def my_filter(value):
        return "<%s>" % value.upper()

    env = Environment()
    env.add_filter("myfilter", my_filter)
    rv = env.eval_expr("'hello'|myfilter")
    assert rv == "<HELLO>"


def test_custom_filter_kwargs():
    def my_filter(value, x):
        return "<%s %s>" % (value.upper(), x)

    env = Environment()
    env.add_filter("myfilter", my_filter)
    rv = env.eval_expr("'hello'|myfilter(x=42)")
    assert rv == "<HELLO 42>"


def test_custom_test():
    def my_test(value, arg):
        return value == arg

    env = Environment()
    env.add_filter("mytest", my_test)
    rv = env.eval_expr("'hello'|mytest(arg='hello')")
    assert rv == True
    rv = env.eval_expr("'hello'|mytest(arg='hellox')")
    assert rv == False


def test_basic_types():
    env = Environment()
    rv = env.eval_expr("{'a': 42, 'b': 42.5, 'c': 'blah'}")
    assert rv == {"a": 42, "b": 42.5, "c": "blah"}


def test_loader():
    called = []

    def my_loader(name):
        called.append(name)
        return "Hello from " + name

    env = Environment(loader=my_loader)
    assert env.render_template("index.html") == "Hello from index.html"
    assert env.render_template("index.html") == "Hello from index.html"
    assert env.render_template("other.html") == "Hello from other.html"
    assert env.loader is my_loader
    assert called == ["index.html", "other.html"]
    env.loader = my_loader
    assert env.render_template("index.html") == "Hello from index.html"
    assert called == ["index.html", "other.html", "index.html"]
    env.reload()
    assert env.render_template("index.html") == "Hello from index.html"
    assert called == ["index.html", "other.html", "index.html", "index.html"]


def test_loader_reload():
    called = []

    def my_loader(name):
        called.append(name)
        return "Hello from " + name

    env = Environment(loader=my_loader)
    env.reload_before_render = True
    assert env.render_template("index.html") == "Hello from index.html"
    assert env.render_template("index.html") == "Hello from index.html"
    assert env.render_template("other.html") == "Hello from other.html"
    assert called == ["index.html", "index.html", "other.html"]


def test_autoescape():
    def auto_escape(name):
        assert name == "foo.html"
        return "html"

    env = Environment(
        auto_escape_callback=auto_escape,
        loader=lambda x: "Hello {{ foo }}",
    )

    rv = env.render_template("foo.html", foo="<x>")
    assert rv == "Hello &lt;x&gt;"

    with catch_unraisable_exception() as cm:
        rv = env.render_template("invalid.html", foo="<x>")
        assert rv == "Hello <x>"
        assert cm.unraisable[0] is AssertionError


def test_globals():
    env = Environment(globals={"x": 23, "y": lambda: 42})
    rv = env.eval_expr("[x, y(), z]", z=11)
    assert rv == [23, 42, 11]


def test_honor_safe():
    env = Environment(auto_escape_callback=lambda x: True)
    rv = env.render_str("{{ x }} {{ y }}", x=safe("<foo>"), y="<bar>")
    assert rv == "<foo> &lt;bar&gt;"


def test_full_object_transfer():
    class X(object):
        def __init__(self):
            self.x = 1
            self.y = 2

    def test_filter(value):
        assert isinstance(value, X)
        return value

    env = Environment(filters=dict(testfilter=test_filter))
    rv = env.eval_expr("x|testfilter", x=X())
    assert isinstance(rv, X)
    assert rv.x == 1
    assert rv.y == 2


def test_markup_transfer():
    env = Environment()
    rv = env.eval_expr("value", value=safe("<foo>"))
    assert hasattr(rv, "__html__")
    assert rv.__html__() == "<foo>"

    rv = env.eval_expr("'<test>'|escape")
    assert hasattr(rv, "__html__")
    assert rv.__html__() == "&lt;test&gt;"


def test_error():
    env = Environment()
    try:
        env.eval_expr("1 +")
    except TemplateError as e:
        assert e.name == "<expression>"
        assert "unexpected end of input" in e.message
        assert "1 > 1 +" not in e.message
        assert "1 > 1 +" in str(e)
        assert e.line == 1
        assert e.kind == "SyntaxError"
    else:
        assert False, "expected error"
