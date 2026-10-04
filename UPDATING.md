# Updating to MiniJinja 3

MiniJinja 3 aligns the value model more closely with Jinja2 and makes `serde`
optional.  The changes below may require updates when moving from MiniJinja 2.

## Removed Deprecated Rust APIs

APIs deprecated before MiniJinja 3 have been removed:

* Replace `Template::render_and_return_state` with `Template::render_captured`
  and access the output and state through the returned `Captured` value.
* Replace `Template::render_to_write` with `Template::render_captured_to`.
* Replace `Template::eval_to_state` with `Template::render_captured`. To discard
  output, use `render_captured_to` with `std::io::sink()`.
* Replace the `filters::Filter` and `tests::Test` aliases with
  `functions::Function`, and `tests::TestResult` with `value::FunctionResult`.
* Replace `value::intern` with `Arc::<str>::from`. The no-op `key_interning`
  Cargo feature has also been removed.
* Remove the no-op `loader` Cargo feature from dependency declarations. Loader
  APIs remain available unconditionally.

## Template Loaders

`Environment::set_loader` is now generic over the returned source, which can be
a `String` or a `TemplateSource`.  Loaders that return `Ok(Some(string))`
continue to work, but closures that rely on inference through `.into()` or only
ever return `Ok(None)` need an explicit type:

```rust
// Old
env.set_loader(|name| Ok(Some("...".into())));
env.set_loader(|_| Ok(None));

// New
env.set_loader(|name| Ok(Some("...".to_string())));
env.set_loader(|_| Ok(None::<String>));
```

`path_loader` now returns a `TemplateSource` which carries an up-to-date check.
Auto reloading is enabled by default, so templates loaded through `path_loader`
are reloaded individually when they change on disk.  Every template lookup now
performs a `stat` call; to restore the old behavior of loading each template
only once, disable auto reloading (for instance outside of debug builds):

```rust
let mut env = Environment::new();
env.set_loader(path_loader("templates"));
env.set_auto_reload(cfg!(debug_assertions));
```

## Removed `minijinja-autoreload`

The `minijinja-autoreload` crate has been discontinued as reloading templates is
now built into MiniJinja and works through a shared reference.  There is no
more need for a lock or a guard around the environment:

```rust
// Old
let reloader = AutoReloader::new(|notifier| {
    let mut env = Environment::new();
    env.set_loader(path_loader("templates"));
    notifier.watch_path("templates", true);
    Ok(env)
});
let env = reloader.acquire_env()?;
let tmpl = env.get_template("index.html")?;

// New
let mut env = Environment::new();
env.set_loader(path_loader("templates"));
let tmpl = env.get_template("index.html")?;
```

The other features of the crate map as follows:

* `Notifier::set_callback`: attach a check to the template with
  `TemplateSource::with_uptodate_check` in a custom loader.
* `Notifier::request_reload` and fast reloading: call
  `Environment::clear_templates`, or attach an up-to-date check that observes
  an `AtomicBool` or a counter that you change to request a reload.
* Recreating the entire environment (for instance because globals are loaded
  from a file): keep the environment in an `RwLock<Arc<Environment<'static>>>`
  and replace it when needed.  Renders clone the `Arc` and do not hold the
  lock while rendering:

```rust
static ENV: RwLock<Option<Arc<Environment<'static>>>> = RwLock::new(None);

fn get_env() -> Arc<Environment<'static>> {
    ENV.read().unwrap().clone().expect("environment not initialized")
}

fn reload_env() {
    *ENV.write().unwrap() = Some(Arc::new(create_env()));
}
```

## Formatting API

The formatting helper and its style enum are no longer exported from the crate
root. The helper was also renamed from `format_filter` to `format`, since it
supports both the built-in `format` filter and Python-style `str.format()`:

```rust
// Old
use minijinja::{format_filter, FormatStyle};

// New
use minijinja::formatting::{format, FormatStyle};
```

The `formatting` module is available when the `builtins` feature is enabled.

## Mutable Execution State

