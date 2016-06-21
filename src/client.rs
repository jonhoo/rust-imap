use std::net::{TcpStream, ToSocketAddrs};
use openssl::ssl::{SslContext, SslStream};
use std::io::{Error, ErrorKind, Read, Result, Write};
use regex::Regex;

static TAG_PREFIX: &'static str = "a";
const INITIAL_TAG: u32 = 0;

/// Stream to interface with the IMAP server. This interface is only for the command stream.
pub struct Client<T> {
	stream: T,
	tag: u32
}

pub struct Mailbox {
	pub flags: String,
	pub exists: u32,
	pub recent: u32,
	pub unseen: Option<u32>,
	pub permanent_flags: Option<String>,
	pub uid_next: Option<u32>,
	pub uid_validity: Option<u32>
}

impl Default for Mailbox {
	fn default() -> Mailbox {
		Mailbox {
			flags: "".to_string(),
			exists: 0,
			recent: 0,
			unseen: None,
			permanent_flags: None,
			uid_next: None,
			uid_validity: None
		}
	}
}

impl Client<TcpStream> {
	/// Creates a new client.
	pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Client<TcpStream>> {
		match TcpStream::connect(addr) {
			Ok(stream) => {
				let mut socket = Client {
					stream: stream,
					tag: INITIAL_TAG
				};

				try!(socket.read_greeting());
				Ok(socket)
			},
			Err(e) => Err(e)
		}
	}
}

impl Client<SslStream<TcpStream>> {
	/// Creates a client with an SSL wrapper.
	pub fn secure_connect<A: ToSocketAddrs>(addr: A, ssl_context: SslContext) -> Result<Client<SslStream<TcpStream>>> {
		match TcpStream::connect(addr) {
			Ok(stream) => {
				let mut socket = Client {
					stream: SslStream::connect(&ssl_context, stream).unwrap(),
					tag: INITIAL_TAG
				};

				try!(socket.read_greeting());
				Ok(socket)
			},
			Err(e) => Err(e)
		}
	}
}

impl<T: Read+Write> Client<T> {

	/// Log in to the IMAP server.
	pub fn login(&mut self, username: & str, password: & str) -> Result<()> {
		self.run_command_and_check_ok(&format!("LOGIN {} {}", username, password).to_string())
	}

	/// Selects a mailbox
	pub fn select(&mut self, mailbox_name: &str) -> Result<Mailbox> {
		match self.run_command(&format!("SELECT {}", mailbox_name).to_string()) {
			Ok(lines) => self.parse_select_or_examine(lines),
			Err(e) => Err(e)
		}
	}

	fn parse_select_or_examine(&mut self, lines: Vec<String>) -> Result<Mailbox> {
		let exists_regex = Regex::new(r"^\* (\d+) EXISTS\r\n").unwrap();

		let recent_regex = Regex::new(r"^\* (\d+) RECENT\r\n").unwrap();

		let flags_regex = Regex::new(r"^\* FLAGS (.+)\r\n").unwrap();

		let unseen_regex = Regex::new(r"^OK \[UNSEEN (\d+)\](.*)\r\n").unwrap();

		let uid_validity_regex = Regex::new(r"^OK \[UIDVALIDITY (\d+)\](.*)\r\n").unwrap();

		let uid_next_regex = Regex::new(r"^OK \[UIDNEXT (\d+)\](.*)\r\n").unwrap();

		let permanent_flags_regex = Regex::new(r"^OK \[PERMANENTFLAGS (.+)\]\r\n").unwrap();

		//Check Ok
		match self.parse_response_ok(lines.clone()) {
			Ok(_) => (),
			Err(e) => return Err(e)
		};

		let mut mailbox = Mailbox::default();

		for line in lines.iter() {
			if exists_regex.is_match(line) {
				let cap = exists_regex.captures(line).unwrap();
				mailbox.exists = cap.at(1).unwrap().parse::<u32>().unwrap();
			} else if recent_regex.is_match(line) {
				let cap = recent_regex.captures(line).unwrap();
				mailbox.recent = cap.at(1).unwrap().parse::<u32>().unwrap();
			} else if flags_regex.is_match(line) {
				let cap = flags_regex.captures(line).unwrap();
				mailbox.flags = cap.at(1).unwrap().to_string();
			} else if unseen_regex.is_match(line) {
				let cap = unseen_regex.captures(line).unwrap();
				mailbox.unseen = Some(cap.at(1).unwrap().parse::<u32>().unwrap());
			} else if uid_validity_regex.is_match(line) {
				let cap = uid_validity_regex.captures(line).unwrap();
				mailbox.uid_validity = Some(cap.at(1).unwrap().parse::<u32>().unwrap());
			} else if uid_next_regex.is_match(line) {
				let cap = uid_next_regex.captures(line).unwrap();
				mailbox.uid_next = Some(cap.at(1).unwrap().parse::<u32>().unwrap());
			} else if permanent_flags_regex.is_match(line) {
				let cap = permanent_flags_regex.captures(line).unwrap();
				mailbox.permanent_flags = Some(cap.at(1).unwrap().to_string());
			}
		}

		Ok(mailbox)
	}

