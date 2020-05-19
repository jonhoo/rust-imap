/**
 * Here's an example showing how to connect to the IMAP server with STARTTLS.
 * The only difference with the `basic.rs` example is when using `imap::connect_starttls()` method
 * instead of `imap::connect()` (l. 52)
 *
 * The following env vars are expected to be set:
 * - IMAP_HOST
 * - IMAP_USERNAME
 * - IMAP_PASSWORD
 * - IMAP_PORT (supposed to be 143)
 */

extern crate imap;
extern crate native_tls;

use native_tls::TlsConnector;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let imap_host = env::var("IMAP_HOST")
        .expect("Missing or invalid env var: IMAP_HOST");
    let imap_username = env::var("IMAP_USERNAME")
        .expect("Missing or invalid env var: IMAP_USERNAME");
    let imap_password = env::var("IMAP_PASSWORD")
        .expect("Missing or invalid env var: IMAP_PASSWORD");
    let imap_port: u16 = env::var("IMAP_PORT")
        .expect("Missing or invalid env var: IMAP_PORT")
        .to_string()
        .parse()
        .unwrap();

    if let Some(_email) = fetch_inbox_top(imap_host, imap_username, imap_password, imap_port)? {
        eprintln!("OK :)");
    }

    Ok(())
}

fn fetch_inbox_top(
    host: String,
    username: String,
    password: String,
    port: u16,
) -> Result<Option<String>, Box<dyn Error>> {
    let domain: &str = host.as_str();

    let tls = TlsConnector::builder().build().unwrap();

    // we pass in the domain twice to check that the server's TLS
    // certificate is valid for the domain we're connecting to.
    let client = imap::connect_starttls(
        (domain, port),
        domain,
        &tls,
    ).unwrap();

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut _imap_session = client
        .login(username.as_str(), password.as_str())
        .map_err(|e| e.0)?;

    // TODO Here you can process as you want. eg. search/fetch messages according to your needs.

    // This returns `Ok(None)` for the need of the example
    Ok(None)
}
