use imap_proto::types::Capability;
use std::collections::hash_set::Iter;
use std::collections::HashSet;

const IMAP4REV1_CAPABILITY: &str = "IMAP4rev1";
const AUTH_CAPABILITY_PREFIX: &str = "AUTH=";

/// From [section 7.2.1 of RFC 3501](https://tools.ietf.org/html/rfc3501#section-7.2.1).
///
/// A list of capabilities that the server supports.
/// The capability list will include the atom "IMAP4rev1".
///
/// In addition, all servers implement the `STARTTLS`, `LOGINDISABLED`, and `AUTH=PLAIN` (described
/// in [IMAP-TLS](https://tools.ietf.org/html/rfc2595)) capabilities. See the [Security
/// Considerations section of the RFC](https://tools.ietf.org/html/rfc3501#section-11) for
/// important information.
///
/// A capability name which begins with `AUTH=` indicates that the server supports that particular
/// authentication mechanism.
///
/// The `LOGINDISABLED` capability indicates that the `LOGIN` command is disabled, and that the
/// server will respond with a [`super::Error::No`] response to any attempt to use the `LOGIN`
/// command even if the user name and password are valid.  An IMAP client MUST NOT issue the
/// `LOGIN` command if the server advertises the `LOGINDISABLED` capability.
///
/// Other capability names indicate that the server supports an extension, revision, or amendment
/// to the IMAP4rev1 protocol. Capability names either begin with `X` or they are standard or
/// standards-track [RFC 3501](https://tools.ietf.org/html/rfc3501) extensions, revisions, or
/// amendments registered with IANA.
///
/// Client implementations SHOULD NOT require any capability name other than `IMAP4rev1`, and MUST
/// ignore any unknown capability names.
pub struct Capabilities(
    // Note that this field isn't *actually* 'static.
    // Rather, it is tied to the lifetime of the `ZeroCopy` that contains this `Name`.
    pub(crate) HashSet<Capability<'static>>,
);

impl Capabilities {
    /// Check if the server has the given capability.
    pub fn has<'a>(&self, s: &Capability<'a>) -> bool {
        self.0.contains(s)
    }

    /// Check if the server has the given capability via str.
    pub fn has_str(&self, s: &str) -> bool {
        if s == IMAP4REV1_CAPABILITY {
            self.has(&Capability::Imap4rev1)
        } else if s.starts_with(AUTH_CAPABILITY_PREFIX) {
            self.has(&Capability::Auth(&s[AUTH_CAPABILITY_PREFIX.len()..]))
        } else {
            self.has(&Capability::Atom(s))
        }
    }

    /// Iterate over all the server's capabilities
    pub fn iter(&self) -> Iter<Capability> {
        self.0.iter()
    }

    /// Returns how many capabilities the server has.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the server purports to have no capabilities.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
