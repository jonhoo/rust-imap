extern crate imap;

use imap::ConnectionMode;

fn test_host() -> String {
    std::env::var("TEST_HOST").unwrap_or("127.0.0.1".to_string())
}

fn test_imap_port() -> u16 {
    std::env::var("TEST_IMAP_PORT")
        .unwrap_or("3143".to_string())
        .parse()
        .unwrap_or(3143)
}

#[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
fn test_imaps_port() -> u16 {
    std::env::var("TEST_IMAPS_PORT")
        .unwrap_or("3993".to_string())
        .parse()
        .unwrap_or(3993)
}

fn list_mailbox(session: &mut imap::Session<imap::Connection>) -> Result<(), imap::Error> {
    session.select("INBOX")?;
    session.search("ALL")?;
    Ok(())
}

#[cfg(all(
    any(feature = "native-tls", feature = "rustls-tls"),
    feature = "test-full-imap"
))]
#[test]
fn starttls_force() {
    let user = "starttls@localhost";
    let host = test_host();
    let c = imap::ClientBuilder::new(&host, test_imap_port())
        .danger_skip_tls_verify(true)
        .mode(ConnectionMode::StartTls)
        .connect()
        .unwrap();
    let mut s = c.login(user, user).unwrap();
    s.debug = true;
    assert!(list_mailbox(&mut s).is_ok());
}

#[cfg(all(
    any(feature = "native-tls", feature = "rustls-tls"),
    feature = "test-full-imap"
))]
#[test]
fn tls_force() {
    let user = "tls@localhost";
    let host = test_host();
    let c = imap::ClientBuilder::new(&host, test_imaps_port())
        .danger_skip_tls_verify(true)
        .mode(ConnectionMode::Tls)
        .connect()
        .unwrap();
    let mut s = c.login(user, user).unwrap();
    s.debug = true;
    assert!(list_mailbox(&mut s).is_ok());
}

#[cfg(feature = "rustls-tls")]
#[test]
fn tls_force_rustls() {
    let user = "tls@localhost";
    let host = test_host();
    let c = imap::ClientBuilder::new(&host, test_imaps_port())
        .danger_skip_tls_verify(true)
        .tls_kind(imap::TlsKind::Rust)
        .mode(ConnectionMode::Tls)
        .connect()
        .unwrap();
    let mut s = c.login(user, user).unwrap();
    s.debug = true;
    assert!(list_mailbox(&mut s).is_ok());
}

#[cfg(feature = "native-tls")]
#[test]
fn tls_force_native() {
    let user = "tls@localhost";
    let host = test_host();
    let c = imap::ClientBuilder::new(&host, test_imaps_port())
        .danger_skip_tls_verify(true)
        .tls_kind(imap::TlsKind::Native)
        .mode(ConnectionMode::Tls)
        .connect()
        .unwrap();
    let mut s = c.login(user, user).unwrap();
    s.debug = true;
    assert!(list_mailbox(&mut s).is_ok());
}

#[test]
#[cfg(all(
    feature = "test-full-imap",
    any(feature = "native-tls", feature = "rustls-tls")
))]
fn auto_tls() {
    let user = "auto@localhost";
    let host = test_host();
    let builder = imap::ClientBuilder::new(&host, test_imap_port()).danger_skip_tls_verify(true);

    let c = builder.connect().unwrap();
    let mut s = c.login(user, user).unwrap();
    s.debug = true;
    assert!(list_mailbox(&mut s).is_ok());
}

#[test]
fn auto() {
    let user = "auto@localhost";
    let host = test_host();
    let builder = imap::ClientBuilder::new(&host, test_imap_port()).mode(ConnectionMode::Auto);
    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    let builder = builder.danger_skip_tls_verify(true);

    let c = builder.connect().unwrap();
    let mut s = c.login(user, user).unwrap();
    s.debug = true;
    assert!(list_mailbox(&mut s).is_ok());
}

#[test]
fn raw_force() {
    let user = "raw@localhost";
    let host = test_host();
    let c = imap::ClientBuilder::new(&host, test_imap_port())
        .mode(ConnectionMode::Plaintext)
        .connect()
        .unwrap();
    let mut s = c.login(user, user).unwrap();
    s.debug = true;
    assert!(list_mailbox(&mut s).is_ok());
}
