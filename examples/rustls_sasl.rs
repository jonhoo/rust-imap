extern crate imap;

use std::{env, error::Error};
use rsasl::config::SASLConfig;
use rsasl::prelude::Mechname;

fn main() -> Result<(), Box<dyn Error>> {
    // Read config from environment or .env file
    let host = env::var("HOST").expect("missing envvar host");
    let user = env::var("MAILUSER").expect("missing envvar USER");
    let password = env::var("PASSWORD").expect("missing envvar password");
    let port = 993;

    if let Some(email) = fetch_inbox_top(host, user, password, port)? {
        println!("{}", &email);
    }

    Ok(())
}

fn fetch_inbox_top(
    host: String,
    user: String,
    password: String,
    port: u16,
) -> Result<Option<String>, Box<dyn Error>> {
    let client = imap::ClientBuilder::new(&host, port).rustls()?;

    let saslconfig = SASLConfig::with_credentials(None, user, password).unwrap();

    let mechanism = Mechname::parse(b"SCRAM-SHA-256").unwrap();

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client.authenticate(saslconfig, mechanism).map_err(|e| e.0)?;

    // we want to fetch the first email in the INBOX mailbox
    imap_session.select("INBOX")?;

    // fetch message number 1 in this mailbox, along with its RFC822 field.
    // RFC 822 dictates the format of the body of e-mails
    let messages = imap_session.fetch("1", "RFC822")?;
    let message = if let Some(m) = messages.iter().next() {
        m
    } else {
        return Ok(None);
    };

    // extract the message's body
    let body = message.body().expect("message did not have a body!");
    let body = std::str::from_utf8(body)
        .expect("message was not valid utf-8")
        .to_string();

    // be nice to the server and log out
    imap_session.logout()?;

    Ok(Some(body))
}
