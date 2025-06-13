# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2025-06-13

## Added

- All `json_assert` macros now have second forms, where a custom panic message
  can be provided.

## [0.3.1] - 2025-06-08

### Fixed

- The types `Difference`, `Path` and `Key` are now properly exposed in the
  public API.
- Fixed documentation of `try_assert_json_matches()`.

## [0.3.0] - 2025-06-08

### Added

- Expose the types `Difference`, `Path` and `Key`.
- New function `try_assert_json_matches()` which returns a `Vec<Difference>`
  instead of a error message. Allow user to do further processing.

## [0.2.1] - 2025-03-11

### Fixed

- Strict comparisons with ignored sorting order no longer allows different
  number of elements in arrays.
- Inclusive comparisons with ignored sorting order fixed. The "expected" and
  "actual" arguments were flipped.

## [0.2.0] - 2025-02-28

### Added

- Float compare mode, by @JonathanMurray
  ([patch](https://github.com/JonathanMurray/assert-json-diff/tree/379b3548c086867cf538ddb77407714a35ee63b1)).
- `assert_json_contains` implementation by @marlon-sousa (and @briankung)
  ([patch](https://github.com/briankung/assert-json-diff/tree/da9af96806e16860c15ff002cf813b021d3bdb8a)).

## [0.1.0] - 2025-02-28

### Added

- Initial release after fork of https://github.com/davidpdrsn/assert-json-diff

[unreleased]: https://github.com/hardselius/serde-json-assert/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/hardselius/serde-json-assert/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/hardselius/serde-json-assert/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/hardselius/serde-json-assert/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/hardselius/serde-json-assert/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/hardselius/serde-json-assert/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/hardselius/serde-json-assert/releases/tag/0.1.0
