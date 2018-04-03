#![crate_name = "imap"]
#![crate_type = "lib"]

//! imap is a IMAP client for Rust.

extern crate bufstream;
extern crate imap_proto;
extern crate native_tls;
extern crate nom;
extern crate regex;

mod parse;
mod types;

pub mod authenticator;
pub mod client;
pub mod error;

pub use types::*;

#[cfg(test)]
mod mock_stream;