	/// Examine is identical to Select, but the selected mailbox is identified as read-only
	pub fn examine(&mut self, mailbox_name: &str) -> Result<Mailbox> {
		match self.run_command(&format!("EXAMINE {}", mailbox_name).to_string()) {
			Ok(lines) => self.parse_select_or_examine(lines),
			Err(e) => Err(e)
		}
	}

	/// Fetch retreives data associated with a message in the mailbox.
	pub fn fetch(&mut self, sequence_set: &str, query: &str) -> Result<Vec<String>> {
		self.run_command(&format!("FETCH {} {}", sequence_set, query).to_string())
	}

	/// Noop always succeeds, and it does nothing.
	pub fn noop(&mut self) -> Result<()> {
		self.run_command_and_check_ok("NOOP")
	}

	/// Logout informs the server that the client is done with the connection.
	pub fn logout(&mut self) -> Result<()> {
		self.run_command_and_check_ok("LOGOUT")
	}

	/// Create creates a mailbox with the given name.
	pub fn create(&mut self, mailbox_name: &str) -> Result<()> {
		self.run_command_and_check_ok(&format!("CREATE {}", mailbox_name).to_string())
	}

	/// Delete permanently removes the mailbox with the given name.
	pub fn delete(&mut self, mailbox_name: &str) -> Result<()> {
		self.run_command_and_check_ok(&format!("DELETE {}", mailbox_name).to_string())
	}

	/// Rename changes the name of a mailbox.
	pub fn rename(&mut self, current_mailbox_name: &str, new_mailbox_name: &str) -> Result<()> {
		self.run_command_and_check_ok(&format!("RENAME {} {}", current_mailbox_name, new_mailbox_name).to_string())
	}

	/// Subscribe adds the specified mailbox name to the server's set of "active" or "subscribed"
	/// mailboxes as returned by the LSUB command.
	pub fn subscribe(&mut self, mailbox: &str) -> Result<()> {
		self.run_command_and_check_ok(&format!("SUBSCRIBE {}", mailbox).to_string())
	}

	/// Unsubscribe removes the specified mailbox name from the server's set of "active" or "subscribed"
	/// mailboxes as returned by the LSUB command.
	pub fn unsubscribe(&mut self, mailbox: &str) -> Result<()> {
		self.run_command_and_check_ok(&format!("UNSUBSCRIBE {}", mailbox).to_string())
	}

	/// Capability requests a listing of capabilities that the server supports.
	pub fn capability(&mut self) -> Result<Vec<String>> {
		match self.run_command(&format!("CAPABILITY").to_string()) {
			Ok(lines) => self.parse_capability(lines),
			Err(e) => Err(e)
		}
	}

	fn parse_capability(&mut self, lines: Vec<String>) -> Result<Vec<String>> {
		let capability_regex = match Regex::new(r"^\* CAPABILITY (.*)\r\n") {
    		Ok(re) => re,
    		Err(err) => panic!("{}", err),
		};

		//Check Ok
		match self.parse_response_ok(lines.clone()) {
			Ok(_) => (),
			Err(e) => return Err(e)
		};

		for line in lines.iter() {
			if capability_regex.is_match(line) {
				let cap = capability_regex.captures(line).unwrap();
				let capabilities_str = cap.at(1).unwrap();
				return Ok(capabilities_str.split(' ').map(|x| x.to_string()).collect());
			}
		}

		Err(Error::new(ErrorKind::Other, "Error parsing capabilities response"))
	}

	/// Expunge permanently removes all messages that have the \Deleted flag set from the currently
	/// selected mailbox.
	pub fn expunge(&mut self) -> Result<()> {
		self.run_command_and_check_ok("EXPUNGE")
	}

	/// Check requests a checkpoint of the currently selected mailbox.
	pub fn check(&mut self) -> Result<()> {
		self.run_command_and_check_ok("CHECK")
	}

	/// Close permanently removes all messages that have the \Deleted flag set from the currently
	/// selected mailbox, and returns to the authenticated state from the selected state.
	pub fn close(&mut self) -> Result<()> {
		self.run_command_and_check_ok("CLOSE")
	}

	/// Copy copies the specified message to the end of the specified destination mailbox.
	pub fn copy(&mut self, sequence_set: &str, mailbox_name: &str) -> Result<()> {
		self.run_command_and_check_ok(&format!("COPY {} {}", sequence_set, mailbox_name).to_string())
	}

	pub fn run_command_and_check_ok(&mut self, command: &str) -> Result<()> {
		match self.run_command(command) {
			Ok(lines) => self.parse_response_ok(lines),
			Err(e) => Err(e)
		}
	}

