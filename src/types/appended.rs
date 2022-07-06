use imap_proto::UidSetMember;
use std::fmt;

/// Meta-information about a message, as returned by
/// [`APPEND`](https://tools.ietf.org/html/rfc3501#section-6.3.11).
/// Note that `APPEND` only returns any data if certain extensions are enabled,
/// for example [`UIDPLUS`](https://tools.ietf.org/html/rfc4315).
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Appended {
    /// The unique identifier validity value of the mailbox that the message was appended to.
    /// See [`Uid`] for more details. Only present if server supports [`UIDPLUS`](https://tools.ietf.org/html/rfc4315).
    pub uid_validity: Option<u32>,

    /// The unique identifier value of the messages that were appended.
    /// Only present if server supports [`UIDPLUS`](https://tools.ietf.org/html/rfc4315).
    /// Contains only a single value unless the [`MULTIAPPEND`](https://tools.ietf.org/html/rfc3502) extension
    /// was used to upload multiple messages.
    pub uids: Option<Vec<UidSetMember>>,
}

#[allow(clippy::derivable_impls)]
impl Default for Appended {
    fn default() -> Appended {
        Appended {
            uid_validity: None,
            uids: None,
        }
    }
}

impl fmt::Display for Appended {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "uid_validity: {:?}, uids: {:?}",
            self.uid_validity, self.uids,
        )
    }
}
