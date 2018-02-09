// Note that none of these fields are *actually* 'static.
// Rather, they are tied to the lifetime of the `ZeroCopy` that contains this `Name`.
#[derive(Debug, Eq, PartialEq)]
pub struct Fetch {
    pub message: u32,
    pub(crate) flags: Vec<&'static str>,
    pub uid: Option<u32>,
    pub(crate) rfc822: Option<&'static [u8]>,
}

impl Fetch {
    pub fn flags<'a>(&'a self) -> &'a [&'a str] {
        &self.flags[..]
    }

    pub fn rfc822<'a>(&'a self) -> Option<&'a [u8]> {
        self.rfc822
    }
}
