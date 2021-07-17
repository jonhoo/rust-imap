use crate::error::Error;
use crate::parse::{parse_many_into, MapOrNot};
use crate::types::UnsolicitedResponse;
use imap_proto::{MailboxDatum, Response};
use ouroboros::self_referencing;
use std::borrow::Cow;
use std::slice::Iter;
use std::sync::mpsc;

/// A wrapper for one or more [`Name`] responses.
#[self_referencing]
pub struct Names {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) names: Vec<Name<'this>>,
}

impl Names {
    /// Parse one or more [`Name`] from a response buffer
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        NamesTryBuilder {
            data: owned,
            names_builder: |input| {
                let mut names = Vec::new();
                parse_many_into(input, &mut names, unsolicited, |response| match response {
                    Response::MailboxData(MailboxDatum::List {
                        flags,
                        delimiter,
                        name,
                    }) => Ok(MapOrNot::Map(Name {
                        attributes: flags.into_iter().map(NameAttribute::from).collect(),
                        delimiter,
                        name,
                    })),
                    resp => Ok(MapOrNot::Not(resp)),
                })?;
                Ok(names)
            },
        }
        .try_build()
    }

    /// Iterate over the contained [`Name`]s
    pub fn iter(&self) -> Iter<'_, Name<'_>> {
        self.borrow_names().iter()
    }

    /// Get the number of [`Name`]s in this container.
    pub fn len(&self) -> usize {
        self.borrow_names().len()
    }

    /// Return true of there are no [`Name`]s in the container.
    pub fn is_empty(&self) -> bool {
        self.borrow_names().is_empty()
    }
}

/// A name that matches a `LIST` or `LSUB` command.
#[derive(Debug, Eq, PartialEq)]
pub struct Name<'a> {
    pub(crate) attributes: Vec<NameAttribute<'a>>,
    pub(crate) delimiter: Option<Cow<'a, str>>,
    pub(crate) name: Cow<'a, str>,
}

/// An attribute set for an IMAP name.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum NameAttribute<'a> {
    /// It is not possible for any child levels of hierarchy to exist
    /// under this name; no child levels exist now and none can be
    /// created in the future.
    NoInferiors,

    /// It is not possible to use this name as a selectable mailbox.
    NoSelect,

    /// The mailbox has been marked "interesting" by the server; the
    /// mailbox probably contains messages that have been added since
    /// the last time the mailbox was selected.
    Marked,

    /// The mailbox does not contain any additional messages since the
    /// last time the mailbox was selected.
    Unmarked,

    /// A non-standard user- or server-defined name attribute.
    Custom(Cow<'a, str>),
}

impl<'a> NameAttribute<'a> {
    fn system(s: &str) -> Option<Self> {
        match s {
            "\\Noinferiors" => Some(NameAttribute::NoInferiors),
            "\\Noselect" => Some(NameAttribute::NoSelect),
            "\\Marked" => Some(NameAttribute::Marked),
            "\\Unmarked" => Some(NameAttribute::Unmarked),
            _ => None,
        }
    }
}

impl<'a> From<String> for NameAttribute<'a> {
    fn from(s: String) -> Self {
        if let Some(f) = NameAttribute::system(&s) {
            f
        } else {
            NameAttribute::Custom(Cow::Owned(s))
        }
    }
}

impl<'a> From<Cow<'a, str>> for NameAttribute<'a> {
    fn from(s: Cow<'a, str>) -> Self {
        if let Some(f) = NameAttribute::system(&*s) {
            f
        } else {
            NameAttribute::Custom(s)
        }
    }
}

impl<'a> From<&'a str> for NameAttribute<'a> {
    fn from(s: &'a str) -> Self {
        if let Some(f) = NameAttribute::system(s) {
            f
        } else {
            NameAttribute::Custom(Cow::Borrowed(s))
        }
    }
}

impl<'a> Name<'a> {
    /// Attributes of this name.
    pub fn attributes(&self) -> &[NameAttribute<'a>] {
        &self.attributes[..]
    }

    /// The hierarchy delimiter is a character used to delimit levels of hierarchy in a mailbox
    /// name.  A client can use it to create child mailboxes, and to search higher or lower levels
    /// of naming hierarchy.  All children of a top-level hierarchy node use the same
    /// separator character.  `None` means that no hierarchy exists; the name is a "flat" name.
    pub fn delimiter(&self) -> Option<&str> {
        self.delimiter.as_deref()
    }

    /// The name represents an unambiguous left-to-right hierarchy, and are valid for use as a
    /// reference in `LIST` and `LSUB` commands. Unless [`NameAttribute::NoSelect`] is indicated,
    /// the name is also valid as an argument for commands, such as `SELECT`, that accept mailbox
    /// names.
    pub fn name(&self) -> &str {
        &*self.name
    }
}
