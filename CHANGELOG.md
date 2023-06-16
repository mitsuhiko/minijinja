# Changelog

All notable changes to MiniJinja are documented here.

## 1.0.0

- Support unicode sorting for filters when the `unicode` feature is enabled.
  This also fixes confusing behavior when mixed types were sorted.  (#299)

- Added `json5` as file extension for JSON formatter.

**Note:** This also includes all the changes in the different 1.0.0 alphas.

### Breaking Changes

1.0 includes a lot of changes that are breaking.  However they should with
some minor exceptions be rather trivial changes.

- `Environment::source`, `Environment::set_source` and the `Source` type
  together with the `source` feature were removed.  The replacement is the
  new `loader` feature which adds the `add_template_owned` and `set_loader`
  APIs.  The functionality previously provided by `Source::from_path` is
  now available via `path_loader`.

    Old:

    ```rust
    let mut source = Source::with_loader(|name| ...);
    source.add_template("foo", "...").unwrap();
    let mut env = Environment::new();
    env.set_source(source);
    ```

    New:

    ```rust
    let mut env = Environment::new();
    env.set_loader(|name| ...);
    env.add_template_owned("foo", "...").unwrap();
    ```

    Old:

    ```rust
    let mut env = Environment::new();
    env.set_source(Source::from_path("templates"));
    ```

    New:

    ```rust
    let mut env = Environment::new();
    env.set_loader(path_loader("templates"));
    ```

- `Template::render_block` and `Template::render_block_to_write` were
  replaced with APIs of the same name on the `State` returned by
  `Template::eval_to_state` or `Template::render_and_return_state`:

    Old:

    ```rust
    let rv = tmpl.render_block("name", ctx)?;
    ```

    New:

    ```rust
    let rv = tmpl.eval_to_state(ctx)?.render_block("name")?;
    ```

- `Kwargs::from_args` was removed as API as it's no longer necessary since
  the `from_args` function now provides the same functionality:

    Before:

    ```rust
    // just split
    let (args, kwargs) = Kwargs::from_args(values);

    // split and parse
    let (args, kwargs) = Kwargs::from_args(values);
    let (a, b): (i32, i32) = from_args(args)?;
    ```

    After:

    ```rust
    // just split
    let (args, kwargs): (&[Value], Kwargs) = from_args(values)?;

    // split and parse
    let (a, b, kwargs): (i32, i32, Kwargs) = from_args(values)?;
    ```

- The `testutils` feature and `testutils` module were removed.  Instead you
  can now directly create an empty `State` and use the methods provided
  to invoke filters.

    Before:

    ```rust
    let env = Environment::new();
    let rv = apply_filter(&env, "upper", &["hello world".into()]).unwrap();
    ```

    After:

    ```rust
    let env = Environment::new();
    let rv = env.empty_state().apply_filter("upper", &["hello world".into()]).unwrap();
    ```

    Before:

    ```rust
    let env = Environment::new();
    let rv = perform_test(&env, "even", &[42.into()]).unwrap();
    ```

    After:

    ```rust
    let env = Environment::new();
    let rv = env.empty_state().perform_test("even", &[42.into()]).unwrap();
    ```

    Before:

    ```rust
    let env = Environment::new();
    let rv = format(&env, 42.into()).unwrap();
    ```

    After:

    ```rust
    let env = Environment::new();
    let rv = env.empty_state().format(42.into()).unwrap();
    ```

- `intern` and some APIs that use `Arc<String>` now use `Arc<str>`.  This
  means that for instance `StructObject::fields` returns `Arc<str>` instead
  of `Arc<String>`.  All the type conversions that previously accepted
  `Arc<String>` now only support `Arc<str>`.

- `State::current_call` was removed without replacement.  This information
  was unreliably maintained in the engine and caused issues with recursive
  calls.  If you have a need for this API please reach out on the issue
  tracker.

- `Output::is_discarding` was removed without replacement.  This is
  an implementation detail and was unintentionally exposed.  You should not
  write code that depends on the internal state of the `Output`.

## 1.0.0-alpha.4

- `Value` now implements `Ord`.  This also improves the ability of the engine
  to sort more complex values in filters.  (#295)

- `Arc<String>` was replaced with `Arc<str>` in some of the public APIs where
  this shined through.  Support for more complex key types in maps was added.
  You can now have tuple keys for instance.  (#297)

## 1.0.0-alpha.3

- Removed `char` as a value type.  Characters are now represented as strings
  instead.  This solves a bunch of Jinja2 incompatibilities that resulted by
  indexing into strings.  (#292)

- Marked `ErrorKind` as `#[non_exhaustive]`.

- Correctly handle coercing of characters and strings.  `"w" == "w"[0]` is
  now evaluating to `true` as one would expect.

## 1.0.0-alpha.2

- The `include` block now supports `with context` and `without context`
  modifiers but they are ignored.  This is mostly helpful to render some
  Jinja2 templates that depend on this functionality.  (#288)

- Added tests `true`, `false`, `filter`, `test` and filters
  `pprint` and `unique`.  (#287)

- Added support for indexing into strings.  (#149)

- Added `Error::detail` which returns the detail help string.  (#289)

- Added `Error::template_source` and `Error::range` to better support
  rendering of errors outside of the built-in debug printing.  (#286)

## 1.0.0-alpha.1

- Removed `testutils` feature.  New replacement APIs are directly available
  on the `State`.

- Added `Template::render_and_return_state` to render a template and return the
  resulting `State` to permit introspection.  `Template::render_to_write` now
  also returns the `State`.

- Added `State::fuel_levels` to introspect fuel consumption when the fuel feature
  is in use.

- Removed `Source` and the `source` feature.  The replacement is the new `loader`
  feature and the functionality of the source is moved directly into the
  `Environment`.  This also adds `Environment::clear_templates` to unload
  all already loaded templates.  (#275)

- Added `Environment::template_from_str` and `Environment::template_from_named_str`
  to compile templates for temporary use.  (#274)

- Removed `Kwargs::from_args` as this can now be expressed with just
  `from_args`.  (#273)

- `Output` no longer reveals if it's discarding in the public API.

- Added `Value::call`, `Value::call_method` and `Template::new_state`.  The
  combination of these APIs makes it possible to call values which was
  previously not possible.  (#272)

- Added `Template::eval_to_state`.  This replaces the functionality of the
  previous `Template::render_block` which is now available via `State`.
  It also adds support for accessing globals and macros from a template
  via the `State`.  (#271)

- Removed support for `State::current_call`.  This property wasn't too useful
  and unreliable.  Supporting it properly for nested invocations would require
  calls to take a mutable state or use interior mutability which did not seem
  reasonable for this.  (#269)

## 0.34.0

- Updated `self_cell` and `percent-encoding` dependencies.  (#264)

- Added `Template::render_block` and `Template::render_block_to_write` which
  allows rendering a single block in isolation.  (#262)

## 0.33.0

- Hide accidentally exposed `Syntax::compile` method.
- Added `undeclared_variables` methods to `Template` and `Expression`. (#250)

## 0.32.1

- Fixed an issue with autoreload not working properly on windows. (#249)

## 0.32.0

- Added `Value::is_number`. (#240)
- `TryFrom` for `Value` now converts integers to `f32` and `f64`.
- Added the new `custom_syntax` feature which allows custom delimiters
  to be configured. (#245)
- Added `Kwargs` abstraction to easier handle keyword arguments.
- Fixed an issue that `Option<T>` was incorrectly picking up `none`
  for undefined values.
- The `sort` filter now accepts `reverse`, `attribute` and `case_sensitive`
  by keyword argument and sorts case insensitive by default.
- The `dictsort` filter now supports reversing, by value sorting,
  and is sorting case insensitive by default.

## 0.31.1

- The `in` operator now does not fail if the value is undefined and the
  undefined behavior is not strict. (#235)
- The Python binding now supports finalizers. (#238)

## 0.31.0

- Changed the closure behavior of macros to match the one of Jinja2. (#233)
- `Value::from_serializable` will no longer panic on invalid values.  Instead
  the error is deferred to runtime which makes working with objects possible
  that are only partially valid for the runtime environment. (#234)

## 0.30.7

- Added `testutils` module. (#221)
- Make it more obvious that serde flatten sometimes does not work. (#223)
- Added configurable "undefined" value behavior. (#227)
- Make `render!()` reuse the hidden environment.

## 0.30.6

- Resolve bad closure being generated for `do` blocks. (#219)
- Add support for float number values in scientific notation. (#220)

## 0.30.5

- Small performance improvements for when `preserve_order` is used by
  passing known capacities to the constructor.
- Minor performance improvements to the VM by giving the stack an initial
  capacity.
- Change the internal representation of spans to use `u32` rather than
  `usize` for line and column offsets as a small speed improvement during
  compilation.
- Performance improvements for the key interning.
- Disabled `key_interning` by default.
- Renamed features `multi-template` to `multi_template` and
  `adjacent-loop-items` to `adjacent_loop_items`. The old feature names will
  hang around until 1.x as legacy aliases.

## 0.30.4

- Restore compilation on 32bit targets. (#207)

## 0.30.3

- Added the Jinja2 tests `==`, `!=`, `<`, `<=`, `>`, `>=` and `in` for the
  use with `select` and `reject`. (#205)
- String rendering now uses fewer reallocations by setting an initial
  capacity based on the complexity of the template. (#206)

## 0.30.2

- Fixed Python bindings not allowing to access dict keys prefixed with an
  underscore. (#197)
- Added `min`, `max` and `sort` filters. (#199)

## 0.30.1

- Changed `add_global` to perform `Into<Value>` implicitly.
- Added `source_mut` to retrieve a mutable reference to the source.
- Improved Python bindings by adding support for template reloading, better
  documentation, error reporting.
- Added `pass_state` to the Python binding.

## 0.30.0

- Expose `debug` flag from environment.
- Added experimental Python bindings.

## 0.29.0

- Resolve a runtime panic if `{% extends %}` appears in the template
  but never is executed.
- Fixed recursion detection for macros.
- Enforce a maximum size of 10000 items on the range output.
- Added fuel tracking support.
- Added `none` test. (#185)
- Improved various issues with whitespace trimming. (#187)
- `Value::default` now returns `Undefined` rather than `None`.
- Added support for `loop.previtem` and `loop.nextitem`. (#188)

## 0.28.0

- Added `capitalize` filter. (#163)
- Added support for `{% call %}`. (#164)
- Added support for `{% do %}`. (#167)
- Improved testsuite to execute on wasm32-wasi.

## 0.27.0

- Filters, tests and other functions can now be registered with a dynamically
  allocated name. (#146)
- Added `State::current_call` which exposes the name of the currently called
  item. (#150)
- Introduced revamped object model with `SeqObject` and `StructObject`. (#148)
- `Object` now directly exposes `downcast_ref` and `is`.
- Removed `Value::as_slice`
- Introduced `Value::as_seq` and `Value::as_struct`.
- Introduced `Value::from_seq_object` and `Value::from_struct_object`.
- Added the ability for function arguments to be of type `&dyn SeqObject`.
- Renamed `Iter` to `ValueIter`.
- Added `Environment::render_named_str`. (#149)
- Exposed string interning via the `intern` function.
- Improved support for structs in built-in filters.
- Added `indent` filter. (#151)
- Added the `map`, `select` / `selectattr` and `reject` / `rejectattr` filters.
- Added `safe` / `escaped` test.
- Strings now have the same iteration behavior as in Jinja2. (#152)

### Breaking Changes

- The `Object` model changed significantly in this release.  It's now possible
  for objects to have different shapes (structs or sequences today).  As a result
  `SeqObject` and `StructObject` were added to the API.  For changing your objects
  over have a look at the new documentation for `Object`.
- The `Iter` type is now called `ValueIter`.

## 0.26.0

- Changed `Object::attributes` to being an iterator. (#138)
- `Arc<T: Object>` now implements `Object`. (#139)
- Aligned semantics of top-level template code after `extends` with Jinja2. (#140)
- Exposed value creation from Arcs. (#141)
- Performance improvements for value conversions and object creation. (#142)
- Align iteration behavior of dynamic objects with maps.

### Breaking Changes

- The `attributes` method on objects now returns iterators.  To make the
  transition easy change `[..]` to `Box::new([..].into_iter())`.

## 0.25.0

- Added support for recursive macro invocations. (#133)
- Added optional unicode identifier support. (#134)

## 0.24.0

- Catch divisions by zero.
- Correctly render `inf` floats.
- Enforce a maximum recursion depth during parsing.
- Added `Value::try_iter` to iterate over maps and sequences. (#132)

## 0.23.0

- Added `Value::from_function`. (#121)
- Fixed incorrect location information with blocks.
- Fixed broken nested `super()` calls.
- Improve error reporting for failures in blocks and trying to
  `super()` when there is no parent block.
- Performance improvements.
- Added support for `{% import %}` / `{% from .. import .. %}`
  and `{% macro %}`.  (#123)
- Added `Value::is_kwargs` which disambiugates if an object passed
  to a function or filter is a normal object or if it represents
  keyword arguments.
- Added the ability to call functions stored on objects.
- Added `macros` and `multi_template` features to disable some of
  the heavier pieces of MiniJinja.
- Fixed an issue that caused trailing commas not to work in lists.

## 0.22.1

- Fixed an incorrect manifest for `minijinja-autoreload`.

## 0.22.0

- Add `defined` and `undefined` tests to always be included.
- Removed `Source::load_from_path`.
- Added `Source::from_path`.

### Breaking Changes

- Removed `Source::load_from_path`.  Use `Source::with_loader` instead.

## 0.21.0

- Added custom autoescape settings.
- Added custom formatters.
- Restructured engine internals for greater clarity.
- Added support for rendering to `io::Write`.  (#111)
- Make it impossible to implement `Fitler`, `Test` or `Function`
  from outside the crate by sealed the traits.  (#113)
- Added support for remaining arguments with `Rest`.  (#114)
- Filters, tests and functions can now borrow arguments.  (#115)
- Added `Value::as_slice` and `Value::get_get_item_by_index`.
- Added support for span error reporting. (#118)
- Greatly improved handling of nested errors. (#119)
- `ImpossibleOperation` is now `InvalidOperation`.
- Added support for slice syntax. (#120)

### Breaking Changes

- `Filter`, `Test` and `Function` are now sealed traits.
- `ErrorKind::ImpossibleOperation` is now `ErrorKind::InvalidOperation`.
- Moved up MSRV to 1.61.0 due to bugs on older rust versions related to
  HRTBs.

## 0.20.0

- Remove internal refcounts from environment.
- Changed `Object` and `FunctionArgs` interface to take
  arguments by reference. (#101)
- `sync` mode is now always enabled. (#104)
- Removed meta API. (#105)
- Error no longer implements `PartialEq`.
- Simplified the function interface.  Filters, tests and global
  functions can now directly return values instead of results. (#107)
- MiniJinja no longer silently iterates over non iterable values.

### Breaking Changes

- The `meta` API is gone without replacement.
- `Object::call_method` and `Object::call` now take the arguments
  as `&[Value]` instead of `Vec<Value>`.
- `Object::call_method`, `Object::call` and `FunctionArgs::from_values`
  now take the arguments as `&[Value]` instead of `Vec<Value>`.
- The error object used to implement `PartialEq` but this was implemented
  by comparing the error kind instead.  Explicitly use the `.kind()`
  method of the error if you want the same behavior.
- `DebugInfo` is no longer exposed.  This might come back if a better
  API can be found.

## 0.19.1

- Fixed binary subtraction operator requiring a space. (#94)
- Fixed trailing commas not working in function calls. (#95)

## 0.19.0

- Small internal improvements to context creation. (#79)
- Add support for JSON/YAML/JavaScript Escaping.  (#82)
- Add missing escape support for single quotes (`'`).  (#81) 
- Added support for newlines in string literals.  (#85)
- Added support for block assignment syntax.  (#86)
- Added string concatenatino with `+` for Jinja compat.  (#87)
- Enable debug mode by default in debug builds.  (#88)
- Added `render!` macro and `render_str`.  (#89)
- Fixed an issue where trailing whitespace removal did not work on blocks.  (#90)
- Added `loop.changed()` method.  (#91)

## 0.18.1

- Fixed a bad dependency declaration.

## 0.18.0

- Improved debug printing of context.
- Added `-`, `_` and `.` to set of unescaped characters in `urlencode`. (#72)
- Bumped `v_htmlescape` dependency. (#74)

## 0.17.0

- Added support for `{% raw %}`. (#67)
- Minimum Rust version moved up to 1.45.
- Added support for `{% set %}`. (#70)

## 0.16.0

- Added support for unpacking in `with` blocks. (#65)

## 0.15.0

- Bumped minimum version requirement to 1.43.
- Internal refactorings.
- Added support for fully dynamic loading via `Source::with_loader`.
- Renamed `get_source` to `source`.

## 0.14.1

- Fixed `or` expressions not working properly.

## 0.14.0

- Added `bool` filter.
- Added `meta` API. (#55)
- Added support for `ignore missing` in include tags. (#56)
- Added support for choices in include tags. (#57)

## 0.13.0

- Removed deprecated functionality.
- Fix an panic in debug error printing. (#49)

## 0.12.0

- Deprecated `Primitive` and `as_primitive`.
- Deprecated `as_f64`.
- Truthiness of values is now checking container length.  Previously containers
  were always true, now they are only true if they are not empty.
- Strings and safe strings no longer compare the same.
- Changed list and map string output to have delimiters and debug printing.
- Added `batch` and `slice` filter.
- Added the new `items` filter.
- Removed value internal distinction between maps and structs.
- Added `list` filter.
- Added `first` and `last` filters.
- Added `round` and `abs` filters.
- Implemented integer division operator (``//``) and changed division to always
  return floats like documented and to match the Jinja2 implementation.  To
  make this more convenient whole integer floats are now handled like integers in
  some situations.
- Added `recursive` support to for loops.
- Merged `builtin_filters`, `builtin_tests` and `builtin_functions` features
  into `builtins`.
- Added `value::serializing_for_value` to check if serialization is taking place
  for MiniJinja.
- The `Value` type now supports deserialization.  This feature can be disabled
  by removing the default `deserialization` feature.
- Removed optional `memchr` dependency as it does not appear to be useful.

## 0.11.0

*Yanked* — this was a release from the wrong branch

## 0.10.0

- Restructured the value type internally to be simpler and not use unsafe at the
  cost of slightly larger memory footprint. (#30)
- Added `debug` support.  If the debug mode is enabled, errors now carry a lot of
  useful debug information and an alternative representation when formatted into
  strings that show the context of the template where it went all wrong. (#31)
- Added automatic string interning of object/map keys in values. This feature can
  be disabled by removing the default `key_interning` feature. (#35)
- Removed deprecated `Single` type.

## 0.9.0

- Remove one trailing newline to be consistent with Jinja2.
- Resolved a bug where borrowed keys on dynamic objects could not be looked up. (#29)

## 0.8.2

- Restored unconditional compatibility with 1.42.

## 0.8.1

- Turned on all features for the docs on docs.rs

## 0.8.0

- Added `context!` and deprecate `Single`.
- Correctly report template file names in errors.
- Added the `source` method on templates.
- Introduced `State` type and changed parameter to functions from
  `&Environment` to `&State`.
- Added `debug` global.
- Added `tojson` filter.
- Added `urlencode` filter.

## 0.7.0

- Made the `source` method on error be bound to `Send` and `Sync`.

## 0.6.0

- Added `default` filter.
- Added `startingwith` and `endingwith` tests.
- Added global variables and function support.
- Added `range` function.
- Added `dict` function.
- Fixed panic caused by `super()` calls outside of blocks.
- Added `Error::with_source` method.
- Added `object` abstraction.
- Added keyword arguments to function and filter invocations.
- Added Jinja2 filter aliases `e`, `d` and `count`.

## 0.5.0

- Added support for rustc 1.41.0
- Added `v_htmlescape` feature to turn on a faster HTML escaping.
- Export `HtmlEscape` helper.
- Also escape `/` in HTML escaping like `v_htmlescape` does.
- Changed return value type of `get_template` to be a result rather than an
  option.
- Added `Source` behind the `source` feature to support loading of templates
  at runtime without lifetime complications.
- Initial auto escaping decision is now made when the template is loaded from
  the environment and not when they are added.
- The environment can now be cloned.
- Added `sync` feature that can be disabled to disable the ability to send
  objects to other threads where that comes at a cost.
- `safe` and `escape` are now always provided as filters.
- Added support for `self.block_name()`.
- Fixed incorrect behavior where `super()` did not allow filters.
- Added `{% filter %}` blocks.
- Added `value::Single` type to render simple templates with a single value passed.

## 0.4.0

- Added the ability to roundtrip arbitrary values via the serialize interface.
- Added support for tuple unpacking in lists.
- Added dictsort filter.
- Introduced a new trait `ArgType` to handle argument conversions for filters
  and tests so optional arguments can exist.
- Renamed `ValueArgs` trait to `FunctionArgs`.
- Added `reverse` filter.
- Added `trim` filter.
- Added `join` filter.
- Added `number` test.
- Added `string` test.
- Added `sequence` test.
- Added `mapping` test.
- Added `builtin_filters` and `builtin_tests` features to disable the built-in
  filter and test functions.
- Added `is not` syntax for negated tests.
- Added `else` block to for loops.
- Added `if` condition expression to for loops.
- Fixed a bug that caused or/and not to evaluate correctly in certain situations.
- Added `in` and `not in` expressions.
- Added inline `if` expressions.

## 0.3.0

- Added support for `{% include %}`
- Resolved a bug that caused `with` blocks to fully shadow the outer scope.
- Improved documentation in the crate.

## 0.2.0

- Added support for rustc versions down to 1.42.0

## 0.1.0

- Initial release of the library
