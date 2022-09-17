# error

This example demonstrates the built-in error rendering support for better
template debuggability

```console
$ cargo run
Error rendering template: could not render an included template: happend in "include.txt" (in hello.txt:8)
---------------------------- Template Source -----------------------------
   5 |             {% with foo = 42 %}
   6 |               {{ range(10) }}
   7 |               {{ other_seq|join(" ") }}
   8 >               {% include "include.txt" %}
     i                  ^^^^^^^^^^^^^^^^^^^^^ could not render an included template
   9 |             {% endwith %}
  10 |           {% endwith %}
  11 |         {% endfor %}
--------------------------------------------------------------------------
Referenced variables: {
    other_seq: [
        0,
        1,
        2,
        3,
        4,
    ],
    range: minijinja::functions::builtins::range,
    foo: 42,
}
--------------------------------------------------------------------------

caused by: impossible operation: tried to use + operator on unsupported types number and string (in include.txt:1)
---------------------------- Template Source -----------------------------
   1 > Hello {{ item_squared + bar }}!
     i          ^^^^^^^^^^^^^^^^^^ impossible operation
--------------------------------------------------------------------------
Referenced variables: {
    bar: "test",
    item_squared: 4,
}
--------------------------------------------------------------------------
```
