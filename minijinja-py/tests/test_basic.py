from minijinja_py import Environment


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
