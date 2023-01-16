from .minijinja_py import Environment
Environment.__module__ = __name__


def render_str(source, **context):
    """Shortcut to render a string with the default environment."""
    return Environment().render_str(source, **context)