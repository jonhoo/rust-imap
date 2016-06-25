extern crate imap;
extern crate openssl;

use openssl::ssl::{SslContext, SslMethod};
use imap::client::Client;
use imap::mailbox::Mailbox;

fn main() {
	let mut imap_socket = Client::secure_connect(("imap.gmail.com", 993), SslContext::new(SslMethod::Sslv23).unwrap()).unwrap();

	imap_socket.login("username", "password").unwrap();

	match imap_socket.capability() {
		Ok(capabilities) => {
			for capability in capabilities.iter() {
				println!("{}", capability);
			}
		},
		Err(_) => println!("Error retreiving capabilities")
	};

	match imap_socket.select("INBOX") {
		Ok(Mailbox{flags, exists, recent, unseen, permanent_flags, uid_next, uid_validity}) => {
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

	imap_socket.logout().unwrap();
}
