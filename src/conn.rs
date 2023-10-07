use crate::extensions::idle::SetReadTimeout;

use std::fmt::{Debug, Formatter};
use std::io::{Read, Write};

/// Imap connection trait of a read/write stream
pub trait ImapConnection: Read + Write + Send + SetReadTimeout + private::Sealed {}

impl<T> ImapConnection for T where T: Read + Write + Send + SetReadTimeout {}

impl Debug for dyn ImapConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Imap connection")
    }
}

/// A boxed connection type
pub type Connection = Box<dyn ImapConnection>;

mod private {
    use super::{Read, SetReadTimeout, Write};

    pub trait Sealed {}

    impl<T> Sealed for T where T: Read + Write + SetReadTimeout {}
}
