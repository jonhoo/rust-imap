//! Enable the test_helpers feature to expose helper methods to build
//! mock response structures for testing your code that uses the imap crate
//!
//! To use add a dev-dependency on the imap extension adding the feature "test_helpers"
//! e.g.
//!
//! ```toml
//! [dependencies]
//! imap = { version = "3.0" }
//!
//! [dev-dependencies]
//! # mirror the same configuration your dependencies and add test_helpers
//! imap = { version = "3.0", features = ["test_helpers"] }
//! ```
//!
#[cfg(doc)]
use crate::{extensions::list_status::ExtendedNames, types::*};

/// Methods to build a [`Capabilities`] response object
pub mod capabilities {
    use crate::types::Capabilities;
    use std::collections::VecDeque;

    /// Builds an [`Capabilities`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "* CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n";
    /// let response = imap::testing::capabilities::parse(input);
    /// ```
    pub fn parse(input: impl Into<Vec<u8>>) -> Capabilities {
        let mut unsolicited_responses = VecDeque::new();

        Capabilities::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`Fetches`] response object
pub mod fetches {
    use crate::types::Fetches;
    use std::collections::VecDeque;

    /// Builds an [`Fetches`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "\
    /// * 24 FETCH (FLAGS (\\Seen) UID 4827943)\r\n\
    /// * 25 FETCH (FLAGS (\\Seen))\r\n\
    /// ";
    /// let response = imap::testing::fetches::parse(input);
    /// ```
    pub fn parse(input: impl Into<Vec<u8>>) -> Fetches {
        let mut unsolicited_responses = VecDeque::new();

        Fetches::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`Names`] response object
pub mod names {
    use crate::types::Names;
    use std::collections::VecDeque;

    /// Builds an [`Names`] based on the provided input
    /// Example input.
    ///
    /// ```
    /// let input = "\
    /// * LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n\
    /// ";
    /// let response = imap::testing::names::parse(input);
    ///```
    pub fn parse(input: impl Into<Vec<u8>>) -> Names {
        let mut unsolicited_responses = VecDeque::new();

        Names::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`ExtendedNames`] response object
pub mod extended_names {
    use crate::extensions::list_status::ExtendedNames;
    use std::collections::VecDeque;

    /// Builds an [`ExtendedNames`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "\
    /// * LIST () \".\" foo\r\n\
    /// * STATUS foo (HIGHESTMODSEQ 122)\r\n\
    /// * LIST () \".\" foo.bar\r\n\
    /// * STATUS foo.bar (HIGHESTMODSEQ 132)\r\n\
    /// * LIST (\\UnMarked) \".\" feeds\r\n\
    /// * LIST () \".\" feeds.test\r\n\
    /// * STATUS feeds.test (HIGHESTMODSEQ 757)\r\n\
    /// ";
    /// let response = imap::testing::extended_names::parse(input);
    /// ```
    pub fn parse(input: impl Into<Vec<u8>>) -> ExtendedNames {
        let mut unsolicited_responses = VecDeque::new();

        ExtendedNames::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`AclResponse`] response object
pub mod acl_response {
    use crate::types::AclResponse;
    use std::collections::VecDeque;

    /// Builds an [`AclResponse`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "* ACL INBOX user1 lr user2 lrx\r\n";
    /// let response = imap::testing::acl_response::parse(input);
    /// ```
    pub fn parse(input: impl Into<Vec<u8>>) -> AclResponse {
        let mut unsolicited_responses = VecDeque::new();

        AclResponse::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`ListRightsResponse`] response object
pub mod list_rights_response {
    use crate::types::ListRightsResponse;
    use std::collections::VecDeque;

    /// Builds an [`ListRightsResponse`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "* LISTRIGHTS INBOX myuser lr x k\r\n";
    /// let response = imap::testing::list_rights_response::parse(input);
    ///```
    pub fn parse(input: impl Into<Vec<u8>>) -> ListRightsResponse {
        let mut unsolicited_responses = VecDeque::new();

        ListRightsResponse::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`MyRightsResponse`] response object
pub mod my_rights_response {
    use crate::types::MyRightsResponse;
    use std::collections::VecDeque;

    /// Builds an [`MyRightsResponse`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "* MYRIGHTS INBOX lrxk\r\n";
    /// let response = imap::testing::my_rights_response::parse(input);
    /// ```
    pub fn parse(input: impl Into<Vec<u8>>) -> MyRightsResponse {
        let mut unsolicited_responses = VecDeque::new();

        MyRightsResponse::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`QuotaResponse`] response object
pub mod quota_response {
    use crate::types::QuotaResponse;
    use std::collections::VecDeque;

    /// Builds an [`QuotaResponse`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "* QUOTA my_root (STORAGE 10 500)\r\n";
    /// let response = imap::testing::quota_response::parse(input);
    /// ```
    pub fn parse(input: impl Into<Vec<u8>>) -> QuotaResponse {
        let mut unsolicited_responses = VecDeque::new();

        QuotaResponse::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}

/// Methods to build a [`QuotaRootResponse`] response object
pub mod quota_root_response {
    use crate::types::QuotaRootResponse;
    use std::collections::VecDeque;

    /// Builds an [`QuotaRootResponse`] based on the provided input
    ///
    /// Example input.
    ///
    /// ```
    /// let input = "\
    /// * QUOTAROOT INBOX my_root\r\n\
    /// * QUOTA my_root (STORAGE 10 500)\r\n\
    /// ";
    /// let response = imap::testing::quota_root_response::parse(input);
    /// ```
    pub fn parse(input: impl Into<Vec<u8>>) -> QuotaRootResponse {
        let mut unsolicited_responses = VecDeque::new();

        QuotaRootResponse::parse(input.into(), &mut unsolicited_responses).unwrap()
    }
}
