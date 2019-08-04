//! This module contains types used throughout the IMAP protocol.

pub use enumset::EnumSet;
use std::borrow::Cow;

/// From section [2.3.1.1 of RFC 3501](https://tools.ietf.org/html/rfc3501#section-2.3.1.1).
///
/// A 32-bit value assigned to each message, which when used with the unique identifier validity
/// value (see below) forms a 64-bit value that will not refer to any other message in the mailbox
/// or any subsequent mailbox with the same name forever.  Unique identifiers are assigned in a
/// strictly ascending fashion in the mailbox; as each message is added to the mailbox it is
/// assigned a higher UID than the message(s) which were added previously.  Unlike message sequence
/// numbers, unique identifiers are not necessarily contiguous.
///
/// The unique identifier of a message will not change during the session, and will generally not
/// change between sessions.  Any change of unique identifiers between sessions will be detectable
/// using the `UIDVALIDITY` mechanism discussed below.  Persistent unique identifiers are required
/// for a client to resynchronize its state from a previous session with the server (e.g.,
/// disconnected or offline access clients); this is discussed further in
/// [`IMAP-DISC`](https://tools.ietf.org/html/rfc3501#ref-IMAP-DISC).
///
/// Associated with every mailbox are two values which aid in unique identifier handling: the next
/// unique identifier value and the unique identifier validity value.
///
/// The next unique identifier value is the predicted value that will be assigned to a new message
/// in the mailbox.  Unless the unique identifier validity also changes (see below), the next
/// unique identifier value will have the following two characteristics.  First, the next unique
/// identifier value will not change unless new messages are added to the mailbox; and second, the
/// next unique identifier value will change whenever new messages are added to the mailbox, even
/// if those new messages are subsequently expunged.
///
/// > Note: The next unique identifier value is intended to provide a means for a client to
/// > determine whether any messages have been delivered to the mailbox since the previous time it
/// > checked this value.  It is not intended to provide any guarantee that any message will have
/// > this unique identifier.  A client can only assume, at the time that it obtains the next
/// > unique identifier value, that messages arriving after that time will have a UID greater than
/// > or equal to that value.
///
/// The unique identifier validity value is sent in a `UIDVALIDITY` response code in an `OK`
/// untagged response at mailbox selection time. If unique identifiers from an earlier session fail
/// to persist in this session, the unique identifier validity value will be greater than the one
/// used in the earlier session.
///
/// > Note: Ideally, unique identifiers will persist at all
/// > times.  Although this specification recognizes that failure
/// > to persist can be unavoidable in certain server
/// > environments, it STRONGLY ENCOURAGES message store
/// > implementation techniques that avoid this problem.  For
/// > example:
/// >
/// >   1. Unique identifiers are strictly ascending in the
/// >      mailbox at all times.  If the physical message store is
/// >      re-ordered by a non-IMAP agent, this requires that the
/// >      unique identifiers in the mailbox be regenerated, since
/// >      the former unique identifiers are no longer strictly
/// >      ascending as a result of the re-ordering.
/// >   2. If the message store has no mechanism to store unique
/// >      identifiers, it must regenerate unique identifiers at
/// >      each session, and each session must have a unique
/// >      `UIDVALIDITY` value.
/// >   3. If the mailbox is deleted and a new mailbox with the
/// >      same name is created at a later date, the server must
/// >      either keep track of unique identifiers from the
/// >      previous instance of the mailbox, or it must assign a
/// >      new `UIDVALIDITY` value to the new instance of the
/// >      mailbox.  A good `UIDVALIDITY` value to use in this case
/// >      is a 32-bit representation of the creation date/time of
/// >      the mailbox.  It is alright to use a constant such as
/// >      1, but only if it guaranteed that unique identifiers
/// >      will never be reused, even in the case of a mailbox
/// >      being deleted (or renamed) and a new mailbox by the
/// >      same name created at some future time.
/// >   4. The combination of mailbox name, `UIDVALIDITY`, and `UID`
/// >      must refer to a single immutable message on that server
/// >      forever.  In particular, the internal date, [RFC 2822](https://tools.ietf.org/html/rfc2822)
/// >      size, envelope, body structure, and message texts
/// >      (RFC822, RFC822.HEADER, RFC822.TEXT, and all BODY[...]
/// >      fetch data items) must never change.  This does not
/// >      include message numbers, nor does it include attributes
/// >      that can be set by a `STORE` command (e.g., `FLAGS`).
pub type Uid = u32;

