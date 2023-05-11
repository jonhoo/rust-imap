extern crate chrono;
extern crate imap;
extern crate lettre;
extern crate native_tls;

use chrono::{FixedOffset, TimeZone};
use lettre::Transport;
use std::collections::VecDeque;
use std::net::TcpStream;

use crate::imap::extensions::sort::{SortCharset, SortCriterion};
use crate::imap::types::Mailbox;

fn tls() -> native_tls::TlsConnector {
    native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .unwrap()
}

fn test_host() -> String {
    std::env::var("TEST_HOST").unwrap_or("127.0.0.1".to_string())
}

fn test_smtp_host() -> String {
    std::env::var("TEST_SMTP_HOST")
        .unwrap_or_else(|_| std::env::var("TEST_HOST").unwrap_or("127.0.0.1".to_string()))
}

fn test_imap_port() -> u16 {
    std::env::var("TEST_IMAP_PORT")
        .unwrap_or("3143".to_string())
        .parse()
        .unwrap_or(3143)
}

fn test_imaps_port() -> u16 {
    std::env::var("TEST_IMAPS_PORT")
        .unwrap_or("3993".to_string())
        .parse()
        .unwrap_or(3993)
}

fn test_smtps_port() -> u16 {
    std::env::var("TEST_SMTPS_PORT")
        .unwrap_or("3465".to_string())
        .parse()
        .unwrap_or(3465)
}

fn clean_mailbox(session: &mut imap::Session<native_tls::TlsStream<TcpStream>>) {
    session.select("INBOX").unwrap();
    let inbox = session.search("ALL").unwrap();
    if !inbox.is_empty() {
        session
            .store(
                inbox
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<String>>()
                    .join(","),
                "+FLAGS (\\Deleted)",
            )
            .unwrap();
    }
    session.expunge().unwrap();
}

fn wait_for_delivery() {
    std::thread::sleep(std::time::Duration::from_millis(500));
}

fn session_with_options(
    user: &str,
    clean: bool,
) -> imap::Session<native_tls::TlsStream<TcpStream>> {
    let host = test_host();
    let mut s = imap::ClientBuilder::new(&host, test_imaps_port())
        .connect(|domain, tcp| {
            let ssl_conn = tls();
            Ok(native_tls::TlsConnector::connect(&ssl_conn, domain, tcp).unwrap())
        })
        .unwrap()
        .login(user, user)
        .unwrap();
    s.debug = true;
    if clean {
        clean_mailbox(&mut s);
    }
    s
}

fn get_greeting() -> String {
    let host = test_host();
    let tcp = TcpStream::connect((host.as_ref(), test_imap_port())).unwrap();
    let mut client = imap::Client::new(tcp);
    let greeting = client.read_greeting().unwrap();
    String::from_utf8(greeting).unwrap()
}

fn delete_mailbox(s: &mut imap::Session<native_tls::TlsStream<TcpStream>>, mailbox: &str) {
    // we are silently eating any error (e.g. mailbox does not exist)
    s.set_acl(
        mailbox,
        "cyrus",
        &"x".try_into().unwrap(),
        imap::types::AclModifyMode::Replace,
    )
    .unwrap_or(());

    s.delete(mailbox).unwrap_or(());
}

fn session(user: &str) -> imap::Session<native_tls::TlsStream<TcpStream>> {
    session_with_options(user, true)
}

fn smtp(user: &str) -> lettre::SmtpTransport {
    use lettre::{
        transport::smtp::{
            authentication::Credentials,
            client::{Tls, TlsParameters},
        },
        SmtpTransport,
    };

    let creds = Credentials::new(user.to_string(), user.to_string());
    let hostname = test_smtp_host();
    let tls = TlsParameters::builder(hostname.clone())
        .dangerous_accept_invalid_certs(true)
        .dangerous_accept_invalid_hostnames(true)
        .build()
        .unwrap();
    SmtpTransport::builder_dangerous(hostname)
        .port(test_smtps_port())
        .tls(Tls::Wrapper(tls))
        .credentials(creds)
        .build()
}

#[test]
#[cfg(feature = "test-full-imap")]
fn connect_insecure_then_secure() {
    let host = test_host();
    // Not supported on greenmail because of https://github.com/greenmail-mail-test/greenmail/issues/135
    imap::ClientBuilder::new(&host, test_imap_port())
        .starttls()
        .connect(|domain, tcp| {
            let ssl_conn = tls();
            Ok(native_tls::TlsConnector::connect(&ssl_conn, domain, tcp).unwrap())
        })
        .unwrap();
}

