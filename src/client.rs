use std::net::{TcpStream, ToSocketAddrs};
use openssl::ssl::{SslContext, SslStream};
use std::io::{Error, ErrorKind, Read, Result, Write};

use super::mailbox::Mailbox;
use super::parse::{parse_response_ok, parse_capability, parse_select_or_examine};

static TAG_PREFIX: &'static str = "a";
const INITIAL_TAG: u32 = 0;

/// Stream to interface with the IMAP server. This interface is only for the command stream.
pub struct Client<T> {
	stream: T,
	tag: u32
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
			Ok(lines) => parse_select_or_examine(lines),
			Err(e) => Err(e)
		}
	}

	/// Examine is identical to Select, but the selected mailbox is identified as read-only
	pub fn examine(&mut self, mailbox_name: &str) -> Result<Mailbox> {
		match self.run_command(&format!("EXAMINE {}", mailbox_name).to_string()) {
			Ok(lines) => parse_select_or_examine(lines),
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
			Ok(lines) => parse_capability(lines),
			Err(e) => Err(e)
		}
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

	/// Runs a command and checks if it returns OK.
	pub fn run_command_and_check_ok(&mut self, command: &str) -> Result<()> {
		match self.run_command(command) {
			Ok(lines) => parse_response_ok(lines),
			Err(e) => Err(e)
		}
	}

	/// Runs any command passed to it.
	pub fn run_command(&mut self, untagged_command: &str) -> Result<Vec<String>> {
		let command = self.create_command(untagged_command.to_string());

		match self.stream.write_fmt(format_args!("{}", &*command)) {
			Ok(_) => (),
			Err(_) => return Err(Error::new(ErrorKind::Other, "Failed to write")),
		};

		self.read_response()
	}

	fn read_response(&mut self) -> Result<Vec<String>> {
		let mut found_tag_line = false;
		let start_str = format!("{}{} ", TAG_PREFIX, self.tag);
		let mut lines: Vec<String> = Vec::new();

		while !found_tag_line {
			match self.readline() {
				Ok(raw_data) => {
					let line = String::from_utf8(raw_data).unwrap();
					lines.push(line.clone());
					if (&*line).starts_with(&*start_str) {
						found_tag_line = true;
					}
				},
				Err(err) => return Err(err)
			}
		}

		Ok(lines)
	}

	fn read_greeting(&mut self) -> Result<()> {
		match self.readline() {
			Ok(_) => Ok(()),
			Err(err) => Err(err)
		}
	}

	fn readline(&mut self) -> Result<Vec<u8>> {
		//Carriage return
		let cr = 0x0d;
		//Line Feed
		let lf = 0x0a;

		let mut line_buffer: Vec<u8> = Vec::new();
		while line_buffer.len() < 2 || (line_buffer[line_buffer.len()-1] != lf && line_buffer[line_buffer.len()-2] != cr) {
				let byte_buffer: &mut [u8] = &mut [0];
				match self.stream.read(byte_buffer) {
					Ok(_) => {},
					Err(_) => return Err(Error::new(ErrorKind::Other, "Failed to read line")),
				}
				line_buffer.push(byte_buffer[0]);
		}
		Ok(line_buffer)
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
	use super::super::mailbox::Mailbox;

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
	#[should_panic]
	fn readline_err() {
		// TODO Check the error test
		let mock_stream = MockStream::new_err();
		let mut client = create_client_with_mock_stream(mock_stream);
		client.readline().unwrap();
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
	fn login() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK Logged in\r\n".to_vec();
		let username = "username";
		let password = "password";
		let command = format!("a1 LOGIN {} {}\r\n", username, password);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.login(username, password).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid login command");
	}

	#[test]
	fn logout() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK Logout completed.\r\n".to_vec();
		let command = format!("a1 LOGOUT\r\n");
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.logout().unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid logout command");
	}

	#[test]
	fn rename() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK RENAME completed\r\n".to_vec();
		let current_mailbox_name = "INBOX";
		let new_mailbox_name = "NEWINBOX";
		let command = format!("a1 RENAME {} {}\r\n", current_mailbox_name, new_mailbox_name);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.rename(current_mailbox_name, new_mailbox_name).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid rename command");
	}

	#[test]
	fn fetch() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK FETCH completed\r\n".to_vec();
		let sequence_set = "1";
		let query = "BODY[]";
		let command = format!("a1 FETCH {} {}\r\n", sequence_set, query);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.fetch(sequence_set, query).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid fetch command");
	}

	#[test]
	fn subscribe() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK SUBSCRIBE completed\r\n".to_vec();
		let mailbox = "INBOX";
		let command = format!("a1 SUBSCRIBE {}\r\n", mailbox);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.subscribe(mailbox).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid subscribe command");
	}

	#[test]
	fn unsubscribe() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK UNSUBSCRIBE completed\r\n".to_vec();
		let mailbox = "INBOX";
		let command = format!("a1 UNSUBSCRIBE {}\r\n", mailbox);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.unsubscribe(mailbox).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid unsubscribe command");
	}

	#[test]
	fn expunge() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK EXPUNGE completed\r\n".to_vec();
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.expunge().unwrap();
		assert!(client.stream.written_buf == b"a1 EXPUNGE\r\n".to_vec(), "Invalid expunge command");
	}

	#[test]
	fn check() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK CHECK completed\r\n".to_vec();
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.check().unwrap();
		assert!(client.stream.written_buf == b"a1 CHECK\r\n".to_vec(), "Invalid check command");
	}

	#[test]
	fn examine() {
		let response = b"* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
			* OK [PERMANENTFLAGS ()] Read-only mailbox.\r\n\
			* 1 EXISTS\r\n\
			* 1 RECENT\r\n\
			* OK [UNSEEN 1] First unseen.\r\n\
			* OK [UIDVALIDITY 1257842737] UIDs valid\r\n\
			* OK [UIDNEXT 2] Predicted next UID\r\n\
			a1 OK [READ-ONLY] Select completed.\r\n".to_vec();
		let expected_mailbox = Mailbox {
			flags: String::from("(\\Answered \\Flagged \\Deleted \\Seen \\Draft)"),
			exists: 1,
			recent: 1,
			unseen: Some(1),
			permanent_flags: None,
			uid_next: Some(2),
			uid_validity: Some(1257842737)
		};
		let mailbox_name = "INBOX";
		let command = format!("a1 EXAMINE {}\r\n", mailbox_name);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		let mailbox = client.examine(mailbox_name).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid examine command");
		assert!(mailbox == expected_mailbox, "Unexpected mailbox returned");
	}

	#[test]
	fn select() {
		let response = b"* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
			* OK [PERMANENTFLAGS ()] Read-only mailbox.\r\n\
			* 1 EXISTS\r\n\
			* 1 RECENT\r\n\
			* OK [UNSEEN 1] First unseen.\r\n\
			* OK [UIDVALIDITY 1257842737] UIDs valid\r\n\
			* OK [UIDNEXT 2] Predicted next UID\r\n\
			a1 OK [READ-ONLY] Select completed.\r\n".to_vec();
		let expected_mailbox = Mailbox {
			flags: String::from("(\\Answered \\Flagged \\Deleted \\Seen \\Draft)"),
			exists: 1,
			recent: 1,
			unseen: Some(1),
			permanent_flags: None,
			uid_next: Some(2),
			uid_validity: Some(1257842737)
		};
		let mailbox_name = "INBOX";
		let command = format!("a1 SELECT {}\r\n", mailbox_name);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		let mailbox = client.select(mailbox_name).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid select command");
		assert!(mailbox == expected_mailbox, "Unexpected mailbox returned");
	}

	#[test]
	fn capability() {
		let response = b"* CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n\
			a1 OK CAPABILITY completed\r\n".to_vec();
		let expected_capabilities = vec!["IMAP4rev1", "STARTTLS", "AUTH=GSSAPI", "LOGINDISABLED"];
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		let capabilities = client.capability().unwrap();
		assert!(client.stream.written_buf == b"a1 CAPABILITY\r\n".to_vec(), "Invalid capability command");
		assert!(capabilities == expected_capabilities, "Unexpected capabilities response");
	}

	#[test]
	fn create() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK CREATE completed\r\n".to_vec();
		let mailbox_name = "INBOX";
		let command = format!("a1 CREATE {}\r\n", mailbox_name);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.create(mailbox_name).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid create command");
	}

	#[test]
	fn delete() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK DELETE completed\r\n".to_vec();
		let mailbox_name = "INBOX";
		let command = format!("a1 DELETE {}\r\n", mailbox_name);
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.delete(mailbox_name).unwrap();
		assert!(client.stream.written_buf == command.as_bytes().to_vec(), "Invalid delete command");
	}

	#[test]
	fn noop() {
		// TODO Make sure the response was read correctly
		let response = b"a1 OK NOOP completed\r\n".to_vec();
		let mock_stream = MockStream::new(response);
		let mut client = create_client_with_mock_stream(mock_stream);
		client.noop().unwrap();
		assert!(client.stream.written_buf == b"a1 NOOP\r\n".to_vec(), "Invalid noop command");
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
