extern crate imap;
extern crate lettre;
extern crate lettre_email;
extern crate native_tls;

use lettre::Transport;
use std::net::TcpStream;

fn tls() -> native_tls::TlsConnector {
    native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

fn session(user: &str) -> imap::Session<native_tls::TlsStream<TcpStream>> {
    let mut s = imap::connect("127.0.0.1:3993", "imap.example.com", &tls())
        .unwrap()
        .login(user, user)
        .unwrap();
    s.debug = true;
    s
}

fn smtp(user: &str) -> lettre::SmtpTransport {
    let creds = lettre::smtp::authentication::Credentials::new(user.to_string(), user.to_string());
    lettre::SmtpClient::new(
        "127.0.0.1:3465",
        lettre::ClientSecurity::Wrapper(lettre::ClientTlsParameters {
            connector: tls(),
            domain: "smpt.example.com".to_string(),
        }),
    )
    .unwrap()
    .credentials(creds)
    .transport()
}

#[test]
fn connect_insecure() {
    imap::connect_insecure("127.0.0.1:3143").unwrap();
}

#[test]
#[ignore]
fn connect_insecure_then_secure() {
    // ignored because of https://github.com/greenmail-mail-test/greenmail/issues/135
    imap::connect_insecure("127.0.0.1:3143")
        .unwrap()
        .secure("imap.example.com", &tls())
        .unwrap();
}

#[test]
fn connect_secure() {
    imap::connect("127.0.0.1:3993", "imap.example.com", &tls()).unwrap();
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
    let mut s = smtp(to);
    let e = lettre_email::Email::builder()
        .from("sender@localhost")
        .to(to)
        .subject("My first e-mail")
        .text("Hello world from SMTP")
        .build()
        .unwrap();
    s.send(e.into()).unwrap();

    // now we should see the e-mail!
    let inbox = c.search("ALL").unwrap();
    // and the one message should have the first message sequence number
    assert_eq!(inbox.len(), 1);
    assert!(inbox.contains(&1));

    // we should also get two unsolicited responses: Exists and Recent
    c.noop().unwrap();
    let mut unsolicited = Vec::new();
    while let Ok(m) = c.unsolicited_responses.try_recv() {
        unsolicited.push(m);
    }
    assert_eq!(unsolicited.len(), 2);
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Exists(1)));
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Recent(1)));

    // let's see that we can also fetch the e-mail
    let fetch = c.fetch("1", "(ALL UID)").unwrap();
    assert_eq!(fetch.len(), 1);
    let fetch = &fetch[0];
    assert_eq!(fetch.message, 1);
    assert_ne!(fetch.uid, None);
    assert_eq!(fetch.size, Some(138));
    let e = fetch.envelope().unwrap();
    assert_eq!(e.subject, Some("My first e-mail"));
    assert_ne!(e.from, None);
    assert_eq!(e.from.as_ref().unwrap().len(), 1);
    let from = &e.from.as_ref().unwrap()[0];
    assert_eq!(from.mailbox, Some("sender"));
    assert_eq!(from.host, Some("localhost"));
    assert_ne!(e.to, None);
    assert_eq!(e.to.as_ref().unwrap().len(), 1);
    let to = &e.to.as_ref().unwrap()[0];
    assert_eq!(to.mailbox, Some("inbox"));
    assert_eq!(to.host, Some("localhost"));
    let date_opt = fetch.internal_date();
    assert!(date_opt.is_some());

    // and let's delete it to clean up
    c.store("1", "+FLAGS (\\Deleted)").unwrap();
    c.expunge().unwrap();

    // the e-mail should be gone now
    let inbox = c.search("ALL").unwrap();
    assert_eq!(inbox.len(), 0);
}

#[test]
fn inbox_uid() {
    let to = "inbox-uid@localhost";

    // first log in so we'll see the unsolicited e-mails
    let mut c = session(to);
    c.select("INBOX").unwrap();

    // then send the e-mail
    let mut s = smtp(to);
    let e = lettre_email::Email::builder()
        .from("sender@localhost")
        .to(to)
        .subject("My first e-mail")
        .text("Hello world from SMTP")
        .build()
        .unwrap();
    s.send(e.into()).unwrap();

    // now we should see the e-mail!
    let inbox = c.uid_search("ALL").unwrap();
    // and the one message should have the first message sequence number
    assert_eq!(inbox.len(), 1);
    let uid = inbox.into_iter().next().unwrap();

    // we should also get two unsolicited responses: Exists and Recent
    c.noop().unwrap();
    let mut unsolicited = Vec::new();
    while let Ok(m) = c.unsolicited_responses.try_recv() {
        unsolicited.push(m);
    }
    assert_eq!(unsolicited.len(), 2);
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Exists(1)));
    assert!(unsolicited
        .iter()
        .any(|m| m == &imap::types::UnsolicitedResponse::Recent(1)));

    // let's see that we can also fetch the e-mail
    let fetch = c.uid_fetch(format!("{}", uid), "(ALL UID)").unwrap();
    assert_eq!(fetch.len(), 1);
    let fetch = &fetch[0];
    assert_eq!(fetch.uid, Some(uid));
    let e = fetch.envelope().unwrap();
    assert_eq!(e.subject, Some("My first e-mail"));
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
    assert_eq!(subdirs.len(), 0);

    // TODO: make a subdir
}
