//! This crate lets you connect to and interact with servers that implement the IMAP protocol ([RFC
//! 3501](https://tools.ietf.org/html/rfc3501) and various extensions). After authenticating with
//! the server, IMAP lets you list, fetch, and search for e-mails, as well as monitor mailboxes for
//! changes. It supports at least the latest three stable Rust releases (possibly even older ones;
//! check the [CI results](https://travis-ci.com/jonhoo/rust-imap)).
//!
//! To connect, use the [`connect`] function. This gives you an unauthenticated [`Client`]. You can
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
//! extern crate imap;
//! extern crate native_tls;
//!
//! fn fetch_inbox_top() -> imap::error::Result<Option<String>> {
//!     let domain = "imap.example.com";
//!     let tls = native_tls::TlsConnector::builder().build().unwrap();
//!
//!     // we pass in the domain twice to check that the server's TLS
//!     // certificate is valid for the domain we're connecting to.
//!     let client = imap::connect((domain, 993), domain, &tls).unwrap();
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

#![deny(missing_docs)]

extern crate base64;
extern crate bufstream;
extern crate imap_proto;
extern crate native_tls;
extern crate nom;
extern crate regex;
extern crate fallible_iterator;
#[macro_use]
extern crate enumset;

mod parse;
mod unsolicited_responses;

pub mod types;

mod authenticator;
pub use authenticator::Authenticator;

mod client;
pub use client::*;

pub mod error;

pub mod extensions;

#[cfg(test)]
mod mock_stream;
