rust-imap
================
IMAP Client for Rust

This client has SSL support. SSL is configured using an SSLContext that is passed into the connect method of a IMAPStream. If no SSL
support is wanted just pass in None. The library rust-openssl is used to support SSL for this project.


[![Build Status](https://travis-ci.org/mattnenterprise/rust-imap.svg)](https://travis-ci.org/mattnenterprise/rust-imap)

[Documentation](http://mattnenterprise.github.io/rust-imap),
[crates.io](https://crates.io/crates/imap).

### Installation

Add imap via your `Cargo.toml`:
```toml
[dependencies]
imap = "*"
```

### Usage
```rust
extern crate imap;
extern crate openssl;

use openssl::ssl::{SslContext, SslMethod};
use imap::client::IMAPStream;
use imap::client::IMAPMailbox;

fn main() {
    let mut imap_socket = match IMAPStream::connect("imap.gmail.com", 993, Some(SslContext::new(SslMethod::Sslv23).unwrap())) {
        Ok(s) => s,
        Err(e) => panic!("{}", e)
    };

    if let Err(e) = imap_socket.login("username", "password") {
        println!("Error: {}", e)
    };

    match imap_socket.capability() {
        Ok(capabilities) => {
            for capability in capabilities.iter() {
                println!("{}", capability);
            }
        },
        Err(_) => println!("Error retreiving capabilities")
    };

    match imap_socket.select("INBOX") {
        Ok(IMAPMailbox{flags, exists, recent, unseen, permanent_flags, uid_next, uid_validity}) => {
            println!("flags: {}, exists: {}, recent: {}, unseen: {:?}, permanent_flags: {:?}, uid_next: {:?}, uid_validity: {:?}", flags, exists, recent, unseen, permanent_flags, uid_next, uid_validity);
        },
        Err(_) => println!("Error selecting INBOX")
    };

    match imap_socket.fetch("2", "body[text]") {
        Ok(lines) => {
            for line in lines.iter() {
                print!("{}", line);
            }
        },
        Err(_) => println!("Error Fetching email 2")
    };

    if let Err(e) = imap_socket.logout() {
        println!("Error: {}", e)
    };  
}
```

### License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
