use super::{Flag, Seq};

/// re-exported from imap_proto;
pub use imap_proto::AttributeValue;
pub use imap_proto::ResponseCode;
pub use imap_proto::StatusAttribute;
use imap_proto::{MailboxDatum, Response, Status};

/// Responses that the server sends that are not related to the current command.
/// [RFC 3501](https://tools.ietf.org/html/rfc3501#section-7) states that clients need to be able
/// to accept any response at any time.
///
/// Not all possible responses are explicitly enumerated here because in practice only
/// some types of responses are delivered as unsolicited responses. If you encounter an
/// unsolicited response in the wild that is not handled here, please
/// [open an issue](https://github.com/jonhoo/rust-imap/issues) and let us know!
///
/// Note that `Recent`, `Exists` and `Expunge` responses refer to the currently `SELECT`ed folder,
/// so the user must take care when interpreting these.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsolicitedResponse {
    /// An unsolicited `BYE` response.
    ///
    /// The `BYE` response may have an optional `ResponseCode` that provides additional
    /// information, per [RFC3501](https://tools.ietf.org/html/rfc3501#section-7.1.5).
    Bye {
        /// Optional response code.
        code: Option<ResponseCode<'static>>,
        /// Information text that may be presented to the user.
        information: Option<String>,
    },

    /// An unsolicited [`EXISTS` response](https://tools.ietf.org/html/rfc3501#section-7.3.1) that
    /// reports the number of messages in the mailbox. This response occurs if the size of the
    /// mailbox changes (e.g., new messages arrive).
    Exists(u32),

    /// An unsolicited [`EXPUNGE` response](https://tools.ietf.org/html/rfc3501#section-7.4.1) that
    /// reports that the specified message sequence number has been permanently removed from the
    /// mailbox.  The message sequence number for each successive message in the mailbox is
    /// immediately decremented by 1, and this decrement is reflected in message sequence numbers
    /// in subsequent responses (including other untagged `EXPUNGE` responses).
    ///
    /// The EXPUNGE response also decrements the number of messages in the mailbox; it is not
    /// necessary to send an `EXISTS` response with the new value.
    ///
    /// As a result of the immediate decrement rule, message sequence numbers that appear in a set
    /// of successive `EXPUNGE` responses depend upon whether the messages are removed starting
    /// from lower numbers to higher numbers, or from higher numbers to lower numbers.  For
    /// example, if the last 5 messages in a 9-message mailbox are expunged, a "lower to higher"
    /// server will send five untagged `EXPUNGE` responses for message sequence number 5, whereas a
    /// "higher to lower server" will send successive untagged `EXPUNGE` responses for message
    /// sequence numbers 9, 8, 7, 6, and 5.
    // TODO: the spec doesn't seem to say anything about when these may be received as unsolicited?
    Expunge(Seq),

    /// An unsolicited `FETCH` response.
    ///
    /// The server may unilaterally send `FETCH` responses, as described in
    /// [RFC3501](https://tools.ietf.org/html/rfc3501#section-7.4.2).
    Fetch {
        /// Message identifier.
        id: u32,
        /// Attribute values for this message.
        attributes: Vec<AttributeValue<'static>>,
    },

    /// An unsolicited [`FLAGS` response](https://tools.ietf.org/html/rfc3501#section-7.2.6) that
    /// identifies the flags (at a minimum, the system-defined flags) that are applicable in the
    /// mailbox. Flags other than the system flags can also exist, depending on server
    /// implementation.
    ///
    /// See [`Flag`] for details.
    // TODO: the spec doesn't seem to say anything about when these may be received as unsolicited?
    Flags(Vec<Flag<'static>>),

    /// An unsolicited METADATA response (https://tools.ietf.org/html/rfc5464#section-4.4.2)
    /// that reports a change in a server or mailbox annotation.
    Metadata {
        /// Mailbox name for which annotations were changed.
        mailbox: String,
        /// List of annotations that were changed.
        metadata_entries: Vec<String>,
    },

    /// An unsolicited `OK` response.
    ///
    /// The `OK` response may have an optional `ResponseCode` that provides additional
    /// information, per [RFC3501](https://tools.ietf.org/html/rfc3501#section-7.1.1).
    Ok {
        /// Optional response code.
        code: Option<ResponseCode<'static>>,
        /// Information text that may be presented to the user.
        information: Option<String>,
    },

    /// An unsolicited [`RECENT` response](https://tools.ietf.org/html/rfc3501#section-7.3.2)
    /// indicating the number of messages with the `\Recent` flag set.  This response occurs if the
    /// size of the mailbox changes (e.g., new messages arrive).
    ///
    /// > Note: It is not guaranteed that the message sequence
    /// > numbers of recent messages will be a contiguous range of
    /// > the highest n messages in the mailbox (where n is the
    /// > value reported by the `RECENT` response).  Examples of
    /// > situations in which this is not the case are: multiple
    /// > clients having the same mailbox open (the first session
    /// > to be notified will see it as recent, others will
    /// > probably see it as non-recent), and when the mailbox is
    /// > re-ordered by a non-IMAP agent.
    /// >
    /// > The only reliable way to identify recent messages is to
    /// > look at message flags to see which have the `\Recent` flag
    /// > set, or to do a `SEARCH RECENT`.
    Recent(u32),

    /// An unsolicited [`STATUS response`](https://tools.ietf.org/html/rfc3501#section-7.2.4).
    Status {
        /// The mailbox that this status response is for.
        mailbox: String,
        /// The attributes of this mailbox.
        attributes: Vec<StatusAttribute>,
    },

    /// An unsolicited [`VANISHED` response](https://tools.ietf.org/html/rfc7162#section-3.2.10)
    /// that reports a sequence-set of `UID`s that have been expunged from the mailbox.
    ///
    /// The `VANISHED` response is similar to the `EXPUNGE` response and can be sent wherever
    /// an `EXPUNGE` response can be sent. It can only be sent by the server if the client
    /// has enabled [`QRESYNC`](https://tools.ietf.org/html/rfc7162).
    ///
    /// The `VANISHED` response has two forms, one with the `EARLIER` tag which is used to
    /// respond to a `UID FETCH` or `SELECT/EXAMINE` command, and one without an `EARLIER`
    /// tag, which is used to announce removals within an already selected mailbox.
    ///
    /// If using `QRESYNC`, the client can fetch new, updated and deleted `UID`s in a
    /// single round trip by including the `(CHANGEDSINCE <MODSEQ> VANISHED)`
    /// modifier to the `UID SEARCH` command, as described in
    /// [RFC7162](https://tools.ietf.org/html/rfc7162#section-3.1.4). For example
    /// `UID FETCH 1:* (UID FLAGS) (CHANGEDSINCE 1234 VANISHED)` would return `FETCH`
    /// results for all `UID`s added or modified since `MODSEQ` `1234`. Deleted `UID`s
    /// will be present as a `VANISHED` response in the `Session::unsolicited_responses`
    /// channel.
    Vanished {
        /// Whether the `EARLIER` tag was set on the response
        earlier: bool,
        /// The list of `UID`s which have been removed
        uids: Vec<std::ops::RangeInclusive<u32>>,
    },
}

