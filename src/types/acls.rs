use crate::error::{Error, ParseError};
use crate::parse::try_handle_unilateral;
use crate::types::UnsolicitedResponse;
use imap_proto::types::AclRight;
use imap_proto::Response;
use ouroboros::self_referencing;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::sync::mpsc;

/// enum used for [Session::set_acl] to specify how the ACL is to be modified.
pub enum AclModifyMode {
    /// Replace all ACLs on the identifier for the mailbox
    Replace,
    /// Add the given ACLs to the identifier for the mailbox
    Add,
    /// Remove the given ACLs from the identifier for the mailbox
    Remove,
}

/// Helpful wrapper around the ACL rights vector
/// Used as input for [Session::set_acl] as output in [ListRights], [MyRights], and [AclEntry]
#[derive(Debug, Eq, PartialEq)]
pub struct AclRights {
    pub(crate) data: HashSet<AclRight>,
}

impl AclRights {
    /// Returns true if the AclRights has the provided ACL (either as a char or an AclRight enum)
    pub fn has_right<T: Into<AclRight>>(&self, right: T) -> bool {
        self.data.contains(&right.into())
    }
}

impl Display for AclRights {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut v: Vec<char> = self.data.iter().map(|c| char::from(*c)).collect();

        v.sort_unstable();

        write!(f, "{}", v.into_iter().collect::<String>())
    }
}

impl From<HashSet<AclRight>> for AclRights {
    fn from(hash: HashSet<AclRight>) -> Self {
        Self { data: hash }
    }
}

impl From<Vec<AclRight>> for AclRights {
    fn from(vec: Vec<AclRight>) -> Self {
        AclRights {
            data: vec.into_iter().collect(),
        }
    }
}

impl TryFrom<&str> for AclRights {
    type Error = AclRightError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        if !input
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(AclRightError::InvalidRight);
        }

        Ok(input
            .chars()
            .map(|c| c.into())
            .collect::<HashSet<AclRight>>()
            .into())
    }
}

/// Error from parsing AclRight strings
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AclRightError {
    /// Returned when a non-lower-case alpha numeric is provided in the rights list string.
    InvalidRight,
}

impl Display for AclRightError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            AclRightError::InvalidRight => {
                write!(f, "Rights may only be lowercase alpha numeric characters")
            }
        }
    }
}

impl std::error::Error for AclRightError {}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// The ACL response from the [Session::get_acl] IMAP command
#[self_referencing]
pub struct Acl {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) acl: InnerAcl<'this>,
}

impl Acl {
    /// Parse the given input into a [`ACL`] response.
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        AclTryBuilder {
            data: owned,
            acl_builder: |input| {
                let mut lines: &[u8] = input;

                // There should only be ONE single ACL response
                while !lines.is_empty() {
                    match imap_proto::parser::parse_response(lines) {
                        Ok((_rest, Response::Acl(a))) => {
                            // lines = rest;
                            return Ok(InnerAcl {
                                mailbox: a.mailbox,
                                acls: a
                                    .acls
                                    .into_iter()
                                    .map(|e| AclEntry {
                                        identifier: e.identifier,
                                        rights: e.rights.into(),
                                    })
                                    .collect(),
                            });
                        }
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

                Err(Error::Parse(ParseError::Invalid(lines.to_vec())))
            },
        }
        .try_build()
    }

    /// Return the mailbox the ACL entries belong to
    pub fn mailbox(&self) -> &str {
        &*self.borrow_acl().mailbox
    }

    /// Returns a list of identifier/rights pairs for the mailbox
    pub fn acls(&self) -> &[AclEntry<'_>] {
        &*self.borrow_acl().acls
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct InnerAcl<'a> {
    /// The mailbox the ACL Entries belong to
    pub(crate) mailbox: Cow<'a, str>,
    /// The list of identifier/rights pairs for the mailbox
    pub(crate) acls: Vec<AclEntry<'a>>,
}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// The list of identifiers and rights for the [Acl] response
#[derive(Debug, Eq, PartialEq)]
pub struct AclEntry<'a> {
    /// The user identifier the rights are for
    pub identifier: Cow<'a, str>,
    /// the rights for the provided identifier
    pub rights: AclRights,
}

/// From [section 3.7 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.7).
///
/// The LISTRIGHTS response from the [Session::list_rights] IMAP command
#[self_referencing]
pub struct ListRights {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) rights: InnerListRights<'this>,
}