/// From section [2.3.1.2 of RFC 3501](https://tools.ietf.org/html/rfc3501#section-2.3.1.2).
///
/// A relative position from 1 to the number of messages in the mailbox.
/// This position is ordered by ascending unique identifier.  As
/// each new message is added, it is assigned a message sequence number
/// that is 1 higher than the number of messages in the mailbox before
/// that new message was added.
///
/// Message sequence numbers can be reassigned during the session.  For
/// example, when a message is permanently removed (expunged) from the
/// mailbox, the message sequence number for all subsequent messages is
/// decremented.  The number of messages in the mailbox is also
/// decremented.  Similarly, a new message can be assigned a message
/// sequence number that was once held by some other message prior to an
/// expunge.
///
/// In addition to accessing messages by relative position in the
/// mailbox, message sequence numbers can be used in mathematical
/// calculations.  For example, if an untagged "11 EXISTS" is received,
/// and previously an untagged "8 EXISTS" was received, three new
/// messages have arrived with message sequence numbers of 9, 10, and 11.
/// Another example, if message 287 in a 523 message mailbox has UID
/// 12345, there are exactly 286 messages which have lesser UIDs and 236
/// messages which have greater UIDs.
pub type Seq = u32;

/// With the exception of [`Flag::Custom`], these flags are system flags that are pre-defined in
/// [RFC 3501 section 2.3.2](https://tools.ietf.org/html/rfc3501#section-2.3.2). All system flags
/// begin with `\` in the IMAP protocol.  Certain system flags (`\Deleted` and `\Seen`) have
/// special semantics described elsewhere.
///
/// A flag can be permanent or session-only on a per-flag basis. Permanent flags are those which
/// the client can add or remove from the message flags permanently; that is, concurrent and
/// subsequent sessions will see any change in permanent flags.  Changes to session flags are valid
/// only in that session.
///
/// > Note: The `\Recent` system flag is a special case of a session flag.  `\Recent` can not be
/// > used as an argument in a `STORE` or `APPEND` command, and thus can not be changed at all.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Flag<'a> {
    /// Message has been read
    Seen,

    /// Message has been answered
    Answered,

    /// Message is "flagged" for urgent/special attention
    Flagged,

    /// Message is "deleted" for removal by later EXPUNGE
    Deleted,

    /// Message has not completed composition (marked as a draft).
    Draft,

    /// Message is "recently" arrived in this mailbox.  This session is the first session to have
    /// been notified about this message; if the session is read-write, subsequent sessions will
    /// not see `\Recent` set for this message.  This flag can not be altered by the client.
    ///
    /// If it is not possible to determine whether or not this session is the first session to be
    /// notified about a message, then that message will generally be considered recent.
    ///
    /// If multiple connections have the same mailbox selected simultaneously, it is undefined
    /// which of these connections will see newly-arrived messages with `\Recent` set and which
    /// will see it without `\Recent` set.
    Recent,

    /// The [`Mailbox::permanent_flags`] can include this special flag (`\*`), which indicates that
    /// it is possible to create new keywords by attempting to store those flags in the mailbox.
    MayCreate,

    /// A non-standard user- or server-defined flag.
    Custom(Cow<'a, str>),
}

