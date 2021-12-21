# Changelog

All notable changes to MiniJinja are documented here.

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
