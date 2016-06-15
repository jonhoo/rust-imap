#![crate_name = "imap"]
#![crate_type = "lib"]

//! imap is a IMAP client for Rust.

extern crate openssl;
extern crate regex;

pub mod client;
