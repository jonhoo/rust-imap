<!-- this file uses https://github.com/livioribeiro/cargo-readme -->
<!-- do not manually edit README.md, instead edit README.tpl or src/lib.rs -->

# imap

[![Crates.io](https://img.shields.io/crates/v/imap.svg)](https://crates.io/crates/imap)
[![Documentation](https://docs.rs/imap/badge.svg)](https://docs.rs/imap/)
[![Crate License](https://img.shields.io/crates/l/imap.svg)](https://crates.io/crates/imap)
[![Codecov](https://codecov.io/github/jonhoo/rust-imap/coverage.svg?branch=master)](https://codecov.io/gh/jonhoo/rust-imap)
[![Dependency status](https://deps.rs/repo/github/jonhoo/rust-imap/status.svg)](https://deps.rs/repo/github/jonhoo/rust-imap)

This crate lets you connect to and interact with servers that implement the IMAP protocol ([RFC
3501](https://tools.ietf.org/html/rfc3501) and various extensions). After authenticating with
the server, IMAP lets you list, fetch, and search for e-mails, as well as monitor mailboxes for
changes. It supports at least the latest three stable Rust releases (possibly even older ones;
check the [CI
results](https://dev.azure.com/jonhoo/jonhoo/_build/latest?definitionId=11&branchName=master)).

**This crate is looking for maintainers — reach out to [@jonhoo] if you're interested.**

[@jonhoo]: https://thesquareplanet.com/

To connect, use the [`ClientBuilder`]. This gives you an unauthenticated [`Client`]. You can
then use [`Client::login`] or [`Client::authenticate`] to perform username/password or
challenge/response authentication respectively. This in turn gives you an authenticated
[`Session`], which lets you access the mailboxes at the server.

The documentation within this crate borrows heavily from the various RFCs, but should not be
considered a complete reference. If anything is unclear, follow the links to the RFCs embedded
in the documentation for the various types and methods and read the raw text there!

Below is a basic client example. See the `examples/` directory for more.

```rust
fn fetch_inbox_top() -> imap::error::Result<Option<String>> {

    let client = imap::ClientBuilder::new("imap.example.com", 993).native_tls()?;

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client
        .login("me@example.com", "password")
        .map_err(|e| e.0)?;

    // we want to fetch the first email in the INBOX mailbox
    imap_session.select("INBOX")?;

    // fetch message number 1 in this mailbox, along with its RFC822 field.
    // RFC 822 dictates the format of the body of e-mails
    let messages = imap_session.fetch("1", "RFC822")?;
    let message = if let Some(m) = messages.iter().next() {
        m
    } else {
        return Ok(None);
    };

    // extract the message's body
    let body = message.body().expect("message did not have a body!");
    let body = std::str::from_utf8(body)
        .expect("message was not valid utf-8")
        .to_string();

    // be nice to the server and log out
    imap_session.logout()?;

    Ok(Some(body))
}
```

### Opting out of `native_tls`

For situations where using openssl becomes problematic, you can disable the
default feature which provides integration with the `native_tls` crate. One major
reason you might want to do this is cross-compiling. To opt out of native_tls, add
this to your Cargo.toml file:

```toml
[dependencies.imap]
version = "<some version>"
default-features = false
```

Even without `native_tls`, you can still use TLS by leveraging the pure Rust `rustls`
crate, which is enabled with the `rustls-tls` feature. See the example/rustls.rs file
for a working example.

## Running the test suite

To run the integration tests, you need to have [GreenMail
running](http://www.icegreen.com/greenmail/#deploy_docker_standalone). The
easiest way to do that is with Docker:

```console
$ docker pull greenmail/standalone:1.6.8
$ docker run -it --rm -e GREENMAIL_OPTS='-Dgreenmail.setup.test.all -Dgreenmail.hostname=0.0.0.0 -Dgreenmail.auth.disabled -Dgreenmail.verbose' -p 3025:3025 -p 3110:3110 -p 3143:3143 -p 3465:3465 -p 3993:3993 -p 3995:3995 greenmail/standalone:1.6.3
```

Another alternative is to test against cyrus imapd which is a more complete IMAP implementation that greenmail (supporting quotas and ACLs).

```
$ docker pull outoforder/cyrus-imapd-tester
$ docker run -it --rm -p 3025:25 -p 3110:110 -p 3143:143 -p 3465:465 -p 3993:993 outoforder/cyrus-imapd-tester:latest
```

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