#[test]
fn connect_secure() {
    let host = test_host();
    imap::ClientBuilder::new(&host, test_imaps_port())
        .connect(|domain, tcp| {
            let ssl_conn = tls();
            Ok(native_tls::TlsConnector::connect(&ssl_conn, domain, tcp).unwrap())
        })
        .unwrap();
}

#[test]
fn login() {
    session("readonly-test@localhost");
}

#[test]
fn logout() {
    let mut s = session("readonly-test@localhost");
    s.logout().unwrap();
}

#[test]
fn inbox_zero() {
    // https://github.com/greenmail-mail-test/greenmail/issues/265
    let mut s = session("readonly-test@localhost");
    s.select("INBOX").unwrap();
    let inbox = s.search("ALL").unwrap();
    assert_eq!(inbox.len(), 0);
}

#[test]
fn inbox() {
    let to = "inbox@localhost";

    // first log in so we'll see the unsolicited e-mails
    let mut c = session(to);
    c.select("INBOX").unwrap();

    // then send the e-mail
    let s = smtp(to);
    let e = lettre::message::Message::builder()
        .from("sender@localhost".parse().unwrap())
        .to(to.parse().unwrap())
        .subject("My first e-mail")
        .body("Hello world from SMTP".to_string())
        .unwrap();
    s.send(&e.into()).unwrap();

    // send a second e-mail
    let e = lettre::message::Message::builder()
        .from("sender2@localhost".parse().unwrap())
        .to(to.parse().unwrap())
        .subject("My second e-mail")
        .body("Hello world from SMTP".to_string())
        .unwrap();
    s.send(&e.into()).unwrap();

    wait_for_delivery();

    // now we should see the e-mails!
    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 2);
    assert!(inbox.contains(&1));
    assert!(inbox.contains(&2));

    // we should also get two unsolicited responses: Exists and Recent
    c.noop().unwrap();
    let mut unsolicited = VecDeque::new();
    while let Some(m) = c.unsolicited_responses.pop_front() {
        unsolicited.push_back(m);
    }
    assert_eq!(unsolicited.len(), 2);
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Exists(2)));
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Recent(2)));

    // let's see that we can also fetch the e-mails
    let fetch = c.fetch("1", "(ENVELOPE INTERNALDATE UID)").unwrap();
    assert_eq!(fetch.len(), 1);
    let fetch = fetch.iter().next().unwrap();
    assert_eq!(fetch.message, 1);
    assert_ne!(fetch.uid, None);
    let e = fetch.envelope().unwrap();
    assert_eq!(e.subject, Some(b"My first e-mail"[..].into()));
    assert_ne!(e.from, None);
    assert_eq!(e.from.as_ref().unwrap().len(), 1);
    let from = &e.from.as_ref().unwrap()[0];
    assert_eq!(from.mailbox, Some(b"sender"[..].into()));
    assert_eq!(from.host, Some(b"localhost"[..].into()));
    assert_ne!(e.to, None);
    assert_eq!(e.to.as_ref().unwrap().len(), 1);
    let to = &e.to.as_ref().unwrap()[0];
    assert_eq!(to.mailbox, Some(b"inbox"[..].into()));
    assert_eq!(to.host, Some(b"localhost"[..].into()));
    let date_opt = fetch.internal_date();
    assert!(date_opt.is_some());

    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 2);

    // e-mails should be sorted by subject
    let inbox = c
        .sort(&[SortCriterion::Subject], SortCharset::UsAscii, "ALL")
        .unwrap();
    assert_eq!(inbox.len(), 2);
    let mut sort = inbox.iter();
    assert_eq!(sort.next().unwrap(), &1);
    assert_eq!(sort.next().unwrap(), &2);

    // e-mails should be reverse sorted by subject
    let inbox = c
        .sort(
            &[SortCriterion::Reverse(&SortCriterion::Subject)],
            SortCharset::Utf8,
            "ALL",
        )
        .unwrap();
    assert_eq!(inbox.len(), 2);
    let mut sort = inbox.iter();
    assert_eq!(sort.next().unwrap(), &2);
    assert_eq!(sort.next().unwrap(), &1);

    // the number of reverse does not change the order
    // one or more Reverse implies a reversed result
    let inbox = c
        .sort(
            &[SortCriterion::Reverse(&SortCriterion::Reverse(
                &SortCriterion::Reverse(&SortCriterion::Subject),
            ))],
            SortCharset::Custom("UTF-8".into()),
            "ALL",
        )
        .unwrap();
    assert_eq!(inbox.len(), 2);
    let mut sort = inbox.iter();
    assert_eq!(sort.next().unwrap(), &2);
    assert_eq!(sort.next().unwrap(), &1);

    // let's delete them to clean up
    c.store("1,2", "+FLAGS (\\Deleted)").unwrap();
    c.expunge().unwrap();

    // e-mails should be gone now
    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 0);

    let inbox = c
        .sort(&[SortCriterion::Subject], SortCharset::Utf8, "ALL")
        .unwrap();
    assert_eq!(inbox.len(), 0);
}

