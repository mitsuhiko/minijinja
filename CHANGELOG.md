# Changelog

All notable changes to MiniJinja are documented here.

# Unreleased

- Added support for recursive macro invocations. (#133)
- Added optional unicode identifier support. (#134)

# 0.24.0

- Catch divisions by zero.
- Correctly render `inf` floats.
- Enforce a maximum recursion depth during parsing.
- Added `Value::try_iter` to iterate over maps and sequences. (#132)

# 0.23.0

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
- Added `macros` and `multi-template` features to disable some of
  the heavier pieces of MiniJinja.
- Fixed an issue that caused trailing commas not to work in lists.

# 0.22.1

- Fixed an incorrect manifest for `minijinja-autoreload`.

# 0.22.0

- Add `defined` and `undefined` tests to always be included.
- Removed `Source::load_from_path`.
- Added `Source::from_path`.

## Breaking Changes

- Removed `Source::load_from_path`.  Use `Source::with_loader` instead.

# 0.21.0

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

## Breaking Changes

- `Filter`, `Test` and `Function` are now sealed traits.
- `ErrorKind::ImpossibleOperation` is now `ErrorKind::InvalidOperation`.
- Moved up MSRV to 1.61.0 due to bugs on older rust versions related to
  HRTBs.

# 0.20.0

- Remove internal refcounts from environment.
- Changed `Object` and `FunctionArgs` interface to take
  arguments by reference. (#101)
- `sync` mode is now always enabled. (#104)
- Removed meta API. (#105)
- Error no longer implements `PartialEq`.
- Simplified the function interface.  Filters, tests and global
  functions can now directly return values instead of results. (#107)
- MiniJinja no longer silently iterates over non iterable values.

## Breaking Changes

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

# 0.19.1

- Fixed binary subtraction operator requiring a space. (#94)
- Fixed trailing commas not working in function calls. (#95)

# 0.19.0

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

# 0.18.1

- Fixed a bad dependency declaration.

# 0.18.0

- Improved debug printing of context.
- Added `-`, `_` and `.` to set of unescaped characters in `urlencode`. (#72)
- Bumped `v_htmlescape` dependency. (#74)

# 0.17.0

- Added support for `{% raw %}`. (#67)
- Minimum Rust version moved up to 1.45.
- Added support for `{% set %}`. (#70)

# 0.16.0

- Added support for unpacking in `with` blocks. (#65)

# 0.15.0

- Bumped minimum version requirement to 1.43.
- Internal refactorings.
- Added support for fully dynamic loading via `Source::with_loader`.
- Renamed `get_source` to `source`.

# 0.14.1

- Fixed `or` expressions not working properly.

# 0.14.0

- Added `bool` filter.
- Added `meta` API. (#55)
- Added support for `ignore missing` in include tags. (#56)
- Added support for choices in include tags. (#57)

# 0.13.0

- Removed deprecated functionality.
- Fix an panic in debug error printing. (#49)

# 0.12.0

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

# 0.11.0

*Yanked* — this was a release from the wrong branch

# 0.10.0

- Restructured the value type internally to be simpler and not use unsafe at the
  cost of slightly larger memory footprint. (#30)
- Added `debug` support.  If the debug mode is enabled, errors now carry a lot of
  useful debug information and an alternative representation when formatted into
  strings that show the context of the template where it went all wrong. (#31)
- Added automatic string interning of object/map keys in values. This feature can
  be disabled by removing the default `key_interning` feature. (#35)
- Removed deprecated `Single` type.

# 0.9.0

- Remove one trailing newline to be consistent with Jinja2.
- Resolved a bug where borrowed keys on dynamic objects could not be looked up. (#29)

# 0.8.2

- Restored unconditional compatibility with 1.42.

# 0.8.1

- Turned on all features for the docs on docs.rs

# 0.8.0

- Added `context!` and deprecate `Single`.
- Correctly report template file names in errors.
- Added the `source` method on templates.
- Introduced `State` type and changed parameter to functions from
  `&Environment` to `&State`.
- Added `debug` global.
- Added `tojson` filter.
- Added `urlencode` filter.

# 0.7.0

- Made the `source` method on error be bound to `Send` and `Sync`.

# 0.6.0

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

# 0.5.0

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

# 0.4.0

- Added the ability to roundtrip arbitrary values via the serialize interface.
- Added support for tuple unpacking in lists.
- Added dictsort filter.
- Introduced a new trait `ArgType` to handle argument conversions for filters
  and tests so optonal arguments can exist.
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

# 0.3.0

- Added support for `{% include %}`
- Resolved a bug that caused `with` blocks to fully shadow the outer scope.
- Improved documentation in the crate.

# 0.2.0

- Added support for rustc versions down to 1.42.0

# 0.1.0

- Initial release of the library
