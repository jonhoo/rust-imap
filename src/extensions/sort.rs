//! Adds support for the IMAP SORT extension specified in [RFC
//! 5464](https://tools.ietf.org/html/rfc5256#section-3).
//!
//! The SORT command is a variant of SEARCH with sorting semantics for
//! the results. There are two arguments before the searching
//! criteria argument: a parenthesized list of sort criteria, and the
//! searching charset.

use std::{borrow::Cow, fmt};

pub(crate) struct SortCriteria<'c>(pub(crate) &'c [SortCriterion<'c>]);

impl<'c> fmt::Display for SortCriteria<'c> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "")
        } else {
            let criteria: Vec<String> = self.0.iter().map(|c| c.to_string()).collect();
            write!(f, "({})", criteria.join(" "))
        }
    }
}

/// Message sorting preferences used for [`Session::sort`](crate::Session::sort)
/// and [`Session::uid_sort`](crate::Session::uid_sort).
///
/// Any sorting criterion that refers to an address (`From`, `To`, etc.) sorts according to the
/// "addr-mailbox" of the indicated address. You can find the formal syntax for addr-mailbox [in
/// the IMAP spec](https://tools.ietf.org/html/rfc3501#section-9), and a more detailed discussion
/// of the relevant semantics [in RFC 2822](https://tools.ietf.org/html/rfc2822#section-3.4.1).
/// Essentially, the address refers _either_ to the name of the contact _or_ to its local-part (the
/// left part of the email address, before the `@`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum SortCriterion<'c> {
    /// Internal date and time of the message. This differs from the
    /// ON criteria in SEARCH, which uses just the internal date.
    Arrival,

    /// IMAP addr-mailbox of the first "Cc" address.
    Cc,

    /// Sent date and time, as described in
    /// [section 2.2](https://tools.ietf.org/html/rfc5256#section-2.2).
    Date,

    /// IMAP addr-mailbox of the first "From" address.
    From,

    /// Followed by another sort criterion, has the effect of that
    /// criterion but in reverse (descending) order.
    Reverse(&'c SortCriterion<'c>),

    /// Size of the message in octets.
    Size,

    /// Base subject text.
    Subject,

    /// IMAP addr-mailbox of the first "To" address.
    To,
}

impl<'c> fmt::Display for SortCriterion<'c> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SortCriterion::*;

        match self {
            Arrival => write!(f, "ARRIVAL"),
            Cc => write!(f, "CC"),
            Date => write!(f, "DATE"),
            From => write!(f, "FROM"),
            Reverse(c) => write!(f, "REVERSE {}", c),
            Size => write!(f, "SIZE"),
            Subject => write!(f, "SUBJECT"),
            To => write!(f, "TO"),
        }
    }
}

/// The character encoding to use for strings that are subject to a [`SortCriterion`].
///
/// Servers are only required to implement [`SortCharset::UsAscii`] and [`SortCharset::Utf8`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SortCharset<'c> {
    /// Strings are UTF-8 encoded.
    Utf8,

    /// Strings are encoded with ASCII.
    UsAscii,

    /// Strings are encoded using some other character set.
    ///
    /// Note that this option is subject to server support for the specified character set.
    Custom(Cow<'c, str>),
}

impl<'c> fmt::Display for SortCharset<'c> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SortCharset::*;

        match self {
            Utf8 => write!(f, "UTF-8"),
            UsAscii => write!(f, "US-ASCII"),
            Custom(c) => write!(f, "{}", c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_criterion_to_string() {
        use SortCriterion::*;

        assert_eq!("ARRIVAL", Arrival.to_string());
        assert_eq!("CC", Cc.to_string());
        assert_eq!("DATE", Date.to_string());
        assert_eq!("FROM", From.to_string());
        assert_eq!("SIZE", Size.to_string());
        assert_eq!("SUBJECT", Subject.to_string());
        assert_eq!("TO", To.to_string());
        assert_eq!("REVERSE TO", Reverse(&To).to_string());
        assert_eq!("REVERSE REVERSE TO", Reverse(&Reverse(&To)).to_string());
    }

    #[test]
    fn test_criteria_to_string() {
        use SortCriterion::*;

        assert_eq!("", SortCriteria(&[]).to_string());
        assert_eq!("(ARRIVAL)", SortCriteria(&[Arrival]).to_string());
        assert_eq!(
            "(ARRIVAL REVERSE FROM)",
            SortCriteria(&[Arrival, Reverse(&From)]).to_string()
        );
        assert_eq!(
            "(ARRIVAL REVERSE REVERSE REVERSE FROM)",
            SortCriteria(&[Arrival, Reverse(&Reverse(&Reverse(&From)))]).to_string()
        );
    }

    #[test]
    fn test_charset_to_string() {
        use SortCharset::*;

        assert_eq!("UTF-8", Utf8.to_string());
        assert_eq!("US-ASCII", UsAscii.to_string());
        assert_eq!("CHARSET", Custom("CHARSET".into()).to_string());
    }
}
