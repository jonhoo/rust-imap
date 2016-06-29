rust-imap
================
IMAP Client for Rust

[![Build Status](https://travis-ci.org/mattnenterprise/rust-imap.svg)](https://travis-ci.org/mattnenterprise/rust-imap)
[![crates.io](http://meritbadge.herokuapp.com/imap)](https://crates.io/crates/imap)
[![Coverage Status](https://coveralls.io/repos/github/mattnenterprise/rust-imap/badge.svg?branch=master)](https://coveralls.io/github/mattnenterprise/rust-imap?branch=master)


[Documentation](http://mattnenterprise.github.io/rust-imap)

### Usage
Here is a basic example of using the client. See the examples directory for more examples.
```rust
extern crate imap;
extern crate openssl;

use openssl::ssl::{SslContext, SslMethod};
use imap::client::Client;

// To connect to the gmail IMAP server with this you will need to allow unsecure apps access.
// See: https://support.google.com/accounts/answer/6010255?hl=en
fn main() {
	let mut imap_socket = Client::secure_connect(("imap.gmail.com", 993), SslContext::new(SslMethod::Sslv23).unwrap()).unwrap();

	imap_socket.login("username", "password").unwrap();

	match imap_socket.capability() {
		Ok(capabilities) => {
			for capability in capabilities.iter() {
				println!("{}", capability);
			}
		},
		Err(e) => println!("Error parsing capability: {}", e)
	};

	match imap_socket.select("INBOX") {
		Ok(mailbox) => {
			println!("{}", mailbox);
		},
		Err(e) => println!("Error selecting INBOX: {}", e)
	};

	match imap_socket.fetch("2", "body[text]") {
		Ok(lines) => {
			for line in lines.iter() {
				print!("{}", line);
			}
		},
		Err(e) => println!("Error Fetching email 2: {}", e)
	};

	imap_socket.logout().unwrap();
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