#[test]
fn inbox_uid() {
    let to = "inbox-uid@localhost";

    // first log in so we'll see the unsolicited e-mails
    let mut c = session(to);
    c.select("INBOX").unwrap();

    // then send the e-mail
    let s = smtp(to);
    let e = lettre::message::Message::builder()
        .from("sender@localhost".parse().unwrap())
        .to(to.parse().unwrap())
        .subject("My first e-mail")
        .body("Hello world from SMTP".to_string())
        .unwrap();
    s.send(&e.into()).unwrap();

    wait_for_delivery();

    // now we should see the e-mail!
    let inbox = c
        .uid_sort(&[SortCriterion::Subject], SortCharset::Utf8, "ALL")
        .unwrap();
    // and the one message should have the first message sequence number
    assert_eq!(inbox.len(), 1);
    let uid = inbox.into_iter().next().unwrap();

    // we should also get two unsolicited responses: Exists and Recent
    c.noop().unwrap();
    let mut unsolicited = VecDeque::new();
    while let Some(m) = c.unsolicited_responses.pop_front() {
        unsolicited.push_back(m);
    }
    assert_eq!(unsolicited.len(), 2);
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Exists(1)));
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Recent(1)));

    // let's see that we can also fetch the e-mail
    let fetch = c
        .uid_fetch(format!("{}", uid), "(ENVELOPE INTERNALDATE FLAGS UID)")
        .unwrap();
    assert_eq!(fetch.len(), 1);
    let fetch = fetch.iter().next().unwrap();
    assert_eq!(fetch.uid, Some(uid));
    let e = fetch.envelope().unwrap();
    assert_eq!(e.subject, Some(b"My first e-mail"[..].into()));
    let date_opt = fetch.internal_date();
    assert!(date_opt.is_some());

    // and let's delete it to clean up
    c.uid_store(format!("{}", uid), "+FLAGS (\\Deleted)")
        .unwrap();
    c.expunge().unwrap();

    // the e-mail should be gone now
    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 0);
}

#[test]
#[ignore]
fn list() {
    let mut s = session("readonly-test@localhost");
    s.select("INBOX").unwrap();
    let subdirs = s.list(None, Some("%")).unwrap();
    assert_eq!(subdirs.len(), 1);

    // TODO: make a subdir
}

#[test]
#[cfg(feature = "test-full-imap")]
fn list_status() {
    let mut s = session("readonly-test@localhost");
    let extnames = s.list_status(None, Some("*"), "(HIGHESTMODSEQ)").unwrap();
    assert_eq!(extnames.len(), 1);
    let (name, status) = extnames.get(0).unwrap();
    assert_eq!(name.name(), "INBOX");
    assert!(status.is_some());
    assert!(status.as_ref().unwrap().highest_mod_seq.is_some());
}

#[test]
fn append() {
    let to = "inbox-append1@localhost";

    // make a message to append
    let e: lettre::Message = lettre::message::Message::builder()
        .from("sender@localhost".parse().unwrap())
        .to(to.parse().unwrap())
        .subject("My second e-mail")
        .body("Hello world".to_string())
        .unwrap()
        .into();

    // connect
    let mut c = session(to);
    let mbox = "INBOX";
    c.select(mbox).unwrap();
    //append
    c.append(mbox, &e.formatted()).finish().unwrap();

    // now we should see the e-mail!
    let inbox = c.uid_search("ALL").unwrap();
    // and the one message should have the first message sequence number
    assert_eq!(inbox.len(), 1);
    let uid = inbox.into_iter().next().unwrap();

    // fetch the e-mail
    let fetch = c.uid_fetch(format!("{}", uid), "(ENVELOPE UID)").unwrap();
    assert_eq!(fetch.len(), 1);
    let fetch = fetch.iter().next().unwrap();
    assert_eq!(fetch.uid, Some(uid));
    let e = fetch.envelope().unwrap();
    assert_eq!(e.subject, Some(b"My second e-mail"[..].into()));

    // and let's delete it to clean up
    c.uid_store(format!("{}", uid), "+FLAGS (\\Deleted)")
        .unwrap();
    c.expunge().unwrap();

    // the e-mail should be gone now
    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 0);
}

