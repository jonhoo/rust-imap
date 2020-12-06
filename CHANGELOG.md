# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
 - VANISHED support in EXPUNGE responses and unsolicited responses (#172).

### Changed
 - MSRV increased to 1.43 for nom6 and bitvec
 - `expunge` and `uid_expunge` return `Result<Deleted>` instead of `Result<Vec<u32>>`.

### Removed

## [2.2.0] - 2020-07-27
### Added

 - Changelog
 - STARTTLS example (#165)
 - Timeout example (#168)
 - Export `Result` and `Error` types (#170)

### Changed

 - MSRV increased
 - Better documentation of server greeting handling (#168)

[Unreleased]: https://github.com/jonhoo/rust-imap/compare/v2.2.0...HEAD
[2.2.0]: https://github.com/jonhoo/rust-imap/compare/v2.1.2...v2.2.0
