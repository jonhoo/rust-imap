use std::net::{TcpStream, ToSocketAddrs};
use openssl::ssl::{SslContext, SslStream};
use std::io::{Error, ErrorKind, Read, Result, Write};
use regex::Regex;

static TAG_PREFIX: &'static str = "a";
const INITIAL_TAG: u32 = 1;

/// Stream to interface with the IMAP server. This interface is only for the command stream.
pub struct IMAPStream<T> {
	stream: T,
	tag: u32
}

pub struct IMAPMailbox {
	pub flags: String,
	pub exists: u32,
	pub recent: u32,
	pub unseen: Option<u32>,
	pub permanent_flags: Option<String>,
	pub uid_next: Option<u32>,
	pub uid_validity: Option<u32>
}

impl IMAPStream<TcpStream> {
	///
	pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<IMAPStream<TcpStream>> {
		match TcpStream::connect(addr) {
			Ok(stream) => {
				let mut socket = IMAPStream { stream: stream, tag: INITIAL_TAG};

				try!(socket.read_greeting());
				Ok(socket)
			},
			Err(e) => Err(e)
		}
	}
}

impl IMAPStream<SslStream<TcpStream>> {
	///
	pub fn secure_connect<A: ToSocketAddrs>(addr: A, ssl_context: SslContext) -> Result<IMAPStream<SslStream<TcpStream>>> {
		match TcpStream::connect(addr) {
			Ok(stream) => {
				let mut socket = IMAPStream { stream: SslStream::connect(&ssl_context, stream).unwrap(), tag: INITIAL_TAG};

				try!(socket.read_greeting());
				Ok(socket)
			},
			Err(e) => Err(e)
		}
	}
}

impl<T: Read+Write> IMAPStream<T> {

	/// Log in to the IMAP server.
	pub fn login(&mut self, username: & str, password: & str) -> Result<()> {
		self.run_command_and_check_ok(&format!("LOGIN {} {}", username, password).to_string())
	}

	/// Selects a mailbox
	pub fn select(&mut self, mailbox_name: &str) -> Result<IMAPMailbox> {
		match self.run_command(&format!("SELECT {}", mailbox_name).to_string()) {
			Ok(lines) => self.parse_select_or_examine(lines),
			Err(e) => Err(e)
		}
	}

	fn parse_select_or_examine(&mut self, lines: Vec<String>) -> Result<IMAPMailbox> {
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

		let mut mailbox = IMAPMailbox{
			flags: "".to_string(),
			exists: 0,
			recent: 0,
			unseen: None,
			permanent_flags: None,
			uid_next: None,
			uid_validity: None
		};

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

		return Ok(mailbox);
	}

	/// Examine is identical to Select, but the selected mailbox is identified as read-only
	pub fn examine(&mut self, mailbox_name: &str) -> Result<IMAPMailbox> {
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
		self.run_command_and_check_ok("CHECK")
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

		let ret = match self.read_response() {
			Ok(lines) => Ok(lines),
			Err(_) => Err(Error::new(ErrorKind::Other, "Failed to read")),
		};

		self.tag += 1;

		return ret;
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
		let command = format!("{}{} {}\r\n", TAG_PREFIX, self.tag, command);
		return command;
	}
}