#[test]
fn append_with_flags() {
    use imap::types::Flag;

    let to = "inbox-append2@localhost";

    // make a message to append
    let e: lettre::Message = lettre::message::Message::builder()
        .from("sender@localhost".parse().unwrap())
        .to(to.parse().unwrap())
        .subject("My third e-mail")
        .body("Hello world".to_string())
        .unwrap()
        .into();

    // connect
    let mut c = session(to);
    let mbox = "INBOX";
    c.select(mbox).unwrap();
    //append
    let flags = vec![Flag::Seen, Flag::Flagged];
    c.append(mbox, &e.formatted())
        .flags(flags)
        .finish()
        .unwrap();

    // now we should see the e-mail!
    let inbox = c.uid_search("ALL").unwrap();
    // and the one message should have the first message sequence number
    assert_eq!(inbox.len(), 1);
    let uid = inbox.into_iter().next().unwrap();

    // fetch the e-mail
    let fetch = c
        .uid_fetch(format!("{}", uid), "(ENVELOPE FLAGS UID)")
        .unwrap();
    assert_eq!(fetch.len(), 1);
    let fetch = fetch.iter().next().unwrap();
    assert_eq!(fetch.uid, Some(uid));
    let e = fetch.envelope().unwrap();
    assert_eq!(e.subject, Some(b"My third e-mail"[..].into()));

    // check the flags
    let setflags = fetch.flags();
    assert!(setflags.contains(&Flag::Seen));
    assert!(setflags.contains(&Flag::Flagged));

    // and let's delete it to clean up
    c.uid_store(format!("{}", uid), "+FLAGS (\\Deleted)")
        .unwrap();
    c.expunge().unwrap();

    // the e-mail should be gone now
    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 0);
}

#[test]
fn append_with_flags_and_date() {
    use imap::types::Flag;

    let to = "inbox-append3@localhost";

    // make a message to append
    let e: lettre::Message = lettre::message::Message::builder()
        .from("sender@localhost".parse().unwrap())
        .to(to.parse().unwrap())
        .subject("My third e-mail")
        .body("Hello world".to_string())
        .unwrap()
        .into();

    // connect
    let mut c = session(to);
    let mbox = "INBOX";
    c.select(mbox).unwrap();
    // append
    let date = FixedOffset::east(8 * 3600)
        .ymd(2020, 12, 13)
        .and_hms(13, 36, 36);
    c.append(mbox, &e.formatted())
        .flag(Flag::Seen)
        .flag(Flag::Flagged)
        .internal_date(date)
        .finish()
        .unwrap();

    // now we should see the e-mail!
    let inbox = c.uid_search("ALL").unwrap();
    // and the one message should have the first message sequence number
    assert_eq!(inbox.len(), 1);
    let uid = inbox.into_iter().next().unwrap();

    // fetch the e-mail
    let fetch = c
        .uid_fetch(format!("{}", uid), "(INTERNALDATE UID)")
        .unwrap();
    assert_eq!(fetch.len(), 1);
    let fetch = fetch.iter().next().unwrap();
    assert_eq!(fetch.uid, Some(uid));
    assert_eq!(fetch.internal_date(), Some(date));

    // and let's delete it to clean up
    c.uid_store(format!("{}", uid), "+FLAGS (\\Deleted)")
        .unwrap();
    c.expunge().unwrap();

    // the e-mail should be gone now
    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 0);
}

