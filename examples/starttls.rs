/**
 * Here's an example showing how to connect to the IMAP server with STARTTLS.
 *
 * The only difference is calling `starttls()` on the `ClientBuilder` before
 * initiating the secure connection with `connect()`, so you
 * can connect on port 143 instead of 993.
 *
 * The following env vars are expected to be set:
 * - IMAP_HOST
 * - IMAP_USERNAME
 * - IMAP_PASSWORD
 * - IMAP_PORT (supposed to be 143)
 */
extern crate imap;

use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let imap_host = env::var("IMAP_HOST").expect("Missing or invalid env var: IMAP_HOST");
    let imap_username =
        env::var("IMAP_USERNAME").expect("Missing or invalid env var: IMAP_USERNAME");
    let imap_password =
        env::var("IMAP_PASSWORD").expect("Missing or invalid env var: IMAP_PASSWORD");
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
    let client = imap::ClientBuilder::new(&host, port)
        .connect()
        .expect("Could not connect to server");

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut _imap_session = client
        .login(username.as_str(), password.as_str())
        .map_err(|e| e.0)?;

    // TODO Here you can process as you want. eg. search/fetch messages according to your needs.

    // This returns `Ok(None)` for the need of the example
    Ok(None)
}
