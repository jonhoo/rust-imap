use crate::error::Error;
use crate::parse::{parse_until_done, MapOrNot};
use crate::types::UnsolicitedResponse;
#[cfg(doc)]
use crate::Session;
use imap_proto::types::AclRight;
use imap_proto::Response;
use ouroboros::self_referencing;
use std::borrow::Cow;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};

/// Specifies how [`Session::set_acl`] should modify an existing permission set.
#[derive(Debug, Clone, Copy)]
pub enum AclModifyMode {
    /// Replace all ACLs on the identifier for the mailbox
    Replace,
    /// Add the given ACLs to the identifier for the mailbox
    Add,
    /// Remove the given ACLs from the identifier for the mailbox
    Remove,
}

/// A set of [`imap_proto::AclRight`]s.
///
/// Used as input for [`Session::set_acl`] as output in [`ListRights`], [`MyRights`], and [`AclEntry`]
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct AclRights {
    pub(crate) data: HashSet<AclRight>,
}

impl AclRights {
    /// Returns true if the AclRights has the provided ACL (either as a char or an AclRight enum)
    pub fn contains<T: Into<AclRight>>(&self, right: T) -> bool {
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
/// This is a wrapper around a single [`Acl`].
///
/// The ACL response from the [`Session::get_acl`] IMAP command
#[self_referencing]
pub struct AclResponse {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) acl: Acl<'this>,
}

impl AclResponse {
    /// Parse the given input into a [`Acl`] response.
    pub(crate) fn parse(
        owned: Vec<u8>,
        unsolicited: &mut VecDeque<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        AclResponseTryBuilder {
            data: owned,
            acl_builder: |input| {
                // There should only be ONE single ACL response
                parse_until_done(input, unsolicited, |response| match response {
                    Response::Acl(a) => Ok(MapOrNot::Map(Acl {
                        mailbox: a.mailbox,
                        acls: a
                            .acls
                            .into_iter()
                            .map(|e| AclEntry {
                                identifier: e.identifier,
                                rights: e.rights.into(),
                            })
                            .collect(),
                    })),
                    resp => Ok(MapOrNot::Not(resp)),
                })
            },
        }
        .try_build()
    }

    /// Access to the wrapped [`ListRights`] struct
    pub fn parsed(&self) -> &Acl<'_> {
        self.borrow_acl()
    }
}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// Used by [`AclResponse`].
#[derive(Debug, Eq, PartialEq)]
pub struct Acl<'a> {
    /// The mailbox the ACL Entries belong to
    pub(crate) mailbox: Cow<'a, str>,
    /// The list of identifier/rights pairs for the mailbox
    pub(crate) acls: Vec<AclEntry<'a>>,
}

impl<'a> Acl<'a> {
    /// Return the mailbox the ACL entries belong to
    pub fn mailbox(&self) -> &str {
        &self.mailbox
    }

    /// Returns a list of identifier/rights pairs for the mailbox
    pub fn acls(&self) -> &[AclEntry<'_>] {
        &self.acls
    }
}

/// From [section 3.6 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.6).
///
/// The list of identifiers and rights for the [`Acl`] response
#[derive(Debug, Eq, PartialEq, Clone)]
#[non_exhaustive]
pub struct AclEntry<'a> {
    /// The user identifier the rights are for
    pub identifier: Cow<'a, str>,
    /// the rights for the provided identifier
    pub rights: AclRights,
}

/// From [section 3.7 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.7).
///
/// This is a wrapper around a single [`ListRights`].
///
/// The LISTRIGHTS response from the [`Session::list_rights`] IMAP command
#[self_referencing]
pub struct ListRightsResponse {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) rights: ListRights<'this>,
}