MiniJinja now uses mutable execution state for all dynamic calls.  Registered
functions, filters, and tests can still take `&State` when they only inspect the
render, or take `&mut State` as their first parameter when they need to modify
render-local data or perform nested calls:

```rust
fn inspect(state: &State, value: Value) -> Value {
    // Shared callbacks continue to work.
    value
}

fn count(state: &mut State, value: Value) -> Value {
    let calls = state.get_or_insert_extension::<usize>(0);
    *calls += 1;
    value
}
```

The erased calling APIs now have a single mutable path.  Update custom
`Object::call` and `Object::call_method` implementations, as well as direct
`Value::call` and `Value::call_method` invocations, to pass `&mut State`.
`State::call_macro`, `State::apply_filter`, and `State::perform_test` likewise
require mutable state.  With `Captured`, use `with_state_mut`:

```rust
let mut captured = template.render_captured(context)?;
let result = captured.with_state_mut(|state| state.call_macro("render", &[]))?;
```

State mutation no longer uses interior mutability.  `State::set_temp` now
requires `&mut State`, and `State::get_or_set_temp_object` was removed.  Replace
object temps with `get_extension`, `get_extension_mut`, or
`get_or_insert_extension`, which provide render-local typed Rust storage without
wrapping data in `Value` or a mutex.  Named temps remain available for data that
needs to be represented as a `Value`.  Extensions persist through includes,
blocks, and macro calls.

Custom formatters and unknown-method callbacks now receive `&mut State` so they
can use the same mutable facilities.

## Whitespace Settings

The `trim_blocks`, `lstrip_blocks` and `keep_trailing_newline` settings moved
from `Environment` into the `SyntaxConfig`.  The syntax config builder,
`Environment::set_syntax` and `Environment::syntax` are now available without
the `custom_syntax` feature, which is only required for custom delimiters:

```rust
// Old
env.set_trim_blocks(true);
env.set_lstrip_blocks(true);
env.set_keep_trailing_newline(true);

// New
use minijinja::syntax::SyntaxConfig;

env.set_syntax(
    SyntaxConfig::builder()
        .trim_blocks(true)
        .lstrip_blocks(true)
        .keep_trailing_newline(true)
        .build()
        .unwrap(),
);
```

To change a single setting while retaining the rest of the current
configuration, use `SyntaxConfig::to_builder`:

```rust
let syntax = env.syntax().to_builder().trim_blocks(true).build().unwrap();
env.set_syntax(syntax);
```

The getters moved as well: use `env.syntax().trim_blocks()` instead of
`env.trim_blocks()`.  Note that setting a syntax config that was created
with `SyntaxConfig::builder()` also resets the whitespace settings to their
defaults.

For users of the `unstable_machinery` feature, `WhitespaceConfig` was removed
and `parse` and `tokenize` no longer take a separate whitespace config.

## Custom Auto Escaping

`AutoEscape::Custom` now holds a `Cow<'static, str>` instead of a
`&'static str`.  This permits custom formats with names determined at runtime
without leaking strings.  As a result `AutoEscape` no longer implements `Copy`
and `State::auto_escape` returns `&AutoEscape`:

```rust
// Old
env.set_auto_escape_callback(|_| AutoEscape::Custom("latex"));
match state.auto_escape() {
    AutoEscape::Custom("latex") => { /* ... */ }
    _ => { /* ... */ }
}

// New
env.set_auto_escape_callback(|_| AutoEscape::Custom("latex".into()));
match state.auto_escape() {
    AutoEscape::Custom(name) if name == "latex" => { /* ... */ }
    _ => { /* ... */ }
}
```

## Go Module Path

MiniJinja-Go now uses the major-version module path
`github.com/mitsuhiko/minijinja/minijinja-go/v3`.  Update Go imports from `/v2`
to `/v3`.  Its tuple values and collection rendering follow the same new
semantics described below.  Most other changes of this document do not apply
to the Go implementation.

## Optional and Explicit Serde Support