impl Flag<'static> {
    fn system(s: &str) -> Option<Self> {
        match s {
            "\\Seen" => Some(Flag::Seen),
            "\\Answered" => Some(Flag::Answered),
            "\\Flagged" => Some(Flag::Flagged),
            "\\Deleted" => Some(Flag::Deleted),
            "\\Draft" => Some(Flag::Draft),
            "\\Recent" => Some(Flag::Recent),
            "\\*" => Some(Flag::MayCreate),
            _ => None,
        }
    }
}

impl<'a> From<String> for Flag<'a> {
    fn from(s: String) -> Self {
        if let Some(f) = Flag::system(&s) {
            f
        } else {
            Flag::Custom(Cow::Owned(s))
        }
    }
}

impl<'a> From<&'a str> for Flag<'a> {
    fn from(s: &'a str) -> Self {
        if let Some(f) = Flag::system(s) {
            f
        } else {
            Flag::Custom(Cow::Borrowed(s))
        }
    }
}

mod mailbox;
pub use self::mailbox::Mailbox;

mod fetch;
pub use self::fetch::Fetch;

mod name;
pub use self::name::{Name, NameAttribute};

mod capabilities;
pub use self::capabilities::Capabilities;

/// re-exported from imap_proto;
pub use imap_proto::StatusAttribute;

// We need a ResponseCode that is not tied to a lifetime, to be used in UnsolicitedResponse.
/// Response code that may be sent with OK/NO/BAD/BYE responses.
/// See [RFC 3501](https://tools.ietf.org/html/rfc3501#section-3.1).
#[derive(Debug, Eq, PartialEq)]
pub enum ResponseCode {
    //Alert: not parsed by imap-proto yet.
    //BadCharset: not parsed by imap-proto yet.
    //Capability: not parsed by imap-proto yet.
    //Parse: not parsed by imap-proto yet.
    /// See [RFC 4551](https://tools.ietf.org/html/rfc4551#section-3.1.1).
    HighestModSeq(u64),
    /// Flags that can be changed permanently.
    PermanentFlags(Vec<String>),
    /// The mailbox status has changed to read-only.
    ReadOnly,
    /// The mailbox status has changed to read-write.
    ReadWrite,
    /// Indicates that the mailbox must be created first.
    TryCreate,
    /// Next unique identifier value.
    UidNext(u32),
    /// The unique identifier validity value.
    UidValidity(u32),
    /// First message without the \Seen flag set.
    Unseen(u32),
}

impl<'a> From<imap_proto::types::ResponseCode<'a>> for ResponseCode {
    fn from(r: imap_proto::types::ResponseCode<'a>) -> Self {
        match r {
            imap_proto::types::ResponseCode::HighestModSeq(n) => ResponseCode::HighestModSeq(n),
            imap_proto::types::ResponseCode::PermanentFlags(v) => {
                ResponseCode::PermanentFlags(v.iter().map(|x| (*x).into()).collect())
            }
            imap_proto::types::ResponseCode::ReadOnly => ResponseCode::ReadOnly,
            imap_proto::types::ResponseCode::ReadWrite => ResponseCode::ReadWrite,
            imap_proto::types::ResponseCode::TryCreate => ResponseCode::TryCreate,
            imap_proto::types::ResponseCode::UidNext(n) => ResponseCode::UidNext(n),
            imap_proto::types::ResponseCode::UidValidity(n) => ResponseCode::UidValidity(n),
            imap_proto::types::ResponseCode::Unseen(n) => ResponseCode::Unseen(n),
        }
    }
}

/// An attribute of the message refered to by a FETCH unsolicited response.
#[derive(Debug, Eq, PartialEq)]
pub enum UnsolicitedFetchAttribute {
    /// The set of flags of this message.
    Flags(Vec<String>),
    /// Some other attribute not handled yet.
    // I don't know which attributes besides FLAGS make sense to be sent unsolicited.
    Other,
}

