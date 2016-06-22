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
