//! Adds support for the IMAP LIST-STATUS extension specificed in [RFC
//! 5819](https://tools.ietf.org/html/rfc5819).

use crate::client::{validate_str, Session};
use crate::error::{Error, ParseError, Result};
use crate::parse::try_handle_unilateral;
use crate::types::{Mailbox, Name, UnsolicitedResponse};
use imap_proto::types::{MailboxDatum, Response, StatusAttribute};
use ouroboros::self_referencing;
use std::io::{Read, Write};
use std::slice::Iter;
use std::sync::mpsc;

/// A wrapper for one or more [`Name`] responses paired with optional [`Mailbox`] responses.
///
/// This structure represents responses to a LIST-STATUS command, as implemented in
/// [`Session::list_status`]. See [RFC 5819, section 2](https://tools.ietf.org/html/rfc5819.html#section-2).
#[self_referencing]
pub struct ExtendedNames {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) extended_names: Vec<(Name<'this>, Option<Mailbox>)>,
}

impl ExtendedNames {
    /// Parse one or more LIST-STATUS responses from a response buffer
    pub(crate) fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> core::result::Result<Self, Error> {
        ExtendedNamesTryBuilder {
            data: owned,
            extended_names_builder: |input| {
                let mut lines: &[u8] = input;
                let mut names = Vec::new();
                let mut current_name: Option<Name<'_>> = None;
                let mut current_mailbox: Option<Mailbox> = None;

                loop {
                    if lines.is_empty() {
                        if let Some(cur_name) = current_name {
                            names.push((cur_name, current_mailbox));
                        }
                        break;
                    }

                    match imap_proto::parser::parse_response(lines) {
                        Ok((
                            rest,
                            Response::MailboxData(MailboxDatum::List {
                                name_attributes,
                                delimiter,
                                name,
                            }),
                        )) => {
                            lines = rest;
                            if let Some(cur_name) = current_name {
                                names.push((cur_name, current_mailbox));
                                current_mailbox = None;
                            }
                            current_name = Some(Name {
                                attributes: name_attributes,
                                delimiter,
                                name,
                            });
                        }
                        Ok((
                            rest,
                            Response::MailboxData(MailboxDatum::Status { mailbox: _, status }),
                        )) => {
                            lines = rest;
                            let mut mb = Mailbox::default();
                            for attr in status {
                                match attr {
                                    StatusAttribute::HighestModSeq(v) => {
                                        mb.highest_mod_seq = Some(v)
                                    }
                                    StatusAttribute::Messages(v) => mb.exists = v,
                                    StatusAttribute::Recent(v) => mb.recent = v,
                                    StatusAttribute::UidNext(v) => mb.uid_next = Some(v),
                                    StatusAttribute::UidValidity(v) => mb.uid_validity = Some(v),
                                    StatusAttribute::Unseen(v) => mb.unseen = Some(v),
                                    _ => {} // needed because StatusAttribute is #[non_exhaustive]
                                }
                            }
                            current_mailbox = Some(mb);
                        }
                        Ok((rest, resp)) => {
                            lines = rest;
                            if let Some(unhandled) = try_handle_unilateral(resp, unsolicited) {
                                return Err(unhandled.into());
                            }
                        }
                        Err(_) => {
                            return Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                        }
                    }
                }

                Ok(names)
            },
        }
        .try_build()
    }

    /// Iterate over the contained elements
    pub fn iter(&self) -> Iter<'_, (Name<'_>, Option<Mailbox>)> {
        self.borrow_extended_names().iter()
    }

    /// Get the number of elements in this container.
    pub fn len(&self) -> usize {
        self.borrow_extended_names().len()
    }

    /// Return true of there are no elements in the container.
    pub fn is_empty(&self) -> bool {
        self.borrow_extended_names().is_empty()
    }

    /// Get the element at the given index
    pub fn get(&self, index: usize) -> Option<&(Name<'_>, Option<Mailbox>)> {
        self.borrow_extended_names().get(index)
    }
}

impl<T: Read + Write> Session<T> {
    /// The [extended `LIST` command](https://tools.ietf.org/html/rfc5819.html#section-2) returns
    /// a subset of names from the complete set of all names available to the client. Each name
    /// _should_ be paired with a STATUS response, though the server _may_ drop it if it encounters
    /// problems looking up the required information.
    ///
    /// This version of the command is also often referred to as `LIST-STATUS` command, as that is
    /// the name of the extension and it is a combination of the two.
    ///
    /// The `reference_name` and `mailbox_pattern` arguments have the same semantics as they do in
    /// [`Session::list`].
    ///
    /// The `data_items` argument has the same semantics as it does in [`Session::status`].
    pub fn list_status(
        &mut self,
        reference_name: Option<&str>,
        mailbox_pattern: Option<&str>,
        data_items: &str,
    ) -> Result<ExtendedNames> {
        let reference = validate_str("LIST-STATUS", "reference", reference_name.unwrap_or(""))?;
        self.run_command_and_read_response(&format!(
            "LIST {} {} RETURN (STATUS {})",
            &reference,
            mailbox_pattern.unwrap_or("\"\""),
            data_items
        ))
        .and_then(|lines| ExtendedNames::parse(lines, &mut self.unsolicited_responses_tx))
    }
}

#[cfg(test)]
mod tests {
    use imap_proto::NameAttribute;

    use super::*;

    #[test]
    fn parse_list_status_test() {
        let lines = b"\
                    * LIST () \".\" foo\r\n\
                    * STATUS foo (HIGHESTMODSEQ 122)\r\n\
                    * LIST () \".\" foo.bar\r\n\
                    * STATUS foo.bar (HIGHESTMODSEQ 132)\r\n\
                    * LIST (\\UnMarked) \".\" feeds\r\n\
                    * LIST () \".\" feeds.test\r\n\
                    * STATUS feeds.test (HIGHESTMODSEQ 757)\r\n";
        let (mut send, recv) = mpsc::channel();
        let fetches = ExtendedNames::parse(lines.to_vec(), &mut send).unwrap();
        assert!(recv.try_recv().is_err());
        assert!(!fetches.is_empty());
        assert_eq!(fetches.len(), 4);
        let (name, status) = fetches.get(0).unwrap();
        assert_eq!(&name.name, "foo");
        assert!(status.is_some());
        assert_eq!(status.as_ref().unwrap().highest_mod_seq, Some(122));
        let (name, status) = fetches.get(1).unwrap();
        assert_eq!(&name.name, "foo.bar");
        assert!(status.is_some());
        assert_eq!(status.as_ref().unwrap().highest_mod_seq, Some(132));
        let (name, status) = fetches.get(2).unwrap();
        assert_eq!(&name.name, "feeds");
        assert_eq!(name.attributes.len(), 1);
        assert_eq!(name.attributes.get(0).unwrap(), &NameAttribute::Unmarked);
        assert!(status.is_none());
        let (name, status) = fetches.get(3).unwrap();
        assert_eq!(&name.name, "feeds.test");
        assert!(status.is_some());
        assert_eq!(status.as_ref().unwrap().highest_mod_seq, Some(757));
    }
}
