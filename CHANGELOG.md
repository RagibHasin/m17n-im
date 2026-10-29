<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

The latest published m17n-im release is [0.2.0](#020-2026-10-29) which was released on 2026-10-29.
You can find its changes [documented below](#020-2026-10-29).

## [Unreleased][]

This release has an MSRV of 1.88.

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

[Unreleased]: https://github.com/RagibHasin/m17n-im/compare/v0.1.0...HEAD
[0.2.0]: https://github.com/RagibHasin/m17n-im/releases/tag/v0.2.0
[0.1.0]: https://github.com/RagibHasin/m17n-im/releases/tag/v0.1.0
