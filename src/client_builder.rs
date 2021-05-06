use crate::{Client, Result};
use std::io::{Read, Write};
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
        self.connect(|domain, tcp| {
            let ssl_conn = TlsConnector::builder().build()?;
            Ok(TlsConnector::connect(&ssl_conn, domain, tcp)?)
        })
    }

    /// Return a new [`Client`] using `rustls` transport.
    #[cfg(feature = "rustls-tls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-tls")))]
    pub fn rustls(&mut self) -> Result<Client<RustlsStream<TcpStream>>> {
        self.connect(|domain, tcp| {
            let ssl_conn = RustlsConnector::new_with_native_certs()?;
            Ok(ssl_conn.connect(domain, tcp)?)
        })
    }

    /// Make a [`Client`] using a custom TLS initialization. This function is intended
    /// to be used if your TLS setup requires custom work such as adding private CAs
    /// or other specific TLS parameters.
    ///
    /// The `handshake` argument should accept two parameters:
    ///
    /// - domain: [`&str`]
    /// - tcp: [`TcpStream`]
    ///
    /// and yield a `Result<C>` where `C` is `Read + Write`. It should only perform
    /// TLS initialization over the given `tcp` socket and return the encrypted stream
    /// object, such as a [`native_tls::TlsStream`] or a [`rustls_connector::TlsStream`].
    ///
    /// If the caller is using `STARTTLS` and previously called [`starttls`](Self::starttls)
    /// then the `tcp` socket given to the `handshake` function will be connected and will
    /// have initiated the `STARTTLS` handshake.
    ///
    /// ```no_run
    /// # use imap::ClientBuilder;
    /// # use rustls_connector::RustlsConnector;
    /// # fn main() -> Result<(), imap::Error> {
    /// let client = ClientBuilder::new("imap.example.com", 993)
    ///     .starttls()
    ///     .connect(|domain, tcp| {
    ///         let ssl_conn = RustlsConnector::new_with_native_certs()?;
    ///         Ok(ssl_conn.connect(domain, tcp)?)
    ///     })?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn connect<F, C>(&mut self, handshake: F) -> Result<Client<C>>
    where
        F: FnOnce(&str, TcpStream) -> Result<C>,
        C: Read + Write,
    {
        let tcp = if self.starttls {
            let tcp = TcpStream::connect((self.domain.as_ref(), self.port))?;
            let mut client = Client::new(tcp);
            client.read_greeting()?;
            client.run_command_and_check_ok("STARTTLS")?;
            client.into_inner()?
        } else {
            TcpStream::connect((self.domain.as_ref(), self.port))?
        };

        let tls = handshake(self.domain.as_ref(), tcp)?;
        Ok(Client::new(tls))
    }
}
