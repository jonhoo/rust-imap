# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added

### Changed

 - Handle empty-set inputs to `fetch` and `uid_fetch` (#177)

### Removed

## [2.4.0] - 2020-12-15
### Added

 - `append_with_flags_and_date` (#174)

## [2.3.0] - 2020-08-23
### Added

 - `append_with_flags` (#171)

## [2.2.0] - 2020-07-27
### Added

 - Changelog
 - STARTTLS example (#165)
 - Timeout example (#168)
 - Export `Result` and `Error` types (#170)

### Changed

 - MSRV increased
 - Better documentation of server greeting handling (#168)

[Unreleased]: https://github.com/jonhoo/rust-imap/compare/v2.4.0...HEAD
[2.4.0]: https://github.com/jonhoo/rust-imap/compare/v2.2.0...v2.4.0
[2.3.0]: https://github.com/jonhoo/rust-imap/compare/v2.2.0...v2.3.0
[2.2.0]: https://github.com/jonhoo/rust-imap/compare/v2.1.2...v2.2.0
