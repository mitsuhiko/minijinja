from . import _lowlevel

__all__ = [
    "Environment",
    "TemplateError",
    "safe",
    "escape",
    "render_str",
    "eval_expr",
    "pass_state",
]


class Environment(_lowlevel.Environment):
    """Represents a MiniJinja environment"""

    def __new__(cls, *args, **kwargs):
        # `_lowlevel.Environment` does not accept any arguments
        return super().__new__(cls)

    def __init__(
        self,
        loader=None,
        templates=None,
        filters=None,
        tests=None,
        globals=None,
        debug=True,
        fuel=None,
        auto_escape_callback=None,
        reload_before_render=False,
    ):
        super().__init__()
        if loader is not None:
            if templates:
                raise TypeError("Cannot set loader and templates at the same time")
            self.loader = loader
        elif templates is not None:
            self.loader = dict(templates).get
        if fuel is not None:
            self.fuel = fuel
        if filters:
            for name, callback in filters.items():
                self.add_filter(name, callback)
        if tests:
            for name, callback in tests.items():
                self.add_test(name, callback)
        if globals is not None:
            for name, value in globals.items():
                self.add_global(name, value)
        self.debug = debug
        if auto_escape_callback is not None:
            self.auto_escape_callback = auto_escape_callback
        self.reload_before_render = reload_before_render


DEFAULT_ENVIRONMENT = Environment()


def render_str(*args, **context):
    """Shortcut to render a string with the default environment."""
    return DEFAULT_ENVIRONMENT.render_str(*args, **context)


def eval_expr(*args, **context):
    """Evaluate an expression with the default environment."""
    return DEFAULT_ENVIRONMENT.eval_expr(*args, **context)


try:
    from markupsafe import escape, Markup
except ImportError:
    from html import escape as _escape

    class Markup(str):
        def __html__(self):
            return self

    def escape(value):
        callback = getattr(value, "__html__", None)
        if callback is not None:
            return callback()
        return Markup(_escape(str(value)))


def safe(s):
    """Marks a string as safe."""
    return Markup(s)


def pass_state(f):
    """Pass the engine state to the function as first argument."""
    f.__minijinja_pass_state__ = True
    return f


class TemplateError(RuntimeError):
    """Represents a runtime error in the template engine."""

    def __init__(self, message):
        super().__init__(message)
        self._info = None

    @property
    def message(self):
        """The short message of the error."""
        return self.args[0]

    @property
    def kind(self):
        """The kind of the error."""
        if self._info is None:
            return "Unknown"
        else:
            return self._info.kind

    @property
    def name(self):
        """The name of the template."""
        if self._info is not None:
            return self._info.name

    @property
    def line(self):
        """The line of the error."""
        if self._info is not None:
            return self._info.line

    def __str__(self):
        if self._info is not None:
            return self._info.full_description
        return self.message
