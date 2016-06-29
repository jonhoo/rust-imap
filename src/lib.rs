#![crate_name = "imap"]
#![crate_type = "lib"]

//! imap is a IMAP client for Rust.

extern crate openssl;
extern crate regex;

pub mod authenticator;
pub mod client;
pub mod error;
pub mod mailbox;

mod parse;

#[cfg(test)]
mod mock_stream;
