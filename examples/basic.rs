extern crate imap;
extern crate native_tls;

use native_tls::TlsConnector;
use imap::client::Client;

// To connect to the gmail IMAP server with this you will need to allow unsecure apps access.
// See: https://support.google.com/accounts/answer/6010255?hl=en
// Look at the gmail_oauth2.rs example on how to connect to a gmail server securely.
fn main() {
    let domain = "imap.gmail.com";
    let port = 993;
    let socket_addr = (domain, port);
    let ssl_connector = TlsConnector::builder().unwrap().build().unwrap();
    let mut imap_socket = Client::secure_connect(socket_addr, domain, ssl_connector).unwrap();

    imap_socket.login("username", "password").unwrap();

    match imap_socket.capability() {
        Ok(capabilities) => for capability in capabilities.iter() {
            println!("{}", capability);
        },
        Err(e) => println!("Error parsing capability: {}", e),
    };

    match imap_socket.select("INBOX") {
        Ok(mailbox) => {
            println!("{}", mailbox);
        }
        Err(e) => println!("Error selecting INBOX: {}", e),
    };

    match imap_socket.fetch("2", "body[text]") {
        Ok(lines) => for line in lines.iter() {
            print!("{}", line);
        },
        Err(e) => println!("Error Fetching email 2: {}", e),
    };

    imap_socket.logout().unwrap();
}