The `serde` feature now controls the `serde` dependency and is disabled by
default.  You can enable it explicitly if you need to use the Serde based
conversions but it might be unnecessary for most users.  The main rendering
APIs, `context!`, and `args!` now convert values through `Into<Value>` rather
than using Serde.

The `deserialization` feature was removed and folded into `serde`.  Replace
`features = ["deserialization"]` with `features = ["serde"]`.  The `json`
feature no longer enables `serde`: `tojson` and JSON auto escaping use a
built-in serializer.  If you relied on `json` to enable Serde support you need
to enable the `serde` feature explicitly.

Serde conversion must be requested with the `minijinja::value::Serde` wrapper:

```rust
use minijinja::value::{Serde, Value};

let value = Value::from(Serde(&custom_data));
let output = template.render(Serde(&custom_context))?;
let context = minijinja::context!(data => Serde(&custom_data));
```

The same wrapper deserializes function arguments into Rust types:

```rust
use minijinja::value::Serde;

fn dirname(path: Serde<std::path::PathBuf>) -> String {
    path.display().to_string()
}
```

The new `Serde` type replaces the former `ViaDeserialize` argument wrapper.

`Value::from_serialize` has been removed. Replace it with the explicit `Serde`
wrapper:

```rust
// Old
let value = Value::from_serialize(&custom_data);

// New
let value = Value::from(Serde(&custom_data));
```

The `Serde` wrapper is unavailable when the `serde` feature is disabled.

`context!` and `args!` consume expressions passed through native conversions via
`Value::from`.  You can pass a reference where a supported native value should
be cloned, or use `Serde(&value)` for borrowed Serde data.

Collecting an iterator into `Value` now always creates a sequence, including
when the items are tuples.  Use `Value::from_pairs(iter)` to explicitly create a
map from key-value pairs.

## Tuples

Tuple literals now produce a distinct `Tuple` value instead of a `Vec<Value>`.
Code that downcast tuple expressions to `Vec<Value>` must downcast to `Tuple` or
use the generic sequence APIs.  Tuples still report `ValueKind::Seq` and support
normal sequence iteration and indexing.

Rust tuples passed through `Value::from` or explicit Serde conversion are also
preserved as tuples.  Tuple concatenation, repetition, and slicing return tuples,
while combining a list and a tuple with `+` is an error as it is in Python.

For the Python bindings, Python tuples now remain tuples.  The JavaScript
binding represents evaluated tuples as arrays while retaining tuple rendering
inside templates.

## Value Rendering

Debug representations of strings, sequences, and maps now use Python-style
quoting.  This primarily changes nested rendering from double quotes to single
quotes where possible.  The `tojson` filter now inserts spaces after commas and
colons to match Jinja2's default `json.dumps` output as this divergence has
caused some unnecessary failures in conformity tests that some people use.

## Keyword Arguments

`Value`, `&Value`, `&[Value]`, and `Rest<Value>` function arguments no longer
accept the internal keyword-argument map.  You now need to use an explicit
`Kwargs` parameter for normal keyword arguments.  Variadic forwarding functions
can use `Rest<ValueOrKwargs>` and call `into_values()` before manually splitting
or forwarding the arguments.

# Updating to MiniJinja 2

MiniJinja 2.0 is a major update to MiniJinja that changes a lot of core
internals and cleans up some APIs.  In particular it resolves some limitations
in the engine in relation to working with dynamic objects, unlocks potentials
for future performance improvements and enhancements.  This document helps with
upgrading to that version.

## Syntax Config

If you want to use custom delimiters, the way to configure this was slightly
changed to enable future improvements to this feature.

**Old:**

```rust
use minijinja::{Environment, Syntax};

let mut env = Environment::new();
env.set_syntax(minijinja::Syntax {
    block_start: "{".into(),
    block_end: "}".into(),
    variable_start: "${".into(),
    variable_end: "}".into(),
    comment_start: "{*".into(),
    comment_end: "*}".into(),
})
.unwrap();
```

**New:**

