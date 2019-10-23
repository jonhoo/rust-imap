extern crate imap;
extern crate native_tls;
use std::env;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("need three arguments: imap-server login password");
    } else {
        fetch_messages_and_idle(&args[1], &args[2], &args[3]).unwrap();
    }
}

fn fetch_messages_and_idle(server: &str, login: &str, password: &str) -> imap::error::Result<()> {
    let tls = native_tls::TlsConnector::builder().build().unwrap();

    // we pass in the domain twice to check that the server's TLS
    // certificate is valid for the domain we're connecting to.
    let mut client = imap::connect((server, 993), server, &tls).unwrap();
    client.debug = true;

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client.login(login, password).map_err(|e| e.0)?;

    // we want to fetch the first email in the INBOX mailbox
    imap_session.select("INBOX")?;

    // fetch message number 1 in this mailbox, along with its RFC822 field.
    // RFC 822 dictates the format of the body of e-mails
    let messages = imap_session.fetch("1", "RFC822")?;
    println!("got {} messages", messages.len());
    {
        match imap_session.idle() {
            Ok(mut idle) => {
                &idle.set_keepalive(Duration::from_secs(20));
                println!("entering idle wait_keepalive");
                let res = &idle.wait_keepalive();
                println!("wait_keepalive returned {}", res.is_ok());
            }
            Err(err) => {
                return Err(imap::error::Error::Bad(err.to_string()));
            }
        };
    }

    // be nice to the server and log out
    imap_session.logout()?;
    Ok(())
}
