// Note that none of these fields are *actually* 'static.
// Rather, they are tied to the lifetime of the `ZeroCopy` that contains this `Name`.
use std::collections::HashSet;
use std::collections::hash_set::Iter;
pub struct Capabilities(pub(crate) HashSet<&'static str>);

use std::borrow::Borrow;
use std::hash::Hash;
impl Capabilities {
    pub fn has<S: ?Sized>(&self, s: &S) -> bool
    where
        for<'a> &'a str: Borrow<S>,
        S: Hash + Eq,
    {
        self.0.contains(s)
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, &'a str> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