impl ListRightsResponse {
    /// Parse the given input into a [`ListRights`] response.
    pub(crate) fn parse(
        owned: Vec<u8>,
        unsolicited: &mut VecDeque<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        ListRightsResponseTryBuilder {
            data: owned,
            rights_builder: |input| {
                // There should only be ONE single LISTRIGHTS response
                parse_until_done(input, unsolicited, |response| match response {
                    Response::ListRights(a) => Ok(MapOrNot::Map(ListRights {
                        mailbox: a.mailbox,
                        identifier: a.identifier,
                        required: a.required.into(),
                        optional: a.optional.into(),
                    })),
                    resp => Ok(MapOrNot::Not(resp)),
                })
            },
        }
        .try_build()
    }

    /// Access to the wrapped [`ListRights`] struct
    pub fn parsed(&self) -> &ListRights<'_> {
        self.borrow_rights()
    }
}

/// From [section 3.7 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.7).
///
/// Used by [`ListRightsResponse`].
#[derive(Debug, Eq, PartialEq)]
pub struct ListRights<'a> {
    /// The mailbox for the rights
    pub(crate) mailbox: Cow<'a, str>,
    /// The user identifier for the rights
    pub(crate) identifier: Cow<'a, str>,
    /// The set of rights that are always provided for this identifier
    pub(crate) required: AclRights,
    /// The set of rights that can be granted to the identifier
    pub(crate) optional: AclRights,
}

impl ListRights<'_> {
    /// Returns the mailbox for the rights
    pub fn mailbox(&self) -> &str {
        &self.mailbox
    }

    /// Returns the user identifier for the rights
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the set of rights that are always provided for this identifier
    pub fn required(&self) -> &AclRights {
        &self.required
    }

    /// Returns the set of rights that can be granted to the identifier
    pub fn optional(&self) -> &AclRights {
        &self.optional
    }
}

/// From [section 3.8 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.8).
///
/// This is a wrapper around a single [`MyRights`].
///
/// The MYRIGHTS response from the [`Session::my_rights`] IMAP command
#[self_referencing]
pub struct MyRightsResponse {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) rights: MyRights<'this>,
}

impl MyRightsResponse {
    /// Parse the given input into a [`MyRights`] response.
    pub(crate) fn parse(
        owned: Vec<u8>,
        unsolicited: &mut VecDeque<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        MyRightsResponseTryBuilder {
            data: owned,
            rights_builder: |input| {
                // There should only be ONE single MYRIGHTS response
                parse_until_done(input, unsolicited, |response| match response {
                    Response::MyRights(a) => Ok(MapOrNot::Map(MyRights {
                        mailbox: a.mailbox,
                        rights: a.rights.into(),
                    })),
                    resp => Ok(MapOrNot::Not(resp)),
                })
            },
        }
        .try_build()
    }

    /// Access to the wrapped [`MyRights`] struct
    pub fn parsed(&self) -> &MyRights<'_> {
        self.borrow_rights()
    }
}

/// From [section 3.8 of RFC 4313](https://datatracker.ietf.org/doc/html/rfc4314#section-3.8).
///
/// Used by [`MyRightsResponse`].
#[derive(Debug, Eq, PartialEq)]
pub struct MyRights<'a> {
    /// The mailbox for the rights
    pub(crate) mailbox: Cow<'a, str>,
    /// The rights for the mailbox
    pub(crate) rights: AclRights,
}

impl MyRights<'_> {
    /// Returns the mailbox for the rights
    pub fn mailbox(&self) -> &str {
        &self.mailbox
    }

    /// Returns the rights for the mailbox
    pub fn rights(&self) -> &AclRights {
        &self.rights
    }
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

        assert_eq!(
            format!("{}", rights.unwrap_err()),
            "Rights may only be lowercase alpha numeric characters"
        );
    }

    #[test]
    fn test_acl_rights_contains() {
        let rights: AclRights = "lrskx".try_into().unwrap();

        assert!(rights.contains('l'));
        assert!(rights.contains(AclRight::Lookup));
        assert!(!rights.contains('0'));
        assert!(!rights.contains(AclRight::Custom('0')));
    }
}
