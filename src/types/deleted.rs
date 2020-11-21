use super::{Seq, Uid};
use std::ops::RangeInclusive;

/// An enum representing message sequence numbers or UID sequence sets returned
/// in response to a `EXPUNGE` command.
///
/// The `EXPUNGE` command may return several `EXPUNGE` responses referencing
/// message sequence numbers, or it may return a `VANISHED` response referencing
/// multiple UID values in a sequence set if the client has enabled
/// [QRESYNC](https://tools.ietf.org/html/rfc7162#section-3.2.7).
///
/// `Deleted` implements some iterators to make it easy to use. If the caller
/// knows that they should be receiving an `EXPUNGE` or `VANISHED` response,
/// then they can use [`seqs()`](#method.seqs) to get an iterator over `EXPUNGE`
/// message sequence numbers, or [`uids()`](#method.uids) to get an iterator over
/// the `VANISHED` UIDs. As a convenience `Deleted` also implents `IntoIterator`
/// which just returns an iterator over whatever is contained within.
///
/// # Examples
/// ```no_run
/// # let domain = "imap.example.com";
/// # let tls = native_tls::TlsConnector::builder().build().unwrap();
/// # let client = imap::connect((domain, 993), domain, &tls).unwrap();
/// # let mut session = client.login("name", "pw").unwrap();
/// // Iterate over whatever is returned
/// if let Ok(deleted) = session.expunge() {
///     for id in &deleted {
///         // Do something with id
///     }
/// }
///
/// // Expect a VANISHED response with UIDs
/// if let Ok(deleted) = session.expunge() {
///     if let Some(uid_iter) = deleted.uids() {
///         for uid in uid_iter {
///             // Do something with uid
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum Deleted {
    /// Message sequence numbers given in an `EXPUNGE` response.
    Expunged(Vec<Seq>),
    /// Message UIDs given in a `VANISHED` response.
    Vanished(Vec<RangeInclusive<Uid>>),
}

impl<'a> Deleted {
    /// Construct a new `Deleted` value from a vector of message sequence
    /// numbers returned in one or more `EXPUNGE` responses.
    pub fn from_expunged(v: Vec<u32>) -> Self {
        Deleted::Expunged(v)
    }

    /// Construct a new `Deleted` value from a sequence-set of UIDs
    /// returned in a `VANISHED` response
    pub fn from_vanished(v: Vec<RangeInclusive<u32>>) -> Self {
        Deleted::Vanished(v)
    }

    /// Return an iterator over message sequence numbers from an `EXPUNGE`
    /// response. If the client is expecting sequence numbers this function
    /// can be used to ensure only sequence numbers returned in an `EXPUNGE`
    /// response are processed.
    pub fn seqs(&'a self) -> Option<DeletedIterator<'a>> {
        match self {
            Deleted::Expunged(s) => Some(DeletedIterator::Seq(s.into())),
            Deleted::Vanished(_) => None,
        }
    }

    /// Return an iterator over UIDs returned in a `VANISHED` response.
    /// If the client is expecting UIDs this function can be used to ensure
    /// only UIDs are processed.
    pub fn uids(&'a self) -> Option<DeletedIterator<'a>> {
        match self {
            Deleted::Expunged(_) => None,
            Deleted::Vanished(s) => Some(DeletedIterator::Set(s.into())),
        }
    }

    /// Return if the set is empty
    pub fn is_empty(&self) -> bool {
        match self {
            Deleted::Expunged(v) => v.is_empty(),
            Deleted::Vanished(v) => v.is_empty(),
        }
    }
}

impl<'a> IntoIterator for &'a Deleted {
    type Item = u32;
    type IntoIter = DeletedIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Deleted::Expunged(s) => DeletedIterator::Seq(s.into()),
            Deleted::Vanished(s) => DeletedIterator::Set(s.into()),
        }
    }
}

/// An iterator over the items in a `Deleted` value.
#[derive(Debug)]
pub enum DeletedIterator<'a> {
    Seq(SeqIter<'a>),
    Set(SeqSetIter<'a>),
}

impl<'a> Iterator for DeletedIterator<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DeletedIterator::Seq(set) => set.next(),
            DeletedIterator::Set(set) => set.next(),
        }
    }
}

