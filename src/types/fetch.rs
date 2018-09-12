// Note that none of these fields are *actually* 'static.
// Rather, they are tied to the lifetime of the `ZeroCopy` that contains this `Name`.
#[derive(Debug, Eq, PartialEq)]
pub struct Fetch {
    pub message: u32,
    pub(crate) flags: Vec<&'static str>,
    pub uid: Option<u32>,
    pub(crate) rfc822_header: Option<&'static [u8]>,
    pub(crate) rfc822: Option<&'static [u8]>,
    pub(crate) body: Option<&'static [u8]>,
}

impl Fetch {
    pub fn flags(&self) -> &[&str] {
        &self.flags[..]
    }

    pub fn rfc822_header(&self) -> Option<&[u8]> {
        self.rfc822_header
    }

    pub fn rfc822(&self) -> Option<&[u8]> {
        self.rfc822
    }

    pub fn body(&self) -> Option<&[u8]> {
        self.body
    }
}
