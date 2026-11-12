<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->
<!-- markdownlint-configure-file
{ "no-duplicate-heading": { "siblings_only": true } }
-->

# Changelog

The latest published m17n-im release is [0.2.2](#022-2026-11-12) which was released on 2026-11-09.
You can find its changes [documented below](#022-2026-11-12).

## [Unreleased][]

This release has an MSRV of 1.75.

## [0.2.2][] (2026-11-12)

This release has an MSRV of 1.75.

### Changed

- This version only changes the MSRV down to 1.75 to support building this with default Rust package found on current LTS releases of Linux distros.

## [0.2.1][] (2026-11-09)

This release has an MSRV of 1.88.

### Fixed

- Input characters unmatched by any rule was not correctly being pushed into pre-edit buffer, causing infinite loop.
  This was a regression in v0.2.0 and has been fixed.

## [0.2.0][] (2026-10-29)

This release has an MSRV of 1.88.

### Changed

- Uses of `HashMap` for variable and marker tracking has been replaced by flat `Vec<(K, V)>`.
  This consequently eliminated the cumbersome `Hash` bound on `MarkerId` and `VarId`.
  This should also improve performance slightly.
- The bundled Khipro example in the repo has been updated.

### Fixed

- `MARK`, `MOVE`, and `DELETE` behavior for custom markers were implemented.

## [0.1.0][] (2026-10-25) [Initial release]

This release has an MSRV of 1.88.

### Added

- `Engine` as the primary entry point.
- `InputMethod` abstraction and its two user facing instance `ParsedInputMethod` and `EmbeddableInputMethod`.
- `.mim` file parser and Rust code generator for it.

[Unreleased]: https://github.com/RagibHasin/m17n-im/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/RagibHasin/m17n-im/releases/tag/v0.2.2
[0.2.1]: https://github.com/RagibHasin/m17n-im/releases/tag/v0.2.1
[0.2.0]: https://github.com/RagibHasin/m17n-im/releases/tag/v0.2.0
[0.1.0]: https://github.com/RagibHasin/m17n-im/releases/tag/v0.1.0