/// Try to convert from a `imap_proto::Response`.
///
/// Not all `Response` variants are supported - only those which
/// are known or likely to be sent by a server as a unilateral response
/// during normal operations or during an IDLE session are implented.
///
/// If the conversion fails, the input `Reponse` is returned.
impl<'a> TryFrom<Response<'a>> for UnsolicitedResponse {
    type Error = Response<'a>;

    fn try_from(response: Response<'a>) -> Result<Self, Self::Error> {
        match response {
            Response::Data {
                status: Status::Bye,
                code,
                information,
            } => Ok(UnsolicitedResponse::Bye {
                code: code.map(|c| c.into_owned()),
                information: information.map(|s| s.to_string()),
            }),
            Response::Data {
                status: Status::Ok,
                code,
                information,
            } => Ok(UnsolicitedResponse::Ok {
                code: code.map(|c| c.into_owned()),
                information: information.map(|s| s.to_string()),
            }),
            Response::Expunge(n) => Ok(UnsolicitedResponse::Expunge(n)),
            Response::Fetch(id, attributes) => Ok(UnsolicitedResponse::Fetch {
                id,
                attributes: attributes.into_iter().map(|a| a.into_owned()).collect(),
            }),
            Response::MailboxData(MailboxDatum::Exists(n)) => Ok(UnsolicitedResponse::Exists(n)),
            Response::MailboxData(MailboxDatum::Flags(flags)) => {
                Ok(UnsolicitedResponse::Flags(Flag::from_strs(flags).collect()))
            }
            Response::MailboxData(MailboxDatum::MetadataUnsolicited { mailbox, values }) => {
                Ok(UnsolicitedResponse::Metadata {
                    mailbox: mailbox.to_string(),
                    metadata_entries: values.iter().map(|s| s.to_string()).collect(),
                })
            }
            Response::MailboxData(MailboxDatum::Recent(n)) => Ok(UnsolicitedResponse::Recent(n)),
            Response::MailboxData(MailboxDatum::Status { mailbox, status }) => {
                Ok(UnsolicitedResponse::Status {
                    mailbox: mailbox.into(),
                    attributes: status,
                })
            }
            Response::Vanished { earlier, uids } => {
                Ok(UnsolicitedResponse::Vanished { earlier, uids })
            }
            _ => Err(response),
        }
    }
}
