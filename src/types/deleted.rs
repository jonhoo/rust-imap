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
/// # #[cfg(feature = "tls")]
/// # {
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
///     for uid in deleted.uids() {
///         // Do something with uid
///     }
/// # }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum Deleted {
    /// Message sequence numbers given in an `EXPUNGE` response.
    Expunged(Vec<Seq>),
    /// Message UIDs given in a `VANISHED` response.
    Vanished(Vec<RangeInclusive<Uid>>),
}

impl Deleted {
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
    pub fn seqs(&self) -> impl Iterator<Item = Seq> + '_ {
        match self {
            Deleted::Expunged(s) => s.iter(),
            Deleted::Vanished(_) => [].iter(),
        }
        .copied()
    }

    /// Return an iterator over UIDs returned in a `VANISHED` response.
    /// If the client is expecting UIDs this function can be used to ensure
    /// only UIDs are processed.
    pub fn uids(&self) -> impl Iterator<Item = Uid> + '_ {
        match self {
            Deleted::Expunged(_) => [].iter(),
            Deleted::Vanished(s) => s.iter(),
        }
        .flat_map(|range| range.clone())
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
    type IntoIter = Box<dyn Iterator<Item = u32> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Deleted::Expunged(_) => Box::new(self.seqs()),
            Deleted::Vanished(_) => Box::new(self.uids()),
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
        let mut i = seqs.into_iter();
        assert_eq!(None, i.next());
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
        for seq in seqs.seqs() {
            count += 3;
            assert_eq!(seq, count);
        }
        assert_eq!(count, 12);
    }

    #[test]
    fn uids() {
        let uids: Deleted = Deleted::from_vanished(vec![1..=6]);
        let mut count: u32 = 0;
        for uid in uids.uids() {
            count += 1;
            assert_eq!(uid, count);
        }
        assert_eq!(count, 6);
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
