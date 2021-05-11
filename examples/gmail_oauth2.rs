extern crate base64;
extern crate imap;

struct GmailOAuth2 {
    user: String,
    access_token: String,
}

impl imap::Authenticator for GmailOAuth2 {
    type Response = String;
    #[allow(unused_variables)]
    fn process(&self, data: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

fn main() {
    let gmail_auth = GmailOAuth2 {
        user: String::from("sombody@gmail.com"),
        access_token: String::from("<access_token>"),
    };

    let client = imap::ClientBuilder::new("imap.gmail.com", 993)
        .native_tls()
        .expect("Could not connect to imap.gmail.com");

    let mut imap_session = match client.authenticate("XOAUTH2", &gmail_auth) {
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
