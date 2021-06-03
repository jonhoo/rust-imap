use super::{Flag, Seq, Uid};
use chrono::{DateTime, FixedOffset};
use imap_proto::types::{AttributeValue, BodyStructure, Envelope, MessageSection, SectionPath};

/// Format of Date and Time as defined RFC3501.
/// See `date-time` element in [Formal Syntax](https://tools.ietf.org/html/rfc3501#section-9)
/// chapter of this RFC.
const DATE_TIME_FORMAT: &str = "%d-%b-%Y %H:%M:%S %z";

/// An IMAP [`FETCH` response](https://tools.ietf.org/html/rfc3501#section-7.4.2) that contains
/// data about a particular message. This response occurs as the result of a `FETCH` or `STORE`
/// command, as well as by unilateral server decision (e.g., flag updates).
#[derive(Debug, Eq, PartialEq)]
pub struct Fetch {
    /// The ordinal number of this message in its containing mailbox.
    pub message: Seq,

    /// A number expressing the unique identifier of the message.
    /// Only present if `UID` was specified in the query argument to `FETCH` and the server
    /// supports UIDs.
    pub uid: Option<Uid>,

    /// A number expressing the [RFC-2822](https://tools.ietf.org/html/rfc2822) size of the message.
    /// Only present if `RFC822.SIZE` was specified in the query argument to `FETCH`.
    pub size: Option<u32>,

    // Note that none of these fields are *actually* 'static. Rather, they are tied to the lifetime
    // of the `ZeroCopy` that contains this `Name`. That's also why they can't be public -- we can
    // only return them with a lifetime tied to self.
    pub(crate) fetch: Vec<AttributeValue<'static>>,
    pub(crate) flags: Vec<Flag<'static>>,
}

impl Fetch {
    /// A list of flags that are set for this message.
    pub fn flags(&self) -> &[Flag<'_>] {
        &self.flags[..]
    }

    /// The bytes that make up the header of this message, if `BODY[HEADER]`, `BODY.PEEK[HEADER]`,
    /// or `RFC822.HEADER` was included in the `query` argument to `FETCH`.
    pub fn header(&self) -> Option<&[u8]> {
        self.fetch.iter().find_map(|av| match av {
            AttributeValue::BodySection {
                section: Some(SectionPath::Full(MessageSection::Header)),
                data: Some(hdr),
                ..
            }
            | AttributeValue::Rfc822Header(Some(hdr)) => Some(&**hdr),
            _ => None,
        })
    }

    /// The bytes that make up this message, included if `BODY[]` or `RFC822` was included in the
    /// `query` argument to `FETCH`. The bytes SHOULD be interpreted by the client according to the
    /// content transfer encoding, body type, and subtype.
    pub fn body(&self) -> Option<&[u8]> {
        self.fetch.iter().find_map(|av| match av {
            AttributeValue::BodySection {
                section: None,
                data: Some(body),
                ..
            }
            | AttributeValue::Rfc822(Some(body)) => Some(&**body),
            _ => None,
        })
    }

    /// The bytes that make up the text of this message, included if `BODY[TEXT]`, `RFC822.TEXT`,
    /// or `BODY.PEEK[TEXT]` was included in the `query` argument to `FETCH`. The bytes SHOULD be
    /// interpreted by the client according to the content transfer encoding, body type, and
    /// subtype.
    pub fn text(&self) -> Option<&[u8]> {
        self.fetch.iter().find_map(|av| match av {
            AttributeValue::BodySection {
                section: Some(SectionPath::Full(MessageSection::Text)),
                data: Some(body),
                ..
            }
            | AttributeValue::Rfc822Text(Some(body)) => Some(&**body),
            _ => None,
        })
    }

    /// The envelope of this message, if `ENVELOPE` was included in the `query` argument to
    /// `FETCH`. This is computed by the server by parsing the
    /// [RFC-2822](https://tools.ietf.org/html/rfc2822) header into the component parts, defaulting
    /// various fields as necessary.
    ///
    /// The full description of the format of the envelope is given in [RFC 3501 section
    /// 7.4.2](https://tools.ietf.org/html/rfc3501#section-7.4.2).
    pub fn envelope(&self) -> Option<&Envelope<'_>> {
        self.fetch.iter().find_map(|av| match av {
            AttributeValue::Envelope(env) => Some(&**env),
            _ => None,
        })
    }

    /// Extract the bytes that makes up the given `BODY[<section>]` of a `FETCH` response.
    ///
    /// See [section 7.4.2 of RFC 3501](https://tools.ietf.org/html/rfc3501#section-7.4.2) for
    /// details.
    pub fn section(&self, path: &SectionPath) -> Option<&[u8]> {
        self.fetch.iter().find_map(|av| match av {
            AttributeValue::BodySection {
                section: Some(sp),
                data: Some(data),
                ..
            } if sp == path => Some(&**data),
            _ => None,
        })
    }

    /// Extract the `INTERNALDATE` of a `FETCH` response
    ///
    /// See [section 2.3.3 of RFC 3501](https://tools.ietf.org/html/rfc3501#section-2.3.3) for
    /// details.
    pub fn internal_date(&self) -> Option<DateTime<FixedOffset>> {
        self.fetch
            .iter()
            .find_map(|av| match av {
                AttributeValue::InternalDate(date_time) => Some(&**date_time),
                _ => None,
            })
            .and_then(
                |date_time| match DateTime::parse_from_str(date_time, DATE_TIME_FORMAT) {
                    Ok(date_time) => Some(date_time),
                    Err(_) => None,
                },
            )
    }

    /// Extract the `BODYSTRUCTURE` of a `FETCH` response
    ///
    /// See [section 2.3.6 of RFC 3501](https://tools.ietf.org/html/rfc3501#section-2.3.6) for
    /// details.
    pub fn bodystructure<'a>(&self) -> Option<&BodyStructure<'a>> {
        self.fetch.iter().find_map(|av| match av {
            AttributeValue::BodyStructure(bs) => Some(bs),
            _ => None,
        })
    }
}
