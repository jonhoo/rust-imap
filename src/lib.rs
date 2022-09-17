//! This crate lets you connect to and interact with servers that implement the IMAP protocol ([RFC
//! 3501](https://tools.ietf.org/html/rfc3501) and various extensions). After authenticating with
//! the server, IMAP lets you list, fetch, and search for e-mails, as well as monitor mailboxes for
//! changes. It supports at least the latest three stable Rust releases (possibly even older ones;
//! check the [CI
//! results](https://dev.azure.com/jonhoo/jonhoo/_build/latest?definitionId=11&branchName=master)).
//!
//! **This crate is looking for maintainers — reach out to [@jonhoo] if you're interested.**
//!
//! [@jonhoo]: https://thesquareplanet.com/
//!
//! To connect, use the [`ClientBuilder`]. This gives you an unauthenticated [`Client`]. You can
//! then use [`Client::login`] or [`Client::authenticate`] to perform username/password or
//! challenge/response authentication respectively. This in turn gives you an authenticated
//! [`Session`], which lets you access the mailboxes at the server.
//!
//! The documentation within this crate borrows heavily from the various RFCs, but should not be
//! considered a complete reference. If anything is unclear, follow the links to the RFCs embedded
//! in the documentation for the various types and methods and read the raw text there!
//!
//! Below is a basic client example. See the `examples/` directory for more.
//!
//! ```no_run
//! # #[cfg(feature = "native-tls")]
//! fn fetch_inbox_top() -> imap::error::Result<Option<String>> {
//!
//!     let client = imap::ClientBuilder::new("imap.example.com", 993).native_tls()?;
//!
//!     // the client we have here is unauthenticated.
//!     // to do anything useful with the e-mails, we need to log in
//!     let mut imap_session = client
//!         .login("me@example.com", "password")
//!         .map_err(|e| e.0)?;
//!
//!     // we want to fetch the first email in the INBOX mailbox
//!     imap_session.select("INBOX")?;
//!
//!     // fetch message number 1 in this mailbox, along with its RFC822 field.
//!     // RFC 822 dictates the format of the body of e-mails
//!     let messages = imap_session.fetch("1", "RFC822")?;
//!     let message = if let Some(m) = messages.iter().next() {
//!         m
//!     } else {
//!         return Ok(None);
//!     };
//!
//!     // extract the message's body
//!     let body = message.body().expect("message did not have a body!");
//!     let body = std::str::from_utf8(body)
//!         .expect("message was not valid utf-8")
//!         .to_string();
//!
//!     // be nice to the server and log out
//!     imap_session.logout()?;
//!
//!     Ok(Some(body))
//! }
//! ```
//!
//! ## Opting out of `native_tls`
//!
//! For situations where using openssl becomes problematic, you can disable the
//! default feature which provides integration with the `native_tls` crate. One major
//! reason you might want to do this is cross-compiling. To opt out of native_tls, add
//! this to your Cargo.toml file:
//!
//! ```toml
//! [dependencies.imap]
//! version = "<some version>"
//! default-features = false
//! ```
//!
//! Even without `native_tls`, you can still use TLS by leveraging the pure Rust `rustls`
//! crate, which is enabled with the `rustls-tls` feature. See the example/rustls.rs file
//! for a working example.
#![deny(missing_docs)]
#![warn(rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod parse;

pub mod types;

mod authenticator;
pub use crate::authenticator::Authenticator;

mod client;
pub use crate::client::*;
mod client_builder;
pub use crate::client_builder::ClientBuilder;

pub mod error;
pub use error::{Error, Result};

pub mod extensions;

#[cfg(test)]
mod mock_stream;
