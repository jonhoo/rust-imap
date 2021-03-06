//! Adds support for the IMAP METADATA extension specificed in [RFC
//! 5464](https://tools.ietf.org/html/rfc5464).

use crate::client::*;
use crate::error::{Error, ParseError, Result};
use crate::parse::handle_unilateral;
use crate::types::*;
use imap_proto::types::{MailboxDatum, Metadata, Response};
use std::io::{Read, Write};
use std::sync::mpsc;

trait CmdListItemFormat {
    fn format_as_cmd_list_item(&self) -> String;
}

impl CmdListItemFormat for Metadata {
    fn format_as_cmd_list_item(&self) -> String {
        format!(
            "{} {}",
            validate_str(self.entry.as_str()).unwrap(),
            self.value
                .as_ref()
                .map(|v| validate_str(v.as_str()).unwrap())
                .unwrap_or("NIL".to_string())
        )
    }
}

/// Represents variants of DEPTH parameters for GETMETADATA command.
///  "0" - no entries below the specified entry are returned
///  "1" - only entries immediately below the specified entry are returned
///  "infinity" -  all entries below the specified entry are returned
/// See [RFC 5464, section 4.2.2](https://tools.ietf.org/html/rfc5464#section-4.2.2)
#[derive(Debug, Copy, Clone)]
pub enum MetadataDepth {
    /// Depth 0 for get metadata
    Zero,
    /// Depth 1 for get metadata
    One,
    /// Depth infinity for get metadata
    Inf,
}

impl Default for MetadataDepth {
    fn default() -> Self {
        Self::Zero
    }
}

impl MetadataDepth {
    fn depth_str<'a>(self) -> &'a str {
        match self {
            MetadataDepth::Zero => return "0",
            MetadataDepth::One => return "1",
            MetadataDepth::Inf => return "infinity",
        }
    }
}

fn parse_metadata<'a>(
    mut lines: &'a [u8],
    unsolicited: &'a mut mpsc::Sender<UnsolicitedResponse>,
) -> Result<Vec<Metadata>> {
    let mut res: Vec<Metadata> = Vec::new();
    loop {
        if lines.is_empty() {
            break Ok(res);
        }

        match imap_proto::parser::parse_response(lines) {
            Ok((rest, resp)) => {
                lines = rest;
                match resp {
                    Response::MailboxData(MailboxDatum::MetadataSolicited {
                        mailbox: _,
                        mut values,
                    }) => {
                        res.append(&mut values);
                    }
                    _ => {
                        if let Some(unhandled) = handle_unilateral(resp, unsolicited) {
                            break Err(unhandled.into());
                        }
                    }
                }
            }
            Err(_) => {
                return Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
            }
        }
    }
}

impl<T: Read + Write> Session<T> {
    /// Sends GETMETADATA command of the METADATA extension to IMAP protocol
    /// to the server and returns the list of entries and their values.
    /// Server support for the extension is indicated by METADATA capability.
    /// @param mbox mailbox name. When the mailbox name is the empty string, this command retrieves server annotations. When the mailbox name is not empty, this command retrieves annotations on the specified mailbox.
    /// @param entries list of metadata entries to be retrieved.
    /// @param depth GETMETADATA DEPTH option, specifies if children entries are to be retrieved as well.
    /// @param maxside GETMETADATA MAXSIZE option. When the MAXSIZE option is specified with the GETMETADATA command, it restricts which entry values are returned by the server. Only entry values that are less than or equal in octet size to the specified MAXSIZE limit are returned.
    /// See [RFC 5464, section 4.2](https://tools.ietf.org/html/rfc5464#section-4.2) for more details.
    pub fn get_metadata(
        &mut self,
        mbox: impl AsRef<str>,
        entries: &[impl AsRef<str>],
        depth: MetadataDepth,
        maxsize: Option<usize>,
    ) -> Result<Vec<Metadata>> {
        let v: Vec<String> = entries
            .iter()
            .map(|e| validate_str(e.as_ref()).unwrap())
            .collect();
        let s = v.as_slice().join(" ");
        let mut command = format!("GETMETADATA (DEPTH {}", depth.depth_str());

        match maxsize {
            Some(size) => {
                command.push_str(format!(" MAXSIZE {}", size).as_str());
            }
            _ => {}
        }

        command.push_str(format!(") {} ({})", validate_str(mbox.as_ref()).unwrap(), s).as_str());
        self.run_command_and_read_response(command)
            .and_then(|lines| parse_metadata(&lines[..], &mut self.unsolicited_responses_tx))
    }

    /// Sends SETMETADATA command of the METADATA extension to IMAP protocol
    /// to the server and checks if it was executed successfully.
    /// Server support for the extension is indicated by METADATA capability.
    /// @param mbox mailbox name. When the mailbox name is the empty string, this command sets server annotations. When the mailbox name is not empty, this command sets annotations on the specified mailbox.
    /// @param keyvl list of entry value pairs to be set.
    /// See [RFC 5464, section 4.3](https://tools.ietf.org/html/rfc5464#section-4.3)
    pub fn set_metadata(&mut self, mbox: impl AsRef<str>, keyval: &[Metadata]) -> Result<()> {
        let v: Vec<String> = keyval
            .iter()
            .map(|metadata| metadata.format_as_cmd_list_item())
            .collect();
        let s = v.as_slice().join(" ");
        let command = format!("SETMETADATA {} ({})", validate_str(mbox.as_ref())?, s);
        self.run_command_and_check_ok(command)
    }
}

#[cfg(test)]
mod tests {
    use crate::extensions::metadata::*;
    use crate::mock_stream::MockStream;
    use crate::*;

    #[test]
    fn test_getmetadata() {
        let response = "a1 OK Logged in.\r\n* METADATA \"\" (/shared/vendor/vendor.coi/a {3}\r\nAAA /shared/vendor/vendor.coi/b {3}\r\nBBB /shared/vendor/vendor.coi/c {3}\r\nCCC)\r\na2 OK GETMETADATA Completed\r\n";
        let mock_stream = MockStream::new(response.as_bytes().to_vec());
        let client = Client::new(mock_stream);
        let mut session = client.login("testuser", "pass").unwrap();
        let r = get_metadata(
            &mut session,
            "",
            &["/shared/vendor/vendor.coi", "/shared/comment"],
            MetadataDepth::Inf,
            Option::None,
        );

        match r {
            Ok(v) => {
                assert_eq!(v.len(), 3);
                assert_eq!(v[0].entry, "/shared/vendor/vendor.coi/a");
                assert_eq!(v[0].value.as_ref().expect("None is not expected"), "AAA");
                assert_eq!(v[1].entry, "/shared/vendor/vendor.coi/b");
                assert_eq!(v[1].value.as_ref().expect("None is not expected"), "BBB");
                assert_eq!(v[2].entry, "/shared/vendor/vendor.coi/c");
                assert_eq!(v[2].value.as_ref().expect("None is not expected"), "CCC");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}
