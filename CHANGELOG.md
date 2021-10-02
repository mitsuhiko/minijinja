# Changelog

All notable changes to MiniJinja are documented here.

# Unreleased

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
