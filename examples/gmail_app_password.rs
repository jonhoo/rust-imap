/// Example of using gmail authenticating using
/// an [App Password](https://support.google.com/accounts/answer/185833)
extern crate imap;
extern crate native_tls;

use native_tls::TlsConnector;

fn main() {
    let domain = "imap.gmail.com";
    let port = 993;
    let socket_addr = (domain, port);
    let ssl_connector = TlsConnector::builder().build().unwrap();
    let client = imap::connect(socket_addr, domain, &ssl_connector).unwrap();

    let mut imap_session = match client.login("somebody@gmail.com", "<app_password>") {
        Ok(c) => c,
        Err((e, _unauth_client)) => {
            println!("error authenticating: {}", e);
            return;
        }
    };

    match imap_session.select("INBOX") {
        Ok(mailbox) => println!("{}", mailbox),
        Err(e) => println!("Error selecting INBOX: {}", e),
    };

    match imap_session.fetch("2", "body[text]") {
        Ok(msgs) => {
            for msg in &msgs {
                print!("{:?}", msg);
            }
        }
        Err(e) => println!("Error Fetching email 2: {}", e),
    };

    imap_session.logout().unwrap();
}
