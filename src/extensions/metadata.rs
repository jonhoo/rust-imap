//! Adds support for the IMAP METADATA extension specified in [RFC
//! 5464](https://tools.ietf.org/html/rfc5464).
//!
//! Mailboxes or the server as a whole may have zero or more annotations associated with them. An
//! annotation contains a uniquely named entry, which has a value. Annotations can be added to
//! mailboxes when a mailbox name is provided as the first argument to
//! [`set_metadata`](Session::set_metadata), or to the server as a whole when the first argument is
//! `None`.
//!
//! For example, a general comment being added to a mailbox may have an entry name of "/comment"
//! and a value of "Really useful mailbox".

use crate::client::*;
use crate::error::{Error, ParseError, Result};
use crate::parse::try_handle_unilateral;
use crate::types::*;
use imap_proto::types::{MailboxDatum, Metadata, Response, ResponseCode};
use std::collections::VecDeque;
use std::io::{Read, Write};

// for intra-doc links
#[allow(unused_imports)]
use crate::error::No;

trait CmdListItemFormat {
    fn format_as_cmd_list_item(&self, item_index: usize) -> Result<String>;
}

impl CmdListItemFormat for Metadata {
    fn format_as_cmd_list_item(&self, item_index: usize) -> Result<String> {
        let synopsis = "SETMETADATA";
        Ok(format!(
            "{} {}",
            validate_str(
                synopsis,
                format!("entry#{}", item_index + 1),
                self.entry.as_str()
            )?,
            self.value
                .as_ref()
                .map(|v| validate_str(synopsis, format!("value#{}", item_index + 1), v.as_str()))
                .unwrap_or_else(|| Ok("NIL".to_string()))?
        ))
    }
}

/// Represents variants of the `DEPTH` parameter for the `GETMETADATA` command.
///
/// When a non-zero depth is specified with the `GETMETADATA` command, it extends the list of entry
/// values returned by the server. For each entry name specified in the `GETMETADATA` command, the
/// server returns the value of the specified entry name (if it exists), plus all entries below the
/// entry name up to the specified `DEPTH`.
///
/// See also [RFC 5464, section 4.2.2](https://tools.ietf.org/html/rfc5464#section-4.2.2).
#[derive(Debug, Copy, Clone)]
pub enum MetadataDepth {
    /// No entries below the specified entry are returned.
    Zero,
    /// Only entries immediately below the specified entry are returned.
    ///
    /// Thus, a depth of one for an entry `/a` will match `/a` as well as its children entries
    /// (e.g., `/a/b`), but will not match grandchildren entries (e.g., `/a/b/c`).
    One,
    /// All entries below the specified entry are returned
    Infinity,
}

impl Default for MetadataDepth {
    fn default() -> Self {
        Self::Zero
    }
}

impl MetadataDepth {
    fn depth_str<'a>(self) -> &'a str {
        match self {
            MetadataDepth::Zero => "0",
            MetadataDepth::One => "1",
            MetadataDepth::Infinity => "infinity",
        }
    }
}

