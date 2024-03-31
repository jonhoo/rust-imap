use super::{Seq, Uid};
use std::ops::RangeInclusive;

/// A struct containing message sequence numbers or UID sequence sets and a mod
/// sequence returned in response to a `EXPUNGE` command.
///
/// The `EXPUNGE` command may return several `EXPUNGE` responses referencing
/// message sequence numbers, or it may return a `VANISHED` response referencing
/// multiple UID values in a sequence set if the client has enabled
/// [QRESYNC](https://tools.ietf.org/html/rfc7162#section-3.2.7). If `QRESYNC` is
/// enabled, the server will also return the mod sequence of the completed
/// operation.
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
/// # {} #[cfg(feature = "native-tls")]
/// # fn main() {
/// # let client = imap::ClientBuilder::new("imap.example.com", 993)
///     .connect().unwrap();
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
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Deleted {
    /// The list of messages that were expunged
    pub messages: DeletedMessages,
    /// The mod sequence of the performed operation, if the `QRESYNC` extension
    /// is enabled.
    pub mod_seq: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum DeletedMessages {
    /// Message sequence numbers given in an `EXPUNGE` response.
    Expunged(Vec<Seq>),
    /// Message UIDs given in a `VANISHED` response.
    Vanished(Vec<RangeInclusive<Uid>>),
}

impl Deleted {
    /// Construct a new `Deleted` value from a vector of message sequence
    /// numbers returned in one or more `EXPUNGE` responses.
    pub fn from_expunged(v: Vec<u32>, mod_seq: Option<u64>) -> Self {
        Self {
            messages: DeletedMessages::Expunged(v),
            mod_seq,
        }
    }

    /// Construct a new `Deleted` value from a sequence-set of UIDs
    /// returned in a `VANISHED` response
    pub fn from_vanished(v: Vec<RangeInclusive<u32>>, mod_seq: Option<u64>) -> Self {
        Self {
            messages: DeletedMessages::Vanished(v),
            mod_seq,
        }
    }

    /// Return an iterator over message sequence numbers from an `EXPUNGE`
    /// response. If the client is expecting sequence numbers this function
    /// can be used to ensure only sequence numbers returned in an `EXPUNGE`
    /// response are processed.
    pub fn seqs(&self) -> impl Iterator<Item = Seq> + '_ {
        match &self.messages {
            DeletedMessages::Expunged(s) => s.iter(),
            DeletedMessages::Vanished(_) => [].iter(),
        }
        .copied()
    }

    /// Return an iterator over UIDs returned in a `VANISHED` response.
    /// If the client is expecting UIDs this function can be used to ensure
    /// only UIDs are processed.
    pub fn uids(&self) -> impl Iterator<Item = Uid> + '_ {
        match &self.messages {
            DeletedMessages::Expunged(_) => [].iter(),
            DeletedMessages::Vanished(s) => s.iter(),
        }
        .flat_map(|range| range.clone())
    }

    /// Return if the set is empty
    pub fn is_empty(&self) -> bool {
        match &self.messages {
            DeletedMessages::Expunged(v) => v.is_empty(),
            DeletedMessages::Vanished(v) => v.is_empty(),
        }
    }
}

impl<'a> IntoIterator for &'a Deleted {
    type Item = u32;
    type IntoIter = Box<dyn Iterator<Item = u32> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        match &self.messages {
            DeletedMessages::Expunged(_) => Box::new(self.seqs()),
            DeletedMessages::Vanished(_) => Box::new(self.uids()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn seq() {
        let seqs = Deleted::from_expunged(vec![3, 6, 9, 12], None);
        let mut i = seqs.into_iter();
        assert_eq!(Some(3), i.next());
        assert_eq!(Some(6), i.next());
        assert_eq!(Some(9), i.next());
        assert_eq!(Some(12), i.next());
        assert_eq!(None, i.next());

        let seqs = Deleted::from_expunged(vec![], None);
        let mut i = seqs.into_iter();
        assert_eq!(None, i.next());
    }

    #[test]
    fn seq_set() {
        let uids = Deleted::from_vanished(vec![1..=1, 3..=5, 8..=9, 12..=12], None);
        let mut i = uids.into_iter();
        assert_eq!(Some(1), i.next());
        assert_eq!(Some(3), i.next());
        assert_eq!(Some(4), i.next());
        assert_eq!(Some(5), i.next());
        assert_eq!(Some(8), i.next());
        assert_eq!(Some(9), i.next());
        assert_eq!(Some(12), i.next());
        assert_eq!(None, i.next());

        let uids = Deleted::from_vanished(vec![], None);
        assert_eq!(None, uids.into_iter().next());
    }

    #[test]
    fn seqs() {
        let seqs: Deleted = Deleted::from_expunged(vec![3, 6, 9, 12], None);
        let mut count: u32 = 0;
        for seq in seqs.seqs() {
            count += 3;
            assert_eq!(seq, count);
        }
        assert_eq!(count, 12);
    }

    #[test]
    fn uids() {
        let uids: Deleted = Deleted::from_vanished(vec![1..=6], None);
        let mut count: u32 = 0;
        for uid in uids.uids() {
            count += 1;
            assert_eq!(uid, count);
        }
        assert_eq!(count, 6);
    }

    #[test]
    fn generic_iteration() {
        let seqs: Deleted = Deleted::from_expunged(vec![3, 6, 9, 12], None);
        let mut count: u32 = 0;
        for seq in &seqs {
            count += 3;
            assert_eq!(seq, count);
        }
        assert_eq!(count, 12);

        let uids: Deleted = Deleted::from_vanished(vec![1..=6], None);
        let mut count: u32 = 0;
        for uid in &uids {
            count += 1;
            assert_eq!(uid, count);
        }
        assert_eq!(count, 6);
    }
}
