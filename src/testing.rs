//! Enable the test_helpers feature to expose helper methods to build
//! mock response structures for testing your code that uses the imap crate
//!
//! To use add a dev-dependency on the imap extension adding the feature "test_helpers"
//! e.g.
//!
//! [features]
//! tls-rustls = ["imap/rustls-tls"]
//! default = ["tls-rustls"]
//!
//! [dependencies]
//! imap = { version = "3.0", default-features = false }
//!
//! [dev-dependencies]
//! # mirror the same configuration your dependencies and add test_helpers
//! imap = { version = "3.0", default-features = false, features = ["test_helpers"] }
//!
use crate::error::Result;
use crate::extensions::list_status::ExtendedNames;
use crate::types::{
    AclResponse, Capabilities, Fetches, ListRightsResponse, MyRightsResponse, Names, QuotaResponse,
    QuotaRootResponse,
};
use std::sync::mpsc;

/// Builds an [`Capabilities`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n\
/// ";
pub fn build_capabilities(input: Vec<u8>) -> Result<Capabilities> {
    let (mut tx, _rx) = mpsc::channel();

    Capabilities::parse(input, &mut tx)
}

/// Builds an [`Fetches`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * 24 FETCH (FLAGS (\\Seen) UID 4827943)\r\n\
/// * 25 FETCH (FLAGS (\\Seen))\r\n\
/// ";
pub fn build_fetches(input: Vec<u8>) -> Result<Fetches> {
    let (mut tx, _rx) = mpsc::channel();

    Fetches::parse(input, &mut tx)
}

/// Builds an [`Names`] based on the provided input
/// Example input.
///
/// let input = b"\
/// * LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n\
/// ";
pub fn build_names(input: Vec<u8>) -> Result<Names> {
    let (mut tx, _rx) = mpsc::channel();

    Names::parse(input, &mut tx)
}

/// Builds an [`ExtendedNames`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * LIST () "." foo\r\n\
/// * STATUS foo (HITESTMODESEQ 122)\r\n\
/// * LIST () "." foo.bar\r\n\
/// * STATUS foo.bar (HIESTMODESEQ 132)\r\n
/// * LIST (\\UnMarked) \".\" feeds\r\n\
/// * LIST () \".\" feeds.test\r\n\
//  * STATUS feeds.test (HIGHESTMODSEQ 757)\r\n\
// ";
pub fn build_extended_names(input: Vec<u8>) -> Result<ExtendedNames> {
    let (mut tx, _rx) = mpsc::channel();

    ExtendedNames::parse(input, &mut tx)
}

/// Builds an [`AclResponse`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * ACL INBOX user1 lr user2 lrx\r\n\
/// ";
pub fn build_acl_response(input: Vec<u8>) -> Result<AclResponse> {
    let (mut tx, _rx) = mpsc::channel();

    AclResponse::parse(input, &mut tx)
}

/// Builds an [`ListRightsResponse`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * LISTRIGHTS INBOX myuser lr x k\r\n\
/// ";
pub fn build_list_rights_response(input: Vec<u8>) -> Result<ListRightsResponse> {
    let (mut tx, _rx) = mpsc::channel();

    ListRightsResponse::parse(input, &mut tx)
}

/// Builds an [`MyRightsResponse`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * MYRIGHTS INBOX lrxk\r\n\
/// ";
pub fn build_my_rights_response(input: Vec<u8>) -> Result<MyRightsResponse> {
    let (mut tx, _rx) = mpsc::channel();

    MyRightsResponse::parse(input, &mut tx)
}

/// Builds an [`QuotaResponse`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * * QUOTA my_root (STORAGE 10 500)\r\n\
/// ";
pub fn build_quota_response(input: Vec<u8>) -> Result<QuotaResponse> {
    let (mut tx, _rx) = mpsc::channel();

    QuotaResponse::parse(input, &mut tx)
}

/// Builds an [`QuotaRootResponse`] based on the provided input
///
/// Example input.
///
/// let input = b"\
/// * QUOTAROOT INBOX my_root\r\n\
//  * QUOTA my_root (STORAGE 10 500)\r\n\
/// ";
pub fn build_quota_root_response(input: Vec<u8>) -> Result<QuotaRootResponse> {
    let (mut tx, _rx) = mpsc::channel();

    QuotaRootResponse::parse(input, &mut tx)
}