fn parse_metadata<'a>(
    mut lines: &'a [u8],
    unsolicited: &'a mut VecDeque<UnsolicitedResponse>,
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
                        if let Some(unhandled) = try_handle_unilateral(resp, unsolicited) {
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
    /// Retrieve server or mailbox annotations.
    ///
    /// This uses the `GETMETADATA` command defined in the METADATA extension of the IMAP protocol.
    /// See [RFC 5464, section 4.2](https://tools.ietf.org/html/rfc5464#section-4.2) for more
    /// details. Server support for the extension is indicated by the `METADATA` capability.
    ///
    /// When the mailbox name is empty, this command retrieves server annotations. Otherwise,
    /// this command retrieves annotations on the specified mailbox. If the `METADATA-SERVER`
    /// capability is present, server metadata is supported, but not mailbox metadata.
    ///
    /// The `entries` list specifies which annotations should be fetched. The RFC defines a number
    /// of standard names in [Section 3.2.1](https://tools.ietf.org/html/rfc5464#section-3.2.1):
    ///
    /// - Server entries (when `mailbox` is `None`):
    ///   - `/shared/comment`: A comment or note that is associated with the server and that is
    ///     shared with authorized users of the server.
    ///   - `/shared/admin`: Indicates a method for contacting the server administrator. The value
    ///     MUST be a URI (e.g., a `mailto:` or `tel:` URL). This entry is always read-only --
    ///     clients cannot change it. It is visible to authorized users of the system.
    ///   - `/shared/vendor/<vendor-token>`: Defines the top level of shared entries associated
    ///     with the server, as created by a particular product of some vendor. This entry can be
    ///     used by vendors to provide server- or client-specific annotations. The vendor-token
    ///     MUST be registered with IANA, using the Application Configuration Access Protocol
    ///     (ACAP) [RFC2244] vendor subtree registry.
    ///   - `/private/vendor/<vendor-token>`: Defines the top level of private entries associated
    ///     with the server, as created by a particular product of some vendor. This entry can be
    ///     used by vendors to provide server- or client-specific annotations. The vendor-token
    ///     MUST be registered with IANA, using the ACAP [RFC2244] vendor subtree registry.
    /// - Mailbox entries (when `mailbox` is `Some`):
    ///   - `/shared/comment`: Defines a shared comment or note associated with a mailbox.
    ///   - `/private/comment`: Defines a private (per-user) comment or note associated with a
    ///     mailbox.
    ///   - `/shared/vendor/<vendor-token>`: Defines the top level of shared entries associated
    ///     with a specific mailbox, as created by a particular product of some vendor.  This entry
    ///     can be used by vendors to provide client-specific annotations.  The vendor-token MUST
    ///     be registered with IANA, using the ACAP [RFC2244] vendor subtree registry.
    ///   - `/private/vendor/<vendor-token>`: Defines the top level of private entries associated
    ///     with a specific mailbox, as created by a particular product of some vendor.  This entry
    ///     can be used by vendors to provide client- specific annotations.  The vendor-token MUST
    ///     be registered with IANA, using the ACAP [RFC2244] vendor subtree registry.
    ///
    /// [RFC2244]: https://tools.ietf.org/html/rfc2244
    ///
    /// The `depth` argument dictates whether metadata on children of the requested entity are
    /// returned. See [`MetadataDepth`] for details
    ///
    /// When `maxsize` is specified, it restricts which entry values are returned by the server.
    /// Only entries that are less than or equal in octet size to the specified `maxsize` are
    /// returned. If there are any entries with values larger than `maxsize`, this method also
    /// returns the size of the biggest entry requested by the client that exceeded `maxsize`.
    pub fn get_metadata(
        &mut self,
        mailbox: Option<&str>,
        entries: &[impl AsRef<str>],
        depth: MetadataDepth,
        maxsize: Option<usize>,
    ) -> Result<(Vec<Metadata>, Option<u64>)> {
        let synopsis = "GETMETADATA";
        let v: Vec<String> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| validate_str(synopsis, format!("entry#{}", i + 1), e.as_ref()))
            .collect::<Result<_>>()?;
        let s = v.as_slice().join(" ");
        let mut command = format!("GETMETADATA (DEPTH {}", depth.depth_str());

        if let Some(size) = maxsize {
            command.push_str(format!(" MAXSIZE {}", size).as_str());
        }

        command.push_str(
            format!(
                ") {} ({})",
                mailbox
                    .map(|mbox| validate_str(synopsis, "mailbox", mbox))
                    .unwrap_or_else(|| Ok("\"\"".to_string()))?,
                s
            )
            .as_str(),
        );
        let (lines, ok) = self.run(command)?;
        let mut unsolicited_responses = self.all_unsolicited().collect();
        let meta = parse_metadata(&lines[..ok], &mut unsolicited_responses)?;
        let missed = if maxsize.is_some() {
            if let Ok((_, Response::Done { code, .. })) =
                imap_proto::parser::parse_response(&lines[ok..])
            {
                match code {
                    None => None,
                    Some(ResponseCode::MetadataLongEntries(v)) => Some(v),
                    Some(_) => None,
                }
            } else {
                unreachable!("already parsed as Done by Client::run");
            }
        } else {
            None
        };
        Ok((meta, missed))
    }

    /// Set annotations.
    ///
    /// This command sets the specified list of entries by adding or replacing the specified values
    /// provided, on the specified existing mailboxes or on the server (if the mailbox argument is
    /// `None`). Clients can use `None` for the value of entries it wants to remove.
    ///
    /// If the server is unable to set an annotation because the size of its value is too large,
    /// this command will fail with a [`Error::No`] and its [status code](No::code) will be
    /// [`ResponseCode::MetadataMaxSize`] where the contained value is the maximum octet count that
    /// the server is willing to accept.
    ///
    /// If the server is unable to set a new annotation because the maximum number of allowed
    /// annotations has already been reached, this command will fail with an [`Error::No`] and its
    /// [status code](No::code) will be [`ResponseCode::MetadataTooMany`].
    ///
    /// If the server is unable to set a new annotation because it does not support private
    /// annotations on one of the specified mailboxes, you guess it, you'll get an [`Error::No`] with
    /// a [status code](No::code) of [`ResponseCode::MetadataNoPrivate`].
    ///
    /// When any one annotation fails to be set and [`Error::No`] is returned, the server will not
    /// change the values for other annotations specified.
    ///
    /// See [RFC 5464, section 4.3](https://tools.ietf.org/html/rfc5464#section-4.3)
    pub fn set_metadata(&mut self, mbox: impl AsRef<str>, annotations: &[Metadata]) -> Result<()> {
        let v: Vec<String> = annotations
            .iter()
            .enumerate()
            .map(|(i, metadata)| metadata.format_as_cmd_list_item(i))
            .collect::<Result<_>>()?;
        let s = v.as_slice().join(" ");
        let command = format!(
            "SETMETADATA {} ({})",
            validate_str("SETMETADATA", "mailbox", mbox.as_ref())?,
            s
        );
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
        let r = session.get_metadata(
            None,
            &["/shared/vendor/vendor.coi", "/shared/comment"],
            MetadataDepth::Infinity,
            Option::None,
        );

        match r {
            Ok((v, missed)) => {
                assert_eq!(missed, None);
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

    use crate::client::testutils::assert_validation_error_session;

    #[test]
    fn test_getmetadata_validation_entry1() {
        assert_validation_error_session(
            |mut session| {
                session.get_metadata(
                    None,
                    &[
                        "/shared/vendor\n/vendor.coi",
                        "/shared/comment",
                        "/some/other/entry",
                    ],
                    MetadataDepth::Infinity,
                    None,
                )
            },
            "GETMETADATA",
            "entry#1",
            '\n',
        )
    }

    #[test]
    fn test_getmetadata_validation_entry2() {
        assert_validation_error_session(
            |mut session| {
                session.get_metadata(
                    Some("INBOX"),
                    &["/shared/vendor/vendor.coi", "/\rshared/comment"],
                    MetadataDepth::Infinity,
                    None,
                )
            },
            "GETMETADATA",
            "entry#2",
            '\r',
        )
    }

    #[test]
    fn test_getmetadata_validation_mailbox() {
        assert_validation_error_session(
            |mut session| {
                session.get_metadata(
                    Some("INB\nOX"),
                    &["/shared/vendor/vendor.coi", "/shared/comment"],
                    MetadataDepth::Infinity,
                    None,
                )
            },
            "GETMETADATA",
            "mailbox",
            '\n',
        );
    }

    #[test]
    fn test_setmetadata_validation_mailbox() {
        assert_validation_error_session(
            |mut session| {
                session.set_metadata(
                    "INB\nOX",
                    &[
                        Metadata {
                            entry: "/shared/vendor/vendor.coi".to_string(),
                            value: None,
                        },
                        Metadata {
                            entry: "/shared/comment".to_string(),
                            value: Some("value".to_string()),
                        },
                    ],
                )
            },
            "SETMETADATA",
            "mailbox",
            '\n',
        );
    }

    #[test]
    fn test_setmetadata_validation_entry1() {
        assert_validation_error_session(
            |mut session| {
                session.set_metadata(
                    "INBOX",
                    &[
                        Metadata {
                            entry: "/shared/\nvendor/vendor.coi".to_string(),
                            value: None,
                        },
                        Metadata {
                            entry: "/shared/comment".to_string(),
                            value: Some("value".to_string()),
                        },
                    ],
                )
            },
            "SETMETADATA",
            "entry#1",
            '\n',
        );
    }

    #[test]
    fn test_setmetadata_validation_entry2_key() {
        assert_validation_error_session(
            |mut session| {
                session.set_metadata(
                    "INBOX",
                    &[
                        Metadata {
                            entry: "/shared/vendor/vendor.coi".to_string(),
                            value: None,
                        },
                        Metadata {
                            entry: "/shared\r/comment".to_string(),
                            value: Some("value".to_string()),
                        },
                    ],
                )
            },
            "SETMETADATA",
            "entry#2",
            '\r',
        );
    }

    #[test]
    fn test_setmetadata_validation_entry2_value() {
        assert_validation_error_session(
            |mut session| {
                session.set_metadata(
                    "INBOX",
                    &[
                        Metadata {
                            entry: "/shared/vendor/vendor.coi".to_string(),
                            value: None,
                        },
                        Metadata {
                            entry: "/shared/comment".to_string(),
                            value: Some("va\nlue".to_string()),
                        },
                    ],
                )
            },
            "SETMETADATA",
            "value#2",
            '\n',
        );
    }

    #[test]
    fn test_setmetadata_validation_entry() {
        assert_validation_error_session(
            |mut session| {
                session.set_metadata(
                    "INBOX",
                    &[Metadata {
                        entry: "/shared/\nvendor/vendor.coi".to_string(),
                        value: None,
                    }],
                )
            },
            "SETMETADATA",
            "entry#1",
            '\n',
        );
    }
}
