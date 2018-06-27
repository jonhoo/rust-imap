<!-- this file uses https://github.com/livioribeiro/cargo-readme -->
<!-- do not manually edit README.md, instead edit README.tpl or src/lib.rs -->

# imap

[![Crate Version](https://img.shields.io/crates/v/imap.svg)](https://crates.io/crates/imap)
[![Documentation](https://docs.rs/imap/badge.svg)](https://docs.rs/imap/)
[![Crate License](https://img.shields.io/crates/l/imap.svg)](https://crates.io/crates/imap)
[![Build Status](https://travis-ci.org/mattnenterprise/rust-imap.svg)](https://travis-ci.org/mattnenterprise/rust-imap)
[![Build Status](https://ci.appveyor.com/api/projects/status/github/mattnenterprise/rust-imap?svg=true)](https://ci.appveyor.com/api/projects/status/github/mattnenterprise/rust-imap)
[![Coverage Status](https://coveralls.io/repos/github/mattnenterprise/rust-imap/badge.svg?branch=master)](https://coveralls.io/github/mattnenterprise/rust-imap?branch=master)

IMAP client bindings for Rust.

## Usage

Here is a basic example of using the client.
See the `examples/` directory for more examples.

```rust
extern crate native_tls;

// To connect to the gmail IMAP server with this you will need to allow unsecure apps access.
// See: https://support.google.com/accounts/answer/6010255?hl=en
// Look at the `examples/gmail_oauth2.rs` for how to connect to gmail securely.
fn main() {
    let domain = "imap.gmail.com";
    let port = 993;
    let socket_addr = (domain, port);
    let ssl_connector = native_tls::TlsConnector::builder().build().unwrap();
    let mut imap_socket = Client::secure_connect(socket_addr, domain, &ssl_connector).unwrap();

    imap_socket.login("username", "password").unwrap();

    match imap_socket.capabilities() {
        Ok(capabilities) => {
            for capability in capabilities.iter() {
                println!("{}", capability);
            }
        }
        Err(e) => println!("Error parsing capabilities: {}", e),
    };

    match imap_socket.select("INBOX") {
        Ok(mailbox) => {
            println!("{}", mailbox);
        }
        Err(e) => println!("Error selecting INBOX: {}", e),
    };

    match imap_socket.fetch("2", "body[text]") {
        Ok(messages) => {
            for message in messages.iter() {
                print!("{:?}", message);
            }
        }
        Err(e) => println!("Error Fetching email 2: {}", e),
    };

    imap_socket.logout().unwrap();
}
```

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
