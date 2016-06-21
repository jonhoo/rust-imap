extern crate imap;
extern crate openssl;

use openssl::ssl::{SslContext, SslMethod};
use imap::client::Client;
use imap::client::IMAPMailbox;

fn main() {
	let mut imap_socket = match Client::secure_connect(("imap.gmail.com", 993), SslContext::new(SslMethod::Sslv23).unwrap()) {
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