```rust
use minijinja::{Environment, syntax::SyntaxConfig};

let mut env = Environment::new();
env.set_syntax(
    SyntaxConfig::builder()
        .block_delimiters("{", "}")
        .variable_delimiters("${", "}")
        .comment_delimiters("{*", "*}")
        .build()
        .unwrap(),
);
```

## Iterators

In MiniJinja 1.x you could create iterators with the `Value::from_iterator`
function.  That same function is now called `Value::make_one_shot_iterator` and
the use is _discouraged_.  Instead most uses should instead use
`Value::make_iterable` which takes a function returning an iterator.  This has
the advantage that the value can be iterated over multiple times.

**Old:**

```rust
let value = Value::from_iterator(1..10);
```

**New Preferred:**

```rust
let value = Value::make_iterable(|| 1..10);
```

Additionally you can now also make iterables that borrow from other values
by using `Value::make_object_iterable`:

```rust
let value = Value::make_iterable(vec![1, 2, 3], |obj| {
    Box::new(obj.iter().map(|x| Value::from(*x * 2)))
});
```

## Objects

The largest change is the new object systems.  In MiniJinja 2, Objects are now
using an entire new trait.  (`Object`, `SeqObject` and `StructObject`) were
replaced by a single trait called `Object`.  It is however a completely new trait
unrelated to the old one, though it retains some common ideas.

In a nutshell:

* All method use `&Arc<Self>` instead of `&self` as receiver.  This allows one
  to clone out of the object when needed.
* `Object::kind` is gone and was replaced with `Object::repr` in spirit
* All trait methods are directly on the `Object` trait and the `StructObject`
  and `SeqObject` functionality is moved onto the object trait.
* Formatting is done via `Object::render` rather than `fmt::Display`.
* `fmt::Debug` is required in all cases now.
* Iteration now is implemented via `Object::enumerate`.

When working with objects of an unknown type, you can use the new `DynObject`
struct which is a type erased box over `Arc<Object>`.  `Value::as_object` now
returns an `Option<&DynObject>` compared to previously an `Option<&dyn Object>`
as an example.  The `DynObject` can be cheaply cloned which bumps the reference
count.

On the value type, the object related APIs were changed a bit to better
accommodate for the new trait:

* `Value::as_struct` was removed, use `Value::as_object` instead.
* `Value::as_seq` was removed, use `Value::as_object` instead.
* `Value::as_object` now returns a `Option<&DynObject>`.
* `Value::downcast_object` was added which returns an `Option<Arc<T>>`
* `ValueKind` is now non exhaustive and has more variants.

### Structs and Maps

Objects can now directly implement structs and maps.  That gives them greater
flexibility.  Because the receiver is an `Arc<Self>` we can also efficiently
borrow from them.

```rust
#[derive(Debug)]
struct User {
    username: String,
    roles: Vec<String>,
}
```

**Old:**

```rust
use minijinja::value::{Value, StructObject};

impl StructObject for User {
    fn get_field(&self, field: &str) -> Option<Value> {
        Some(match field {
            "username" => Value::from(&self.username),
            "roles" => Value::from(self.roles.clone()),
            _ => return None,
        })
    }

    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(&["username", "roles"])
    }
}

let user = Value::from_struct_object(User { ... });
```

**New:**

The big changes are that `get_value` is now used instead of `get_field` and the
field that is looked up is a `&Value`.  To match on a string we need to call
`as_str()` on it.  For the `roles` here we can keep using the old pattern, or
use the more efficient `Value::make_object_iterable` which can borrow from the
object and make a lazy iterable.  For iteration an `Enumerator::Str` over all
keys is returned from `Object::enumerate`.

```rust
use std::sync::Arc;
use minijinja::value::{Value, Object, Enumerator};

impl Object for User {
    fn get_value(self: &Arc<Self>, field: &Value) -> Option<Value> {
        Some(match field.as_str()? {
            "username" => Value::from(&self.username),
            "roles" => Value::make_object_iterable(self.clone(), |o| {
                Box::new(o.roles.iter().map(Value::from))
            }),
            _ => return None,
        })
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Str(&["foo", "bar"])
    }
}

let value = Value::from_object(User { ... });
```

