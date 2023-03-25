use crate::error::Error;
use crate::parse::{parse_many_into, MapOrNot};
use crate::types::UnsolicitedResponse;
use imap_proto::{Capability, Response};
use ouroboros::self_referencing;
use std::collections::hash_set::Iter;
use std::collections::HashSet;
use std::fmt;
use std::sync::mpsc;

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
#[self_referencing]
pub struct Capabilities {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) capabilities: HashSet<Capability<'this>>,
}

impl Clone for Capabilities {
    fn clone(&self) -> Self {
        // Give _rx a name so it's not immediately dropped. Otherwise any unsolicited responses
        // that would be send there will return a SendError instead of the parsed response simply
        // being dropped later.
        let (mut tx, _rx) = mpsc::channel();
        Self::parse(self.borrow_data().clone(), &mut tx)
            .expect("failed to parse capabilities from data which was already successfully parse before")
    }
}

impl Capabilities {
    /// Parse the given input into one or more [`Capabilitity`] responses.
    pub(crate) fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        CapabilitiesTryBuilder {
            data: owned,
            capabilities_builder: |input| {
                let mut caps = HashSet::new();
                parse_many_into(input, &mut caps, unsolicited, |response| match response {
                    Response::Capabilities(c) => Ok(MapOrNot::MapVec(c)),
                    resp => Ok(MapOrNot::Not(resp)),
                })?;
                Ok(caps)
            },
        }
        .try_build()
    }

    /// Check if the server has the given capability.
    pub fn has<'a>(&self, cap: &Capability<'a>) -> bool {
        self.borrow_capabilities().contains(cap)
    }

    /// Check if the server has the given capability via str.
    pub fn has_str<S: AsRef<str>>(&self, cap: S) -> bool {
        let s = cap.as_ref();
        if s.eq_ignore_ascii_case(IMAP4REV1_CAPABILITY) {
            return self.has(&Capability::Imap4rev1);
        }
        if s.len() > AUTH_CAPABILITY_PREFIX.len() {
            let (pre, val) = s.split_at(AUTH_CAPABILITY_PREFIX.len());
            if pre.eq_ignore_ascii_case(AUTH_CAPABILITY_PREFIX) {
                return self.has(&Capability::Auth(val.into()));
            }
        }
        self.has(&Capability::Atom(s.into()))
    }

    /// Iterate over all the server's capabilities
    pub fn iter(&self) -> Iter<'_, Capability<'_>> {
        self.borrow_capabilities().iter()
    }

    /// Returns how many capabilities the server has.
    pub fn len(&self) -> usize {
        self.borrow_capabilities().len()
    }

    /// Returns true if the server purports to have no capabilities.
    pub fn is_empty(&self) -> bool {
        self.borrow_capabilities().is_empty()
    }
}

impl fmt::Debug for Capabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_tuple("Capabilities");
        for x in self.borrow_capabilities() {
            dbg.field(x);
        }
        dbg.finish()
    }
}