/// Responses that the server sends that are not related to the current command.
/// [RFC 3501](https://tools.ietf.org/html/rfc3501#section-7) states that clients need to be able
/// to accept any response at any time. These are the ones we've encountered in the wild.
///
/// Note that `Recent`, `Exists` and `Expunge` responses refer to the currently `SELECT`ed folder,
/// so the user must take care when interpreting these.
#[derive(Debug, PartialEq, Eq)]
pub enum UnsolicitedResponse {
    /// An unsolicited [`STATUS response`](https://tools.ietf.org/html/rfc3501#section-7.2.4).
    ///
    /// It can only happen during a [`Session::status`] command.
    Status {
        /// The mailbox that this status response is for.
        mailbox: String,
        /// The attributes of this mailbox.
        attributes: Vec<StatusAttribute>,
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
    Expunge(u32),

    /// An unsolicited [`OK` response](https://tools.ietf.org/html/rfc3501#section-7.1.1).
    Ok {
        /// Optional response code.
        code: Option<ResponseCode>,
        /// Information text that may be presented to the user.
        information: Option<String>,
    },

    /// An unsolicited [`NO` response](https://tools.ietf.org/html/rfc3501#section-7.1.2).
    No {
        /// Optional response code.
        code: Option<ResponseCode>,
        /// Information text that may be presented to the user.
        information: Option<String>,
    },

    /// An unsolicited [`BAD` response](https://tools.ietf.org/html/rfc3501#section-7.1.3).
    Bad {
        /// Optional response code.
        code: Option<ResponseCode>,
        /// Information text that may be presented to the user.
        information: Option<String>,
    },

    /// An unsolicited [`BYE` response](https://tools.ietf.org/html/rfc3501#section-7.1.5).
    Bye {
        /// Optional response code.
        code: Option<ResponseCode>,
        /// Information text that may be presented to the user.
        information: Option<String>,
    },

    /// An unsolicited [`FETCH` response](https://tools.ietf.org/html/rfc3501#section-7.4.2).
    Fetch {
        /// Message identifier.
        id: u32,
        /// Attribute values for this message.
        attributes: Vec<UnsolicitedFetchAttribute>,
    },
}

enum_set_type! {
    /// Unsolicited responses categories, to be used by the
    /// [`Session::request_unsolicited_responses`] method.
    pub enum UnsolicitedResponseCategory {
        /// Asks for `RECENT` responses.
        Recent,
        /// Asks for `EXISTS` responses.
        Exists,
        /// Asks for `EXPUNGE` responses.
        Expunge,
        /// Asks for `OK` responses.
        Ok,
        /// Asks for `NO` responses.
        No,
        /// Asks for `BAD` responses.
        Bad,
        /// Asks for the `BYE` response.
        Bye,
        /// Asks for `STATUS` responses.
        Status,
        /// Asks for `FETCH` responses.
        Fetch,
    }
}

impl UnsolicitedResponse {
    /// Category corresponding to a response
    pub(crate) fn category(&self) -> UnsolicitedResponseCategory {
        match self {
            UnsolicitedResponse::Status { .. } => UnsolicitedResponseCategory::Status,
            UnsolicitedResponse::Recent(_) => UnsolicitedResponseCategory::Recent,
            UnsolicitedResponse::Exists(_) => UnsolicitedResponseCategory::Exists,
            UnsolicitedResponse::Expunge(_) => UnsolicitedResponseCategory::Expunge,
            UnsolicitedResponse::Ok { .. } => UnsolicitedResponseCategory::Ok,
            UnsolicitedResponse::No { .. } => UnsolicitedResponseCategory::No,
            UnsolicitedResponse::Bad { .. } => UnsolicitedResponseCategory::Bad,
            UnsolicitedResponse::Bye { .. } => UnsolicitedResponseCategory::Bye,
            UnsolicitedResponse::Fetch { .. } => UnsolicitedResponseCategory::Fetch,
        }
    }
}

/// This type wraps an input stream and a type that was constructed by parsing that input stream,
/// which allows the parsed type to refer to data in the underlying stream instead of copying it.
///
/// Any references given out by a `ZeroCopy` should never be used after the `ZeroCopy` is dropped.
pub struct ZeroCopy<D> {
    _owned: Box<[u8]>,
    derived: D,
}

impl<D> ZeroCopy<D> {
    /// Derive a new `ZeroCopy` view of the byte data stored in `owned`.
    ///
    /// # Safety
    ///
    /// The `derive` callback will be passed a `&'static [u8]`. However, this reference is not, in
    /// fact `'static`. Instead, it is only valid for as long as the `ZeroCopy` lives. Therefore,
    /// it is *only* safe to call this function if *every* accessor on `D` returns either a type
    /// that does not contain any borrows, *or* where the return type is bound to the lifetime of
    /// `&self`.
    ///
    /// It is *not* safe for the error type `E` to borrow from the passed reference.
    pub(crate) unsafe fn make<F, E>(owned: Vec<u8>, derive: F) -> Result<Self, E>
    where
        F: FnOnce(&'static [u8]) -> Result<D, E>,
    {
        use std::mem;

        // the memory pointed to by `owned` now has a stable address (on the heap).
        // even if we move the `Box` (i.e., into `ZeroCopy`), a slice to it will remain valid.
        let _owned = owned.into_boxed_slice();

        // this is the unsafe part -- the implementor of `derive` must be aware that the reference
        // they are passed is not *really* 'static, but rather the lifetime of `&self`.
        let static_owned_ref: &'static [u8] = mem::transmute(&*_owned);

        Ok(ZeroCopy {
            _owned,
            derived: derive(static_owned_ref)?,
        })
    }

    /// Take out the derived value of this `ZeroCopy`.
    ///
    /// Only safe if `D` contains no references into the underlying input stream (i.e., the `owned`
    /// passed to `ZeroCopy::new`).
    pub(crate) unsafe fn take(self) -> D {
        self.derived
    }
}

use super::error::Error;
pub(crate) type ZeroCopyResult<T> = Result<ZeroCopy<T>, Error>;

use std::ops::Deref;
impl<D> Deref for ZeroCopy<D> {
    type Target = D;
    fn deref(&self) -> &Self::Target {
        &self.derived
    }
}

// re-implement standard traits
// basically copied from Rc

impl<D: PartialEq> PartialEq for ZeroCopy<D> {
    fn eq(&self, other: &ZeroCopy<D>) -> bool {
        **self == **other
    }
}
impl<D: Eq> Eq for ZeroCopy<D> {}

use std::cmp::Ordering;
impl<D: PartialOrd> PartialOrd for ZeroCopy<D> {
    fn partial_cmp(&self, other: &ZeroCopy<D>) -> Option<Ordering> {
        (**self).partial_cmp(&**other)
    }
    fn lt(&self, other: &ZeroCopy<D>) -> bool {
        **self < **other
    }
    fn le(&self, other: &ZeroCopy<D>) -> bool {
        **self <= **other
    }
    fn gt(&self, other: &ZeroCopy<D>) -> bool {
        **self > **other
    }
    fn ge(&self, other: &ZeroCopy<D>) -> bool {
        **self >= **other
    }
}
impl<D: Ord> Ord for ZeroCopy<D> {
    fn cmp(&self, other: &ZeroCopy<D>) -> Ordering {
        (**self).cmp(&**other)
    }
}

use std::hash::{Hash, Hasher};
impl<D: Hash> Hash for ZeroCopy<D> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

use std::fmt;
impl<D: fmt::Display> fmt::Display for ZeroCopy<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}
impl<D: fmt::Debug> fmt::Debug for ZeroCopy<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, D> IntoIterator for &'a ZeroCopy<D>
where
    &'a D: IntoIterator,
{
    type Item = <&'a D as IntoIterator>::Item;
    type IntoIter = <&'a D as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        (**self).into_iter()
    }
}
