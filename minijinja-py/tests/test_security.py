from _pytest.unraisableexception import catch_unraisable_exception
from minijinja import Environment, TemplateError, safe


def test_private_attrs():
    class MyClass:
        def __init__(self):
            self.public = 42
            self._private = 23

    env = Environment()
    rv = env.eval_expr("[x.public, x._private]", x=MyClass())
    assert rv == [42, None]


def test_dict_is_always_public():
    env = Environment()
    rv = env.eval_expr("[x.public, x._private]", x={"public": 42, "_private": 23})
    assert rv == [42, 23]
