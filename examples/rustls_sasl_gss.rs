extern crate imap;

use std::{env, error::Error, io};
use std::cmp::Ordering;
use rsasl::callback::{Context, Request, SessionData};
use rsasl::config::SASLConfig;
use rsasl::prelude::{Mechanism, Mechname, SessionError};
use rsasl::property::{AuthId, AuthzId, Hostname, Password};

fn main() -> Result<(), Box<dyn Error>> {
    // Read config from environment or .env file
    let host = env::var("HOST").expect("missing envvar host");
    let user = env::var("MAILUSER").ok();
    let password = env::var("PASSWORD").ok();
    let port = 993;

    if let Some(email) = fetch_inbox_top(host, user, password, port)? {
        println!("{}", &email);
    }

    Ok(())
}

struct MyCb {
    authid: Option<String>,
    authzid: Option<String>,
    passwd: Option<String>,
    host: String,
}
impl rsasl::callback::SessionCallback for MyCb {
    fn callback(&self, _session_data: &SessionData, _context: &Context, request: &mut Request) -> Result<(), SessionError> {
        request
            .satisfy::<Hostname>(&self.host)?
            .satisfy::<rsasl::mechanisms::gssapi::properties::GssService>("imap")?;

        if let Some(authid) = self.authid.as_deref() { request.satisfy::<AuthId>(authid)?; }
        if let Some(authzid) = self.authzid.as_deref() { request.satisfy::<AuthzId>(authzid)?; }
        if let Some(passwd) = self.passwd.as_deref() { request.satisfy::<Password>(passwd.as_bytes())?; }
        if let Some(authid) = self.authid.as_ref() { request.satisfy::<AuthId>(authid)?; }
        Ok(())
    }
    fn prefer<'a>(&self, a: Option<&'a Mechanism>, b: &'a Mechanism) -> Ordering {
        if let Some(a) = a {
            let mut buffer = String::new();
            let stdin = io::stdin();
            for _ in 0..3 {
                println!("Mechanism preference option: Enter '<' to prefer {a}, or '>' to prefer {b}.");
                let _ = stdin.read_line(&mut buffer);
                match buffer.trim() {
                    "<" => { return Ordering::Greater; }
                    ">" => { return Ordering::Less; }
                    x => { println!("Invalid input '{x}', enter one of '<' or '>'") },
                }
                buffer.clear();
            }
        }
        Ordering::Less
    }
}

fn fetch_inbox_top(
    host: String,
    user: Option<String>,
    password: Option<String>,
    port: u16,
) -> Result<Option<String>, Box<dyn Error>> {
    let client = imap::ClientBuilder::new(&host, port).rustls()?;

    let cb = MyCb {
        authid: user,
        authzid: None,
        passwd: password,
        host,
    };
    let saslconfig = SASLConfig::builder().with_defaults().with_callback(cb)?;

    println!("SASL configuration options — enable features like 'rsasl/sha2' to add available mechanisms:");
    println!("{saslconfig:#?}\n");

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client.sasl_auth(saslconfig).map_err(|e| e.0)?;

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
