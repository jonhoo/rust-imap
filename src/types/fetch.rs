use super::{Flag, Seq, Uid};

/// An IMAP [`FETCH` response](https://tools.ietf.org/html/rfc3501#section-7.4.2) that contains
/// data about a particular message. This response occurs as the result of a `FETCH` or `STORE`
/// command, as well as by unilateral server decision (e.g., flag updates).
#[derive(Debug, Eq, PartialEq)]
pub struct Fetch {
    /// The ordinal number of this message in its containing mailbox.
    pub message: Seq,

    /// A number expressing the unique identifier of the message.
    pub uid: Option<Uid>,

    // Note that none of these fields are *actually* 'static. Rather, they are tied to the lifetime
    // of the `ZeroCopy` that contains this `Name`. That's also why they can't be public -- we can
    // only return them with a lifetime tied to self.
    pub(crate) flags: Vec<Flag<'static>>,
    pub(crate) rfc822_header: Option<&'static [u8]>,
    pub(crate) rfc822: Option<&'static [u8]>,
    pub(crate) body: Option<&'static [u8]>,
}

impl Fetch {
    /// A list of flags that are set for this message.
    pub fn flags(&self) -> &[Flag] {
        &self.flags[..]
    }

    /// The bytes that make up the RFC822 header of this message, if `RFC822.HEADER` was included
    /// in the flags argument to `FETCH`.
    pub fn rfc822_header(&self) -> Option<&[u8]> {
        self.rfc822_header
    }

    /// The entire body of this message, if `RFC822` was included in the flags argument to `FETCH`.
    /// The bytes SHOULD be interpreted by the client according to the content transfer encoding,
    /// body type, and subtype.
    pub fn rfc822(&self) -> Option<&[u8]> {
        self.rfc822
    }

    /// An [MIME-IMB](https://tools.ietf.org/html/rfc2045) representation of this message, included
    /// if `BODY` was included in the flags argument to `FETCH`. See also the documentation for
    /// `BODYSTRUCTURE` in the documentation for [`FETCH
    /// responses`](https://tools.ietf.org/html/rfc3501#section-7.4.2).
    pub fn body(&self) -> Option<&[u8]> {
        self.body
    }
}
