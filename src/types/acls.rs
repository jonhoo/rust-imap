use crate::error::{Error, ParseError};
use crate::types::UnsolicitedResponse;
use crate::parse::try_handle_unilateral;
use imap_proto::Response;
use imap_proto::types::AclRight;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::sync::mpsc;

/// enum used for set_acl to specify how the ACL is to be modified.
pub enum AclModifyMode {
    /// Replace all ACLs on the identifier for the mailbox
    Replace,
    /// Add the given ACLs to the identifier for the mailbox
    Add,
    /// Remove the given ACLs from the identifier for the mailbox
    Remove,
}

/// Helpful wrapper around the ACL rights vector
#[derive(Debug, Eq, PartialEq)]
pub struct AclRightList {
    pub(crate) data: HashSet<AclRight>,
}

impl AclRightList {
    /// Returns if the AclRightList has the provided ACL (either as a char or an AclRight enum)
    pub fn has_right<T: Into<AclRight>>(&self, right: T) -> bool {
        self.data.contains(&right.into())
    }
}

impl Display for AclRightList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut v: Vec<char> = self.data.iter().map(|c| char::from(*c)).collect();

        v.sort_unstable();

        write!(f, "{}", v.into_iter().collect::<String>())
    }
}

impl From<HashSet<AclRight>> for AclRightList {
    fn from(hash: HashSet<AclRight>) -> Self {
        Self { data: hash }
    }
}

impl From<Vec<AclRight>> for AclRightList {
    fn from(vec: Vec<AclRight>) -> Self {
        AclRightList {
            data: vec.into_iter().collect(),
        }
    }
}

impl From<&str> for AclRightList {
    fn from(i: &str) -> Self {
        i.chars()
            .into_iter()
            .map(|c| c.into())
            .collect::<HashSet<AclRight>>()
            .into()
    }
}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// The ACL response from the getacl IMAP command
#[derive(Debug, Eq, PartialEq)]
pub struct Acl {
    /// the mailbox these rights list are for
    pub mailbox: String,
    /// The list of identifier/rights pairs for the mailbox
    pub acls: Vec<AclEntry>,
}

impl Acl {
    /// Parse the given input into a [`ACL`] response.
    pub fn parse(
        lines: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        let mut lines: &[u8] = &lines;
        let mut acl = None;

        while !lines.is_empty() {
            match imap_proto::parser::parse_response(lines) {
                Ok((rest, Response::Acl(a))) => {
                    lines = rest;
                    acl = Some(a.into());
                },
                Ok((rest, data)) => {
                    lines = rest;
                    if let Some(resp) = try_handle_unilateral(data, unsolicited) {
                        return Err(resp.into());
                    }
                }
                _ => {
                    return Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                }
            }
        }

        acl.ok_or_else(|| Error::Parse(ParseError::Invalid(lines.to_vec())))
    }
}

impl Into<Acl> for imap_proto::types::Acl<'_> {
    fn into(self) -> Acl {
        Acl {
            mailbox: self.mailbox.to_string(),
            acls: self.acls.into_iter().map(|e| e.into()).collect(),
        }
    }
}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// The list of identifiers and rights for the ACL response
#[derive(Debug, Eq, PartialEq)]
pub struct AclEntry {
    /// The user identifier the rights are for
    pub identifier: String,
    /// the rights for the provided identifier
    pub rights: AclRightList,
}

impl Into<AclEntry> for imap_proto::types::AclEntry<'_> {
    fn into(self) -> AclEntry {
        AclEntry {
            identifier: self.identifier.to_string(),
            rights: self.rights.into(),
        }
    }
}

/// From [section 3.7 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.7).
///
/// The LISTRIGHTS response from the listrights IMAP command
#[derive(Debug, Eq, PartialEq)]
pub struct ListRights {
    /// The mailbox for the rights
    pub mailbox: String,
    /// The user identifier for the rights
    pub identifier: String,
    /// The set of rights that are always provided for this identifier
    pub required: AclRightList,
    /// The set of rights that can be granted to the identifier
    pub optional: AclRightList,
}

impl ListRights {
    /// Parse the given input into a [`LISTRIGHTS`] response.
    pub fn parse(
        lines: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        let mut lines: &[u8] = &lines;
        let mut acl = None;

        while !lines.is_empty() {
            match imap_proto::parser::parse_response(lines) {
                Ok((rest, Response::ListRights(a))) => {
                    lines = rest;
                    acl = Some(a.into());
                },
                Ok((rest, data)) => {
                    lines = rest;
                    if let Some(resp) = try_handle_unilateral(data, unsolicited) {
                        return Err(resp.into());
                    }
                }
                _ => {
                    return Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                }
            }
        }

        acl.ok_or_else(|| Error::Parse(ParseError::Invalid(lines.to_vec())))
    }
}

impl Into<ListRights> for imap_proto::types::ListRights<'_> {
    fn into(self) -> ListRights {
        ListRights {
            mailbox: self.mailbox.to_string(),
            identifier: self.identifier.to_string(),
            required: self.required.into(),
            optional: self.optional.into(),
        }
    }
}

/// From [section 3.8 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.8).
///
/// The MYRIGHTS response from the myrights IMAP command
#[derive(Debug, Eq, PartialEq)]
pub struct MyRights {
    /// The mailbox for the rights
    pub mailbox: String,
    /// The rights for the mailbox
    pub rights: AclRightList,
}

impl MyRights {
    /// Parse the given input into a [`MRIGHTS`] response.
    pub fn parse(
        lines: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        let mut lines: &[u8] = &lines;
        let mut acl = None;

        while !lines.is_empty() {
            match imap_proto::parser::parse_response(lines) {
                Ok((rest, Response::MyRights(a))) => {
                    lines = rest;
                    acl = Some(a.into());
                },
                Ok((rest, data)) => {
                    lines = rest;
                    if let Some(resp) = try_handle_unilateral(data, unsolicited) {
                        return Err(resp.into());
                    }
                }
                _ => {
                    return Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                }
            }
        }

        acl.ok_or_else(|| Error::Parse(ParseError::Invalid(lines.to_vec())))
    }
}

impl Into<MyRights> for imap_proto::types::MyRights<'_> {
    fn into(self) -> MyRights {
        MyRights {
            mailbox: self.mailbox.to_string(),
            rights: self.rights.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_right_list_to_string() {
        let rights: AclRightList = vec![AclRight::Lookup, AclRight::Read, AclRight::Seen].into();
        let expected = "lrs";

        assert_eq!(rights.to_string(), expected);
    }

    #[test]
    fn test_str_to_acl_right_list() {
        let right_string = "lrskx";

        let rights: AclRightList = right_string.into();

        assert_eq!(
            rights,
            vec![
                AclRight::Lookup,
                AclRight::Read,
                AclRight::Seen,
                AclRight::CreateMailbox,
                AclRight::DeleteMailbox,
            ]
                .into()
        );
    }

    #[test]
    fn test_acl_right_list_has_right() {
        let rights: AclRightList = "lrskx".into();

        assert!(rights.has_right('l'));
        assert!(rights.has_right(AclRight::Lookup));
        assert!(!rights.has_right('0'));
        assert!(!rights.has_right(AclRight::Custom('0')));
    }
}
