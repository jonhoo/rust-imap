use std::borrow::Cow;

/// A name that matches a `LIST` or `LSUB` command.
#[derive(Debug, Eq, PartialEq)]
pub struct Name {
    // Note that none of these fields are *actually* 'static.
    // Rather, they are tied to the lifetime of the `ZeroCopy` that contains this `Name`.
    pub(crate) attributes: Vec<NameAttribute<'static>>,
    pub(crate) delimiter: Option<&'static str>,
    pub(crate) name: &'static str,
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

impl NameAttribute<'static> {
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

impl<'a> From<&'a str> for NameAttribute<'a> {
    fn from(s: &'a str) -> Self {
        if let Some(f) = NameAttribute::system(s) {
            f
        } else {
            NameAttribute::Custom(Cow::Borrowed(s))
        }
    }
}

impl Name {
    /// Attributes of this name.
    pub fn attributes(&self) -> &[NameAttribute] {
        &self.attributes[..]
    }

    /// The hierarchy delimiter is a character used to delimit levels of hierarchy in a mailbox
    /// name.  A client can use it to create child mailboxes, and to search higher or lower levels
    /// of naming hierarchy.  All children of a top-level hierarchy node use the same
    /// separator character.  `None` means that no hierarchy exists; the name is a "flat" name.
    pub fn delimiter(&self) -> Option<&str> {
        self.delimiter
    }

    /// The name represents an unambiguous left-to-right hierarchy, and are valid for use as a
    /// reference in `LIST` and `LSUB` commands. Unless [`NameAttribute::NoSelect`] is indicated,
    /// the name is also valid as an argument for commands, such as `SELECT`, that accept mailbox
    /// names.
    pub fn name(&self) -> &str {
        self.name
    }
}
