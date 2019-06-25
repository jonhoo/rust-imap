//! Adds support for the IMAP ENABLE command specificed in [RFC
//! 5464](https://tools.ietf.org/html/rfc5464).

use crate::client::*;
use crate::error::{Error, ParseError, Result};
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

/// Represents variants of DEPTH parameters for GETMETADATA command
#[derive(Debug, Copy, Clone)]
pub enum MetadataDepth {
    /// Depth 0 for get metadata
    Zero,
    /// Depth 1 for get metadata
    One,
    /// Depth infinity for get metadata
    Inf,
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

        match imap_proto::parse_response(lines) {
            Ok((rest, resp)) => {
                lines = rest;
                match resp {
                    Response::MailboxData(MailboxDatum::MetadataSolicited {
                        mailbox: _,
                        mut values,
                    }) => {
                        res.append(&mut values);
                    }
                    Response::MailboxData(MailboxDatum::MetadataUnsolicited {
                        mailbox,
                        values,
                    }) => {
                        unsolicited
                            .send(UnsolicitedResponse::Metadata {
                                mailbox: mailbox.to_string(),
                                metadata_entries: values.iter().map(|s| s.to_string()).collect(),
                            })
                            .unwrap();
                    }
                    _ => {}
                }
            }
            Err(_) => {
                return Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
            }
        }
    }
}

/// Sends GETMETADATA command to the server and returns the list of entries and their values.
pub fn get_metadata<'a, S: AsRef<str>, T: Read + Write>(
    session: &'a mut Session<T>,
    mbox: S,
    entries: &[S],
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
    session
        .run_command_and_read_response(command)
        .and_then(|lines| parse_metadata(&lines[..], &mut session.unsolicited_responses_tx))
}

/// Sends SETMETADATA command to the server and checks if it was executed successfully.
pub fn set_metadata<'a, S: AsRef<str>, T: Read + Write>(
    session: &'a mut Session<T>,
    mbox: S,
    keyval: &[Metadata],
) -> Result<()> {
    let v: Vec<String> = keyval
        .iter()
        .map(|metadata| metadata.format_as_cmd_list_item())
        .collect();
    let s = v.as_slice().join(" ");
    let command = format!("SETMETADATA {} ({})", validate_str(mbox.as_ref())?, s);
    session.run_command_and_check_ok(command)
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