#[test]
#[cfg(feature = "test-full-imap")]
fn acl_tests() {
    use imap::types::AclModifyMode;

    let user_friend = "inbox-acl-friend@localhost";
    let user_me = "inbox-acl@localhost";

    // ensure we have this user by logging in once
    session(user_friend);

    let mut s_me = session(user_me);
    let acl = s_me.get_acl("INBOX").unwrap();
    // one ACL
    // assert_eq!(acl.acls().len(), 1);
    // ACL is for me
    assert_eq!(acl.parsed().acls()[0].identifier, user_me);
    // ACL has administration rights
    assert!(acl.parsed().acls()[0].rights.contains('a'));
    // Grant read to friend
    let ret = s_me.set_acl(
        "INBOX",
        user_friend,
        &"lr".try_into().unwrap(),
        AclModifyMode::Replace,
    );
    assert!(ret.is_ok());
    // Check rights again
    let acl = s_me.get_acl("INBOX").unwrap();
    assert_eq!(acl.parsed().acls().len(), 2);
    let idx = acl
        .parsed()
        .acls()
        .binary_search_by(|e| (*e.identifier).cmp(user_friend))
        .unwrap();
    assert_eq!(acl.parsed().acls()[idx].rights, "lr".try_into().unwrap());

    // Add "p" right (post)
    let ret = s_me.set_acl(
        "INBOX",
        user_friend,
        &"p".try_into().unwrap(),
        AclModifyMode::Add,
    );
    assert!(ret.is_ok());
    // Check rights again
    let acl = s_me.get_acl("INBOX").unwrap();
    assert_eq!(acl.parsed().acls().len(), 2);
    let idx = acl
        .parsed()
        .acls()
        .binary_search_by(|e| (*e.identifier).cmp(user_friend))
        .unwrap();
    assert_eq!(acl.parsed().acls()[idx].rights, "lrp".try_into().unwrap());
    // remove "p" right (post)
    let ret = s_me.set_acl(
        "INBOX",
        user_friend,
        &"p".try_into().unwrap(),
        AclModifyMode::Remove,
    );
    assert!(ret.is_ok());
    // Check rights again
    let acl = s_me.get_acl("INBOX").unwrap();
    assert_eq!(acl.parsed().acls().len(), 2);
    let idx = acl
        .parsed()
        .acls()
        .binary_search_by(|e| (*e.identifier).cmp(user_friend))
        .unwrap();
    assert_eq!(acl.parsed().acls()[idx].rights, "lr".try_into().unwrap());
    // Delete rights for friend
    let ret = s_me.delete_acl("INBOX", user_friend);
    assert!(ret.is_ok());
    // Check rights again
    let acl = s_me.get_acl("INBOX").unwrap();
    assert_eq!(acl.parsed().acls().len(), 1);
    assert_eq!(acl.parsed().acls()[0].identifier, user_me);
    // List rights
    let acl = s_me.list_rights("INBOX", user_friend).unwrap();
    assert_eq!(acl.parsed().mailbox(), "INBOX");
    assert_eq!(acl.parsed().identifier(), user_friend);
    assert!(acl.parsed().optional().contains('0'));
    assert!(!acl.parsed().required().contains('0'));

    // My Rights
    let acl = s_me.my_rights("INBOX").unwrap();
    assert_eq!(acl.parsed().mailbox(), "INBOX");
    assert!(acl.parsed().rights().contains('a'));
}

#[test]
fn status() {
    let mut s = session("readonly-test@localhost");

    // Test all valid fields except HIGHESTMODSEQ, which apparently
    // isn't supported by the IMAP server used for this test.
    let mb = s
        .status("INBOX", "(MESSAGES RECENT UIDNEXT UIDVALIDITY UNSEEN)")
        .unwrap();
    assert_eq!(mb.flags, Vec::new());
    assert_eq!(mb.exists, 0);
    assert_eq!(mb.recent, 0);
    assert!(mb.unseen.is_some());
    assert_eq!(mb.permanent_flags, Vec::new());
    assert!(mb.uid_next.is_some());
    assert!(mb.uid_validity.is_some());
    assert_eq!(mb.highest_mod_seq, None);
    assert_eq!(mb.is_read_only, false);

    // If we only request one field, we should only get one field
    // back. (A server could legally send an unsolicited STATUS
    // response, but this one won't.)
    let mb = s.status("INBOX", "(MESSAGES)").unwrap();
    let mut expected = Mailbox::default();
    expected.exists = 0;
    assert_eq!(mb, expected);
}