/// An iterator over a vector of sequence numbers, as returned in one or
/// more `EXPUNGE` responses.
#[derive(Debug)]
pub struct SeqIter<'a>(std::slice::Iter<'a, u32>);

impl<'a> From<&'a Vec<u32>> for SeqIter<'a> {
    fn from(v: &'a Vec<u32>) -> Self {
        SeqIter(v.iter())
    }
}

impl<'a> Iterator for SeqIter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().copied()
    }
}

/// An iterator of a sequence-set, as returned in a `VANISHED` response.
///
/// A sequence-set is defined in [RFC 3501](https://tools.ietf.org/html/rfc3501)
/// as a comma separated list of numbers or number ranges. This iterator returns
/// each number in the set by iterating over elements in the list and iterating
/// though each member of any number ranges.
#[derive(Debug)]
pub struct SeqSetIter<'a> {
    sets: std::slice::Iter<'a, RangeInclusive<u32>>,
    set: Option<RangeInclusive<u32>>,
}

impl<'a> From<&'a Vec<RangeInclusive<u32>>> for SeqSetIter<'a> {
    fn from(v: &'a Vec<RangeInclusive<u32>>) -> Self {
        let mut sets = v.iter();
        let set = sets.next().cloned();
        SeqSetIter { sets, set }
    }
}

impl<'a> Iterator for SeqSetIter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(range) = &mut self.set {
                if let Some(uid) = range.next() {
                    return Some(uid);
                } else {
                    self.set = self.sets.next().cloned();
                }
            } else {
                // exhausted
                return None;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn seq() {
        let seqs = Deleted::from_expunged(vec![3, 6, 9, 12]);
        let mut i = seqs.into_iter();
        assert_eq!(Some(3), i.next());
        assert_eq!(Some(6), i.next());
        assert_eq!(Some(9), i.next());
        assert_eq!(Some(12), i.next());
        assert_eq!(None, i.next());

        let seqs = Deleted::from_expunged(vec![]);
        assert_eq!(None, seqs.into_iter().next())
    }

    #[test]
    fn seq_set() {
        let uids = Deleted::from_vanished(vec![1..=1, 3..=5, 8..=9, 12..=12]);
        let mut i = uids.into_iter();
        assert_eq!(Some(1), i.next());
        assert_eq!(Some(3), i.next());
        assert_eq!(Some(4), i.next());
        assert_eq!(Some(5), i.next());
        assert_eq!(Some(8), i.next());
        assert_eq!(Some(9), i.next());
        assert_eq!(Some(12), i.next());
        assert_eq!(None, i.next());

        let uids = Deleted::from_vanished(vec![]);
        assert_eq!(None, uids.into_iter().next());
    }

    #[test]
    fn seqs() {
        let seqs: Deleted = Deleted::from_expunged(vec![3, 6, 9, 12]);
        let mut count: u32 = 0;
        for seq in seqs.seqs().unwrap() {
            count += 3;
            assert_eq!(seq, count);
        }
        assert_eq!(count, 12);
    }

    #[test]
    fn uids() {
        let uids: Deleted = Deleted::from_vanished(vec![1..=6]);
        let mut count: u32 = 0;
        if let Some(uid_iter) = uids.uids() {
            for uid in uid_iter {
                count += 1;
                assert_eq!(uid, count);
            }
            assert_eq!(count, 6);
        } else {
            panic!("uids() returned no uids!");
        }
    }

    #[test]
    fn generic_iteration() {
        let seqs: Deleted = Deleted::from_expunged(vec![3, 6, 9, 12]);
        let mut count: u32 = 0;
        for seq in &seqs {
            count += 3;
            assert_eq!(seq, count);
        }
        assert_eq!(count, 12);

        let uids: Deleted = Deleted::from_vanished(vec![1..=6]);
        let mut count: u32 = 0;
        for uid in &uids {
            count += 1;
            assert_eq!(uid, count);
        }
        assert_eq!(count, 6);
    }
}
