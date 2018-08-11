extern crate base64;
extern crate imap;
extern crate native_tls;

use base64::encode;
use imap::authenticator::Authenticator;
use imap::client::UnauthenticatedClient;
use native_tls::TlsConnector;

struct GmailOAuth2 {
    user: String,
    access_token: String,
}

impl Authenticator for GmailOAuth2 {
    #[allow(unused_variables)]
    fn process(&self, data: String) -> String {
        encode(
            format!(
                "user={}\x01auth=Bearer {}\x01\x01",
                self.user, self.access_token
            ).as_bytes(),
        )
    }
}

fn main() {
    let gmail_auth = GmailOAuth2 {
        user: String::from("sombody@gmail.com"),
        access_token: String::from("<access_token>"),
    };
    let domain = "imap.gmail.com";
    let port = 993;
    let socket_addr = (domain, port);
    let ssl_connector = TlsConnector::builder().build().unwrap();
    let unauth_client = UnauthenticatedClient::secure_connect(socket_addr, domain, &ssl_connector).unwrap();

    let mut imap_socket = match unauth_client.authenticate("XOAUTH2", gmail_auth) {
        Ok(c) => c,
        Err((e, _unauth_client)) => {
            println!("error authenticating: {}", e);
            return;
        }
    };

    match imap_socket.select("INBOX") {
        Ok(mailbox) => println!("{}", mailbox),
        Err(e) => println!("Error selecting INBOX: {}", e),
    };

    match imap_socket.fetch("2", "body[text]") {
        Ok(msgs) => for msg in &msgs {
            print!("{:?}", msg);
        },
        Err(e) => println!("Error Fetching email 2: {}", e),
    };

    imap_socket.logout().unwrap();
}
