# Changelog

All notable changes to MiniJinja are documented here.

# Unreleased

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

# 0.3.0

- Added support for `{% include %}`
- Resolved a bug that caused `with` blocks to fully shadow the outer scope.
- Improved documentation in the crate.

# 0.2.0

- Added support for rustc versions down to 1.42.0

# 0.1.0

- Initial release of the library
