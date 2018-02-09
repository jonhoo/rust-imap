// Note that none of these fields are *actually* 'static.
// Rather, they are tied to the lifetime of the `ZeroCopy` that contains this `Name`.
#[derive(Debug, Eq, PartialEq)]
pub struct Name {
    pub(crate) attributes: Vec<&'static str>,
    pub(crate) delimiter: &'static str,
    pub(crate) name: &'static str,
}

impl Name {
    pub fn attributes<'a>(&'a self) -> &'a [&'a str] {
        &self.attributes[..]
    }

    pub fn delimiter<'a>(&'a self) -> &'a str {
        self.delimiter
    }

    pub fn name<'a>(&'a self) -> &'a str {
        self.name
    }
}