impl ListRights {
    /// Parse the given input into a [`LISTRIGHTS`] response.
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        ListRightsTryBuilder {
            data: owned,
            rights_builder: |input| {
                let mut lines: &[u8] = input;

                // There should only be ONE single LISTRIGHTS response
                while !lines.is_empty() {
                    match imap_proto::parser::parse_response(lines) {
                        Ok((_rest, Response::ListRights(a))) => {
                            // lines = rest;
                            return Ok(InnerListRights {
                                mailbox: a.mailbox,
                                identifier: a.identifier,
                                required: a.required.into(),
                                optional: a.optional.into(),
                            });
                        }
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

                Err(Error::Parse(ParseError::Invalid(lines.to_vec())))
            },
        }
        .try_build()
    }

    /// Returns the mailbox for the rights
    pub fn mailbox(&self) -> &str {
        &*self.borrow_rights().mailbox
    }

    /// Returns the user identifier for the rights
    pub fn identifier(&self) -> &str {
        &*self.borrow_rights().identifier
    }

    /// Returns the set of rights that are always provided for this identifier
    pub fn required(&self) -> &AclRights {
        &self.borrow_rights().required
    }

    /// Returns the set of rights that can be granted to the identifier
    pub fn optional(&self) -> &AclRights {
        &self.borrow_rights().optional
    }
}

/// From [section 3.7 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.7).
///
/// The LISTRIGHTS response from the listrights IMAP command
#[derive(Debug, Eq, PartialEq)]
pub struct InnerListRights<'a> {
    /// The mailbox for the rights
    pub(crate) mailbox: Cow<'a, str>,
    /// The user identifier for the rights
    pub(crate) identifier: Cow<'a, str>,
    /// The set of rights that are always provided for this identifier
    pub(crate) required: AclRights,
    /// The set of rights that can be granted to the identifier
    pub(crate) optional: AclRights,
}

/// From [section 3.8 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.8).
///
/// The MYRIGHTS response from the [Session::my_rights] IMAP command
#[self_referencing]
pub struct MyRights {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) rights: InnerMyRights<'this>,
}

impl MyRights {
    /// Parse the given input into a [`MRIGHTS`] response.
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        MyRightsTryBuilder {
            data: owned,
            rights_builder: |input| {
                let mut lines: &[u8] = input;

                // There should only be ONE single MYRIGHTS response
                while !lines.is_empty() {
                    match imap_proto::parser::parse_response(lines) {
                        Ok((_rest, Response::MyRights(a))) => {
                            // lines = rest;
                            return Ok(InnerMyRights {
                                mailbox: a.mailbox,
                                rights: a.rights.into(),
                            });
                        }
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

                Err(Error::Parse(ParseError::Invalid(lines.to_vec())))
            },
        }
        .try_build()
    }

    /// Returns the mailbox for the rights
    pub fn mailbox(&self) -> &str {
        &*self.borrow_rights().mailbox
    }

    /// Returns the rights for the mailbox
    pub fn rights(&self) -> &AclRights {
        &self.borrow_rights().rights
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct InnerMyRights<'a> {
    /// The mailbox for the rights
    pub(crate) mailbox: Cow<'a, str>,
    /// The rights for the mailbox
    pub(crate) rights: AclRights,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_rights_to_string() {
        let rights: AclRights = vec![
            AclRight::Lookup,
            AclRight::Read,
            AclRight::Seen,
            AclRight::Custom('0'),
        ]
        .into();
        let expected = "0lrs";

        assert_eq!(rights.to_string(), expected);
    }

    #[test]
    fn test_str_to_acl_rights() {
        let right_string = "lrskx0";

        let rights: Result<AclRights, _> = right_string.try_into();

        assert_eq!(
            rights,
            Ok(vec![
                AclRight::Lookup,
                AclRight::Read,
                AclRight::Seen,
                AclRight::CreateMailbox,
                AclRight::DeleteMailbox,
                AclRight::Custom('0'),
            ]
            .into())
        );
    }

    #[test]
    fn test_str_to_acl_rights_invalid_right_character() {
        let right_string = "l_";

        let rights: Result<AclRights, _> = right_string.try_into();

        assert_eq!(rights, Err(AclRightError::InvalidRight));
    }

    #[test]
    fn test_acl_rights_has_right() {
        let rights: AclRights = "lrskx".try_into().unwrap();

        assert!(rights.has_right('l'));
        assert!(rights.has_right(AclRight::Lookup));
        assert!(!rights.has_right('0'));
        assert!(!rights.has_right(AclRight::Custom('0')));
    }
}
