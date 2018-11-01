mod mailbox;
pub use self::mailbox::Mailbox;

mod fetch;
pub use self::fetch::Fetch;

mod name;
pub use self::name::Name;

mod capabilities;
pub use self::capabilities::Capabilities;


/// re-exported from imap_proto;
pub use imap_proto::StatusAttribute;

/// Responses that the server sends that are not related to the current command.
/// [RFC 3501](https://tools.ietf.org/html/rfc3501#section-7) states that clients need to be able
/// to accept any response at any time. These are the ones we've encountered in the wild.
///
/// Note that `Recent`, `Exists` and `Expunge` responses refer to the currently `SELECT`ed folder,
/// so the user must take care when interpreting these.
#[derive(Debug, PartialEq, Eq)]
pub enum UnsolicitedResponse {
    Status(String, Vec<StatusAttribute>),
    Recent(u32),
    Exists(u32),
    Expunge(u32),
}


pub struct ZeroCopy<D> {
    _owned: Box<[u8]>,
    derived: D,
}

impl<D> ZeroCopy<D> {
    /// Derive a new `ZeroCopy` view of the byte data stored in `owned`.
    ///
    /// # Safety
    ///
    /// The `derive` callback will be passed a `&'static [u8]`. However, this reference is not, in
    /// fact `'static`. Instead, it is only valid for as long as the `ZeroCopy` lives. Therefore,
    /// it is *only* safe to call this function if *every* accessor on `D` returns either a type
    /// that does not contain any borrows, *or* where the return type is bound to the lifetime of
    /// `&self`.
    ///
    /// It is *not* safe for the error type `E` to borrow from the passed reference.
    pub unsafe fn new<F, E>(owned: Vec<u8>, derive: F) -> Result<Self, E>
    where
        F: FnOnce(&'static [u8]) -> Result<D, E>,
    {
        use std::mem;

        // the memory pointed to by `owned` now has a stable address (on the heap).
        // even if we move the `Box` (i.e., into `ZeroCopy`), a slice to it will remain valid.
        let _owned = owned.into_boxed_slice();

        // this is the unsafe part -- the implementor of `derive` must be aware that the reference
        // they are passed is not *really* 'static, but rather the lifetime of `&self`.
        let static_owned_ref: &'static [u8] = mem::transmute(&*_owned);

        Ok(ZeroCopy {
            _owned,
            derived: derive(static_owned_ref)?,
        })
    }
}

use super::error::Error;
pub type ZeroCopyResult<T> = Result<ZeroCopy<T>, Error>;

use std::ops::Deref;
impl<D> Deref for ZeroCopy<D> {
    type Target = D;
    fn deref(&self) -> &Self::Target {
        &self.derived
    }
}

// re-implement standard traits
// basically copied from Rc

impl<D: PartialEq> PartialEq for ZeroCopy<D> {
    fn eq(&self, other: &ZeroCopy<D>) -> bool {
        **self == **other
    }
}
impl<D: Eq> Eq for ZeroCopy<D> {}

use std::cmp::Ordering;
impl<D: PartialOrd> PartialOrd for ZeroCopy<D> {
    fn partial_cmp(&self, other: &ZeroCopy<D>) -> Option<Ordering> {
        (**self).partial_cmp(&**other)
    }
    fn lt(&self, other: &ZeroCopy<D>) -> bool {
        **self < **other
    }
    fn le(&self, other: &ZeroCopy<D>) -> bool {
        **self <= **other
    }
    fn gt(&self, other: &ZeroCopy<D>) -> bool {
        **self > **other
    }
    fn ge(&self, other: &ZeroCopy<D>) -> bool {
        **self >= **other
    }
}
impl<D: Ord> Ord for ZeroCopy<D> {
    fn cmp(&self, other: &ZeroCopy<D>) -> Ordering {
        (**self).cmp(&**other)
    }
}

use std::hash::{Hash, Hasher};
impl<D: Hash> Hash for ZeroCopy<D> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

use std::fmt;
impl<D: fmt::Display> fmt::Display for ZeroCopy<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}
impl<D: fmt::Debug> fmt::Debug for ZeroCopy<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, D> IntoIterator for &'a ZeroCopy<D>
where
    &'a D: IntoIterator,
{
    type Item = <&'a D as IntoIterator>::Item;
    type IntoIter = <&'a D as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        (**self).into_iter()
    }
}
