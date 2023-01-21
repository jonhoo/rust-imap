# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
 - VANISHED support in EXPUNGE responses and unsolicited responses (#172).
 - SORT command extension (#178).
 - Support for grabbing Gmail labels (#225).
 - Support for the ACL extension (#227).
 - Support for the quote extension (#235).
 - Support for the list-status extension (#249).
 - Expose APPENDUID data (#232).

### Changed
 - MSRV increased to 1.56.1 for 2021 edition
 - `expunge` and `uid_expunge` return `Result<Deleted>` instead of `Result<Vec<u32>>`.
 - IDLE capability now provides a builder interface. All `wait_*` functions are merged into `wait_while` which takes a callback with an `UnsolicitedResponse` in parameter.
 - All `Session.append_with_*` methods are obsoleted by `append` which returns now an `AppendCmd` builder.
 - Envelope `&'a [u8]` attributes are replaced by `Cow<'a, [u8]>`.
 - `Flag`, `Mailbox`, `UnsolicitedResponse` and `Error` are now declared as non exhaustive.
 - `ClientBuilder` now replaces the `imap::connect` function [#197](https://github.com/jonhoo/rust-imap/pull/197).
 - The `tls` feature is now `native-tls` to disambiguate it from the new `rustls-tls` feature. `native-tls` remains in the default feature set.

## [2.4.1] - 2021-01-12
### Changed

 - Handle empty-set inputs to `fetch` and `uid_fetch` (#177)

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

[Unreleased]: https://github.com/jonhoo/rust-imap/compare/v2.4.1...HEAD
[2.4.1]: https://github.com/jonhoo/rust-imap/compare/v2.4.0...v2.4.1
[2.4.0]: https://github.com/jonhoo/rust-imap/compare/v2.3.0...v2.4.0
[2.3.0]: https://github.com/jonhoo/rust-imap/compare/v2.2.0...v2.3.0
[2.2.0]: https://github.com/jonhoo/rust-imap/compare/v2.1.2...v2.2.0
