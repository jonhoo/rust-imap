//! IMAP client bindings for Rust.
//!
//! # Usage
//!
//! Here is a basic example of using the client.
//! See the `examples/` directory for more examples.
//!
//! ```no_run
//! extern crate imap;
//! extern crate native_tls;
//!
//! // To connect to the gmail IMAP server with this you will need to allow unsecure apps access.
//! // See: https://support.google.com/accounts/answer/6010255?hl=en
//! // Look at the `examples/gmail_oauth2.rs` for how to connect to gmail securely.
//! fn main() {
//!     let domain = "imap.gmail.com";
//!     let port = 993;
//!     let socket_addr = (domain, port);
//!     let ssl_connector = native_tls::TlsConnector::builder().build().unwrap();
//!     let client = imap::client::secure_connect(socket_addr, domain, &ssl_connector).unwrap();
//!
//!     let mut imap_session = match client.login("username", "password") {
//!         Ok(c) => c,
//!         Err((e, _unauth_client)) => {
//!             eprintln!("failed to login: {}", e);
//!             return;
//!         }
//!     };
//!
//!     match imap_session.capabilities() {
//!         Ok(capabilities) => {
//!             for capability in capabilities.iter() {
//!                 println!("{}", capability);
//!             }
//!         }
//!         Err(e) => println!("Error parsing capabilities: {}", e),
//!     };
//!
//!     match imap_session.select("INBOX") {
//!         Ok(mailbox) => {
//!             println!("{}", mailbox);
//!         }
//!         Err(e) => println!("Error selecting INBOX: {}", e),
//!     };
//!
//!     match imap_session.fetch("2", "body[text]") {
//!         Ok(messages) => {
//!             for message in messages.iter() {
//!                 print!("{:?}", message);
//!             }
//!         }
//!         Err(e) => println!("Error Fetching email 2: {}", e),
//!     };
//!
//!     imap_session.logout().unwrap();
//! }
//! ```

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