	pub fn run_command(&mut self, untagged_command: &str) -> Result<Vec<String>> {
		let command = self.create_command(untagged_command.to_string());

		match self.stream.write_fmt(format_args!("{}", &*command)) {
			Ok(_) => (),
			Err(_) => return Err(Error::new(ErrorKind::Other, "Failed to write")),
		};

		self.read_response()
	}

	fn parse_response_ok(&mut self, lines: Vec<String>) -> Result<()> {
		let ok_regex = match Regex::new(r"^([a-zA-Z0-9]+) ([a-zA-Z0-9]+)(.*)") {
    		Ok(re) => re,
    		Err(err) => panic!("{}", err),
		};
		let last_line = lines.last().unwrap();

		for cap in ok_regex.captures_iter(last_line) {
			let response_type = cap.at(2).unwrap_or("");
			if response_type == "OK" {
				return Ok(());
			}
		}

		return Err(Error::new(ErrorKind::Other, format!("Invalid Response: {}", last_line).to_string()));
	}

	fn read_response(&mut self) -> Result<Vec<String>> {
		//Carriage return
		let cr = 0x0d;
		//Line Feed
		let lf = 0x0a;
		let mut found_tag_line = false;
		let start_str = format!("{}{} ", TAG_PREFIX, self.tag);
		let mut lines: Vec<String> = Vec::new();

		while !found_tag_line {
			let mut line_buffer: Vec<u8> = Vec::new();
			while line_buffer.len() < 2 || (line_buffer[line_buffer.len()-1] != lf && line_buffer[line_buffer.len()-2] != cr) {
					let byte_buffer: &mut [u8] = &mut [0];
					match self.stream.read(byte_buffer) {
						Ok(_) => {},
						Err(_) => return Err(Error::new(ErrorKind::Other, "Failed to read the response")),
					}
					line_buffer.push(byte_buffer[0]);
			}

			let line = String::from_utf8(line_buffer).unwrap();

			lines.push(line.clone());

			if (&*line).starts_with(&*start_str) {
				found_tag_line = true;
			}
		}

		Ok(lines)
	}

	fn read_greeting(&mut self) -> Result<()> {
		//Carriage return
		let cr = 0x0d;
		//Line Feed
		let lf = 0x0a;

		let mut line_buffer: Vec<u8> = Vec::new();
		while line_buffer.len() < 2 || (line_buffer[line_buffer.len()-1] != lf && line_buffer[line_buffer.len()-2] != cr) {
				let byte_buffer: &mut [u8] = &mut [0];
				match self.stream.read(byte_buffer) {
					Ok(_) => {},
					Err(_) => return Err(Error::new(ErrorKind::Other, "Failed to read the response")),
				}
				line_buffer.push(byte_buffer[0]);
		}

		Ok(())
	}

	fn create_command(&mut self, command: String) -> String {
		self.tag += 1;
		let command = format!("{}{} {}\r\n", TAG_PREFIX, self.tag, command);
		return command;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use super::INITIAL_TAG;
	use super::super::mock_stream::MockStream;

	fn create_client_with_mock_stream(mock_stream: MockStream) -> Client<MockStream> {
		Client {
			stream: mock_stream,
			tag: INITIAL_TAG
		}
	}

	#[test]
	fn read_response() {
		let response = "a0 OK Logged in.\r\n";
		let expected_response: Vec<String> = vec![response.to_string()];
		let mock_stream = MockStream::new(response.as_bytes().to_vec());
		let mut client = create_client_with_mock_stream(mock_stream);
		let actual_response = client.read_response().unwrap();
		assert!(expected_response == actual_response, "expected response doesn't equal actual");
	}

	#[test]
	fn read_greeting() {
		let greeting = "* OK Dovecot ready.\r\n";
		let mock_stream = MockStream::new(greeting.as_bytes().to_vec());
		let mut client = create_client_with_mock_stream(mock_stream);
		client.read_greeting().unwrap();
	}

	#[test]
	fn create_command() {
		let base_command = "CHECK";
		let mock_stream = MockStream::new(Vec::new());
		let mut imap_stream = create_client_with_mock_stream(mock_stream);

		let expected_command = format!("a1 {}\r\n", base_command);
		let command = imap_stream.create_command(String::from(base_command));
		assert!(command == expected_command, "expected command doesn't equal actual command");

		let expected_command2 = format!("a2 {}\r\n", base_command);
		let command2 = imap_stream.create_command(String::from(base_command));
		assert!(command2 == expected_command2, "expected command doesn't equal actual command");
	}

	#[test]
	fn check() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK CHECK completed\r\n".to_vec();
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.check().unwrap();
		assert!(client.stream.written_buf == b"a1 CHECK\r\n".to_vec(), "Invalid close command");
	}

	#[test]
	fn close() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK CLOSE completed\r\n".to_vec();
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.close().unwrap();
		assert!(client.stream.written_buf == b"a1 CLOSE\r\n".to_vec(), "Invalid close command");
	}
}
