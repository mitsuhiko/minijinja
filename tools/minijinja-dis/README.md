# minijinja-dis

Takes a template on stdin and prints out the bytecode of the
compiled template.

```
$ echo '{% block foo %}{{ variable|filter }}{% endblock %}' | cargo run
Block: "foo"
     0: Lookup("variable")
     1: ApplyFilter("filter", 1, 0)
     2: Emit
Block: "<root>"
     0: CallBlock("foo")
```