#[test]
#[cfg(feature = "test-full-imap")]
fn qresync() {
    // Ignored because Greenmail does not support QRESYNC. Does work with Cyrus, though.
    let to = "inbox-qresync@localhost";

    // make a message to append
    let e: lettre::Message = lettre::message::Message::builder()
        .from("sender@localhost".parse().unwrap())
        .to(to.parse().unwrap())
        .subject("QRESYNC test mail")
        .body("Hello world".to_string())
        .unwrap()
        .into();

    let mut s = session(to);

    // Enable extension
    s.run_command_and_read_response("ENABLE QRESYNC").unwrap();

    // Add an email and get its UID
    let mbox = "INBOX";
    s.select(mbox).unwrap();
    s.append(mbox, &e.formatted()).finish().unwrap();
    let inbox = s.uid_search("ALL").unwrap();
    assert_eq!(inbox.len(), 1);
    let uid = inbox.into_iter().next().unwrap();

    // Mark it as deleted
    let fetches = s
        .uid_store(format!("{}", uid), "+FLAGS.SILENT (\\Deleted)")
        .unwrap();
    // Assert that MODSEQ attribute is returned
    for f in fetches.iter() {
        assert_ne!(f.mod_seq(), None);
    }
    // Expunge it
    let r = s.uid_expunge(format!("{}", uid)).unwrap();
    // Assert that the new highest mod sequence is being returned
    assert_ne!(r.mod_seq, None);
}

fn assert_quota_resource(
    resource: &imap::types::QuotaResource,
    name: imap::types::QuotaResourceName,
    limit: u64,
    usage: Option<u64>,
) {
    assert_eq!(resource.name, name);
    if let Some(usage) = usage {
        assert_eq!(resource.usage, usage);
    }
    assert_eq!(resource.limit, limit);
}

#[test]
fn quota() {
    use imap::types::{QuotaResourceLimit, QuotaResourceName};

    let greeting = get_greeting();
    let is_greenmail = greeting.find("Cyrus").is_none();

    let to = "inbox-quota@localhost";

    if is_greenmail {
        let mut c = session(to);

        // Set a quota
        c.set_quota("INBOX", &[QuotaResourceLimit::new("STORAGE", 1000)])
            .unwrap();

        // Check it
        let quota_root = c.get_quota_root("INBOX").unwrap();

        assert_eq!(quota_root.mailbox_name(), "INBOX");

        let root_names = quota_root.quota_root_names().collect::<Vec<&str>>();
        // not sure why, but greenmail returns no quota root names
        assert_eq!(root_names, Vec::<&str>::new());

        assert_eq!(quota_root.quotas().len(), 1);

        let quota = quota_root.quotas().first().unwrap();
        assert_eq!(quota.root_name, "INBOX");
        assert_quota_resource(
            &quota.resources[0],
            QuotaResourceName::Storage,
            1000,
            Some(0),
        );

        // TODO no reliable way to delete a quota from greenmail other than resetting the whole system
        // Deleting a mailbox or user in greenmail does not remove the quota
    } else {
        // because we are cyrus we can "test" the admin account for checking the GET/SET commands
        // the clean: false is because the cyrus admin user has no INBOX.
        let mut admin = session_with_options("cyrus", false);

        // purge mailbox from previous run
        delete_mailbox(&mut admin, "user/inbox-quota@localhost");

        let mut c = session(to);

        let quota_root = c.get_quota_root("INBOX").unwrap();

        assert_eq!(quota_root.mailbox_name(), "INBOX");

        let root_names = quota_root.quota_root_names().collect::<Vec<&str>>();

        assert_eq!(root_names, vec!["INBOX"]);

        let quota = quota_root.quotas().first().unwrap();
        assert_eq!(quota.root_name, "INBOX");
        assert_quota_resource(
            &quota.resources[0],
            QuotaResourceName::Storage,
            1000,
            Some(0),
        );

        let update = admin
            .set_quota(
                "user/inbox-quota@localhost",
                &[QuotaResourceLimit::new(QuotaResourceName::Storage, 500)],
            )
            .unwrap();
        // Cyrus does not return the quota root definition on modification
        assert_eq!(update.parsed(), &None);

        let quota_response = admin.get_quota("user/inbox-quota@localhost").unwrap();
        let quota = quota_response.parsed().as_ref().unwrap();
        assert_eq!(quota.root_name, "user/inbox-quota@localhost");
        assert_eq!(quota.resources.len(), 1);
        assert_quota_resource(
            &quota.resources[0],
            QuotaResourceName::Storage,
            500,
            Some(0),
        );
    }
}
