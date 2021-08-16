use std::borrow::Cow;

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
#[non_exhaustive]
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

    /// Helper function to transform Strings into owned Flags
    pub fn from_strs<S: ToString>(
        v: impl IntoIterator<Item = S>,
    ) -> impl Iterator<Item = Flag<'static>> {
        v.into_iter().map(|s| Flag::from(s.to_string()))
    }
}

impl<'a> Flag<'a> {
    /// Get an owned version of the [`Flag`].
    pub fn into_owned(self) -> Flag<'static> {
        match self {
            Flag::Custom(cow) => Flag::Custom(Cow::Owned(cow.into_owned())),
            Flag::Seen => Flag::Seen,
            Flag::Answered => Flag::Answered,
            Flag::Flagged => Flag::Flagged,
            Flag::Deleted => Flag::Deleted,
            Flag::Draft => Flag::Draft,
            Flag::Recent => Flag::Recent,
            Flag::MayCreate => Flag::MayCreate,
        }
    }
}

impl<'a> std::fmt::Display for Flag<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Flag::Seen => write!(f, "\\Seen"),
            Flag::Answered => write!(f, "\\Answered"),
            Flag::Flagged => write!(f, "\\Flagged"),
            Flag::Deleted => write!(f, "\\Deleted"),
            Flag::Draft => write!(f, "\\Draft"),
            Flag::Recent => write!(f, "\\Recent"),
            Flag::MayCreate => write!(f, "\\*"),
            Flag::Custom(ref s) => write!(f, "{}", s),
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
