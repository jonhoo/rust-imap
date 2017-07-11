use std::fmt;

#[derive(Eq, PartialEq)]
pub struct Mailbox {
    pub flags: String,
    pub exists: u32,
    pub recent: u32,
    pub unseen: Option<u32>,
    pub permanent_flags: Option<String>,
    pub uid_next: Option<u32>,
    pub uid_validity: Option<u32>,
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
            uid_validity: None,
        }
    }
}

impl fmt::Display for Mailbox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "flags: {}, exists: {}, recent: {}, unseen: {:?}, permanent_flags: {:?},\
             uid_next: {:?}, uid_validity: {:?}",
            self.flags,
            self.exists,
            self.recent,
            self.unseen,
            self.permanent_flags,
            self.uid_next,
            self.uid_validity
        )
    }
}
