# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog].

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/

## [Unreleased]
### Added
- test: 7zip compatibility test
- Changelog file

### Changed
- Remove `unsafe` code

### Deprecated

### Removed

### Fixed

### Security

## [0.1.4]
### Added
- `Deflate64Decoder`, Streaming `Read` decoder implementation

### Fixed
- Overflow error in debug build

## [0.1.3]
### Added
- Many documentation comment
- `InflaterManaged.errored()`

### Changed
- Remove Box usage in `InflaterManaged`

## [0.1.2] - 2023-01-16
### Fixed
- Release build will cause compilation error

### Fixed
- Several bugs

## [0.1.1] - 2023-01-15
### Added
- Implement Debug in many struct

## [0.1.0] - 2023-07-29
### Added
- Initial Deflate64 implementation

[Unreleased]: https://github.com/anatawa12/deflate64-rs/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/anatawa12/deflate64-rs/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/anatawa12/deflate64-rs/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/anatawa12/deflate64-rs/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/anatawa12/deflate64-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/anatawa12/deflate64-rs/releases/tag/v0.1.0