use crate::{Client, Result};
use std::net::TcpStream;

#[cfg(feature = "tls")]
use native_tls::{TlsConnector, TlsStream};
#[cfg(feature = "rustls-tls")]
use rustls_connector::{RustlsConnector, TlsStream as RustlsStream};

/// A convenience builder for [`Client`] structs over various encrypted transports.
///
/// Creating a [`Client`] using `native-tls` transport is straightforward:
/// ```no_run
/// # use imap::ClientBuilder;
/// # fn main() -> Result<(), imap::Error> {
/// let client = ClientBuilder::new("imap.example.com", 993).native_tls()?;
/// # Ok(())
/// # }
/// ```
///
/// Similarly, if using the `rustls-tls` feature you can create a [`Client`] using rustls:
/// ```no_run
/// # use imap::ClientBuilder;
/// # fn main() -> Result<(), imap::Error> {
/// let client = ClientBuilder::new("imap.example.com", 993).rustls()?;
/// # Ok(())
/// # }
/// ```
///
/// To use `STARTTLS`, just call `starttls()` before one of the [`Client`]-yielding
/// functions:
/// ```no_run
/// # use imap::ClientBuilder;
/// # fn main() -> Result<(), imap::Error> {
/// let client = ClientBuilder::new("imap.example.com", 993)
///     .starttls()
///     .rustls()?;
/// # Ok(())
/// # }
/// ```
pub struct ClientBuilder<D>
where
    D: AsRef<str>,
{
    domain: D,
    port: u16,
    starttls: bool,
}

impl<D> ClientBuilder<D>
where
    D: AsRef<str>,
{
    /// Make a new `ClientBuilder` using the given domain and port.
    pub fn new(domain: D, port: u16) -> Self {
        ClientBuilder {
            domain,
            port,
            starttls: false,
        }
    }

    /// Use `STARTTLS` for this connection.
    #[cfg(any(feature = "tls", feature = "rustls-tls"))]
    pub fn starttls(&mut self) -> &mut Self {
        self.starttls = true;
        self
    }

    /// Return a new [`Client`] using a `native-tls` transport.
    #[cfg(feature = "tls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls")))]
    pub fn native_tls(&mut self) -> Result<Client<TlsStream<TcpStream>>> {
        let tcp = TcpStream::connect((self.domain.as_ref(), self.port))?;
        if self.starttls {
            let mut client = Client::new(tcp);
            client.read_greeting()?;
            client.run_command_and_check_ok("STARTTLS")?;
            let ssl_conn = TlsConnector::builder().build()?;
            let tls = TlsConnector::connect(&ssl_conn, self.domain.as_ref(), client.into_inner()?)?;
            Ok(Client::new(tls))
        } else {
            let ssl_conn = TlsConnector::builder().build()?;
            let tls = TlsConnector::connect(&ssl_conn, self.domain.as_ref(), tcp)?;
            Ok(Client::new(tls))
        }
    }

    /// Return a new [`Client`] using `rustls` transport.
    #[cfg(feature = "rustls-tls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-tls")))]
    pub fn rustls(&mut self) -> Result<Client<RustlsStream<TcpStream>>> {
        let tcp = TcpStream::connect((self.domain.as_ref(), self.port))?;
        if self.starttls {
            let mut client = Client::new(tcp);
            client.read_greeting()?;
            client.run_command_and_check_ok("STARTTLS")?;
            let ssl_conn = RustlsConnector::new_with_native_certs()?;
            let tls = ssl_conn.connect(self.domain.as_ref(), client.into_inner()?)?;
            Ok(Client::new(tls))
        } else {
            let ssl_conn = RustlsConnector::new_with_native_certs()?;
            let tls = ssl_conn.connect(self.domain.as_ref(), tcp)?;
            Ok(Client::new(tls))
        }
    }
}