### Sequences

Sequences are now also just an `Object`.

```rust
#[derive(Debug)]
struct SimpleDynamicSeq([char; 4]);
```

**Old:**

```rust
use minijinja::value::SeqObject;

impl SeqObject for SimpleDynamicSeq {
    fn get_item(&self, idx: usize) -> Option<Value> {
        self.0.get(idx).copied().map(Value::from)
    }

    fn item_count(&self) -> usize {
        4
    }
}

let value = Value::from_seq_object(SimpleDynamicSeq(...));
```

**New:**

Because the default object representation is a `Map`, we need to
change it to `ObjectRepr::seq` in the `repr` method.  As sequences
iterate over their values, we can use the convenient `Enumerator::Seq`
enumerator which instructs the engine to sequentially iterate over
the object from `0` to the given `length`.  Otherwise the interface
is the same as with the map above, which means that rather than
implementing `get_item` you now also implement `get_value` which
replaces it.  To match over the index, use `as_usize()` on the value.

```rust
use minijinja::value::{Object, ObjectRepr, Enumerator, Value};

#[derive(Debug)]
struct SimpleDynamicSeq([char; 4]);

impl Object for SimpleDynamicSeq {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, idx: &Value) -> Option<Value> {
        self.0.get(idx.as_usize()?).copied().map(Value::from)
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Seq(self.0.len())
    }
}

let value = Value::from_object(SimpleDynamicSeq(...));
```

### Methods, Callables and Rendering

The interface for callables is largely unchanged other than the new
receiver.

**Old:**

```rust
use minijinja::{Error, ErrorKind};
use minijinja::value::Object;

#[derive(Debug)]
struct Markdown(String);

impl fmt::Display for Markdown {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl Object for Markdown {
    fn call_method(
        &self,
        _state: &State,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        if name == "render" {
            // assert no arguments
            from_args(args)?;
            Ok(Value::from(render_markdown(&self.0)))
        } else {
            Err(Error::new(
                ErrorKind::UnknownMethod,
                format!("object has no method named {name}"),
            ))
        }
    }
}
```

**New:**

The replacement for `fmt::Display` is the new `Object::render` method.
If you implement it, it overrides the implied default.  Additionally
if you leave out the error message in the `UnknownMethod` error the
engine provides a useful one by default.

```rust
use minijinja::{Error, ErrorKind};
use minijinja::value::Object;

#[derive(Debug)]
struct Markdown(String);

impl Object for Markdown {
    fn call_method(
        self: &Arc<Self>,
        _state: &State,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        if name == "render" {
            from_args(args)?;
            Ok(Value::from(render_markdown(&self.0)))
        } else {
            Err(Error::from(ErrorKind::UnknownMethod))
        }
    }

    fn render(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}
```

## Stack Ref

The old `minijinja-stack-ref` module was removed as it can no longer accommodate the
new object model.  However that module largely is no longer useful as the new object
system is powerful enough to support it _for the most part_.  While it's not possible
any more to return references to objects on the stack, you can now trivially work with
reference counted externally held objects which should resolve a lot of the needs for
the stack-ref module.

For examples of how to do that, look at the new
[`object-ref`](https://github.com/mitsuhiko/minijinja/tree/main/examples/object-ref)
example that is modelled after the old
[`stack-ref`](https://github.com/mitsuhiko/minijinja/tree/1.0.16/examples/stack-ref)
example where you can see the differences between the two.

## Lazy Iterables

With MiniJinja 2 various things that were previously sequences, are now just iterables.
For instance using `|reverse` will only return an iterable, not a sequence.  This means
that you cannot index into this for instance.  On the other hand it performs better
and more efficiently.  The same is now true for slicing into things that are not strings
with the `[:]` operator.

If you do still want a list, you can force it into a list with the `|list` operator.
