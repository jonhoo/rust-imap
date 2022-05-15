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

impl TryFrom<&str> for AclRightList {
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
pub enum AclRightError {
    /// Returned when a non-lower-case alpha numeric is provided in the rights list string.
    InvalidRight,
}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// The ACL response from the getacl IMAP command
#[self_referencing]
pub struct Acl {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) inner: (Cow<'this, str>, Vec<AclEntry<'this>>),
}

impl Acl {
    /// Parse the given input into a [`ACL`] response.
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        AclTryBuilder {
            data: owned,
            inner_builder: |input| {
                let mut lines: &[u8] = input;

                // There should only be ONE single ACL response
                while !lines.is_empty() {
                    match imap_proto::parser::parse_response(lines) {
                        Ok((_rest, Response::Acl(a))) => {
                            // lines = rest;
                            return Ok((
                                a.mailbox,
                                a.acls
                                    .into_iter()
                                    .map(|e| AclEntry {
                                        identifier: e.identifier,
                                        rights: e.rights.into(),
                                    })
                                    .collect(),
                            ));
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
        &*self.borrow_inner().0
    }

    /// Returns a list of identifier/rights pairs for the mailbox
    pub fn acls(&self) -> &[AclEntry<'_>] {
        &*self.borrow_inner().1
    }
}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// The list of identifiers and rights for the ACL response
#[derive(Debug, Eq, PartialEq)]
pub struct AclEntry<'a> {
    /// The user identifier the rights are for
    pub identifier: Cow<'a, str>,
    /// the rights for the provided identifier
    pub rights: AclRightList,
}

/// From [section 3.7 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.7).
///
/// The LISTRIGHTS response from the listrights IMAP command
#[self_referencing]
pub struct ListRights {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) inner: (
        Cow<'this, str>, // mailbox
        Cow<'this, str>, // identifier
        AclRightList,    // required
        AclRightList,    // optional
    ),
}

impl ListRights {
    /// Parse the given input into a [`LISTRIGHTS`] response.
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        ListRightsTryBuilder {
            data: owned,
            inner_builder: |input| {
                let mut lines: &[u8] = input;

                // There should only be ONE single LISTRIGHTS response
                while !lines.is_empty() {
                    match imap_proto::parser::parse_response(lines) {
                        Ok((_rest, Response::ListRights(a))) => {
                            // lines = rest;
                            return Ok((
                                a.mailbox,
                                a.identifier,
                                a.required.into(),
                                a.optional.into(),
                            ));
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
        &*self.borrow_inner().0
    }

    /// Returns the user identifier for the rights
    pub fn identifier(&self) -> &str {
        &*self.borrow_inner().1
    }

    /// Returns the set of rights that are always provided for this identifier
    pub fn required(&self) -> &AclRightList {
        &self.borrow_inner().2
    }

    /// Returns the set of rights that can be granted to the identifier
    pub fn optional(&self) -> &AclRightList {
        &self.borrow_inner().3
    }
}

/// From [section 3.8 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.8).
///
/// The MYRIGHTS response from the myrights IMAP command
#[self_referencing]
pub struct MyRights {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) inner: (Cow<'this, str>, AclRightList),
}

impl MyRights {
    /// Parse the given input into a [`MRIGHTS`] response.
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        MyRightsTryBuilder {
            data: owned,
            inner_builder: |input| {
                let mut lines: &[u8] = input;

                // There should only be ONE single MYRIGHTS response
                while !lines.is_empty() {
                    match imap_proto::parser::parse_response(lines) {
                        Ok((_rest, Response::MyRights(a))) => {
                            // lines = rest;
                            return Ok((a.mailbox, a.rights.into()));
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
        &*self.borrow_inner().0
    }

    /// Returns the rights for the mailbox
    pub fn rights(&self) -> &AclRightList {
        &self.borrow_inner().1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_right_list_to_string() {
        let rights: AclRightList = vec![
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
    fn test_str_to_acl_right_list() {
        let right_string = "lrskx0";

        let rights: Result<AclRightList, _> = right_string.try_into();

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

        let rights: Result<AclRightList, _> = right_string.try_into();

        assert_eq!(rights, Err(AclRightError::InvalidRight));
    }

    #[test]
    fn test_acl_right_list_has_right() {
        let rights: AclRightList = "lrskx".try_into().unwrap();

        assert!(rights.has_right('l'));
        assert!(rights.has_right(AclRight::Lookup));
        assert!(!rights.has_right('0'));
        assert!(!rights.has_right(AclRight::Custom('0')));
    }
}
