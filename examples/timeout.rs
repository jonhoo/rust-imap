extern crate imap;
extern crate native_tls;

use imap::Client;
use native_tls::TlsConnector;
use native_tls::TlsStream;
use std::env;
use std::error::Error;
use std::fmt;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    let server = env::var("IMAP_SERVER")?;
    let port = env::var("IMAP_PORT").unwrap_or_else(|_| String::from("993"));
    let port = port.parse()?;

    let username = env::var("IMAP_USER")?;
    let password = env::var("IMAP_PASSWORD")?;

    let timeout = env::var("IMAP_TIMEOUT").unwrap_or_else(|_| String::from("1"));
    let timeout = timeout.parse()?;
    let timeout = Duration::from_secs(timeout);

    let tls = TlsConnector::builder().build()?;

    let client = connect_all_timeout((server.as_str(), port), server.as_str(), &tls, timeout)?;

    let mut session = client.login(&username, &password).map_err(|e| e.0)?;

    // do something productive with session

    session.logout()?;

    Ok(())
}

// connect to an IMAP host with a `Duration` timeout; note that this accepts only a single
// `SocketAddr` while `connect_all_timeout` does resolve the DNS entry and try to connect to all;
// this is necessary due to the difference of the function signatures of `TcpStream::connect` and
// `TcpStream::connect_timeout`
fn connect_timeout<S: AsRef<str>>(
    addr: &SocketAddr,
    domain: S,
    ssl_connector: &TlsConnector,
    timeout: Duration,
) -> Result<Client<TlsStream<TcpStream>>, Box<dyn Error>> {
    // the timeout is actually used with the initial TcpStream
    let tcp_stream = TcpStream::connect_timeout(addr, timeout)?;

    let tls_stream = TlsConnector::connect(ssl_connector, domain.as_ref(), tcp_stream)?;

    let mut client = Client::new(tls_stream);

    // don't forget to wait for the IMAP protocol server greeting ;)
    client.read_greeting()?;

    Ok(client)
}

// resolve address and try to connect to all in order
fn connect_all_timeout<A: ToSocketAddrs, S: AsRef<str>>(
    addr: A,
    domain: S,
    ssl_connector: &TlsConnector,
    timeout: Duration,
) -> Result<Client<TlsStream<TcpStream>>, Box<dyn Error>> {
    let addrs = addr.to_socket_addrs()?;

    for addr in addrs {
        match connect_timeout(&addr, &domain, ssl_connector, timeout) {
            Ok(client) => return Ok(client),
            Err(error) => eprintln!("couldn't connect to {}: {}", addr, error),
        }
    }

    Err(Box::new(TimeoutError))
}

// very simple timeout error; instead of printing the errors immediately like in
// `connect_all_timeout`, you may want to collect and return them
#[derive(Debug)]
struct TimeoutError;

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "all addresses failed to connect")
    }
}

impl Error for TimeoutError {}
