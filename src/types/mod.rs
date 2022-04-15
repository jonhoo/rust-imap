//! This module contains types used throughout the IMAP protocol.

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

mod fetch;
pub use self::fetch::{Fetch, Fetches};

mod flag;
pub use self::flag::Flag;

mod mailbox;
pub use self::mailbox::Mailbox;

mod name;
pub use self::name::{Name, Names};

mod capabilities;
pub use self::capabilities::Capabilities;

mod deleted;
pub use self::deleted::Deleted;

mod unsolicited_response;
pub use self::unsolicited_response::{AttributeValue, UnsolicitedResponse};
