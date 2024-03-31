use crate::{Client, Connection, Error, Result};

use lazy_static::lazy_static;
use std::io::{Read, Write};
use std::net::TcpStream;

#[cfg(feature = "native-tls")]
use native_tls::TlsConnector as NativeTlsConnector;

use crate::extensions::idle::SetReadTimeout;
#[cfg(feature = "rustls-tls")]
use rustls_connector::{
    rustls,
    rustls::pki_types::{CertificateDer, ServerName},
    rustls::{ClientConfig, RootCertStore},
    rustls_native_certs::load_native_certs,
    RustlsConnector,
};
#[cfg(feature = "rustls-tls")]
use std::sync::Arc;

#[cfg(feature = "rustls-tls")]
#[derive(Debug)]
struct NoCertVerification(rustls::client::WebPkiServerVerifier);

#[cfg(feature = "rustls-tls")]
impl rustls::client::danger::ServerCertVerifier for NoCertVerification {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::prelude::v1::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
    {
        self.0.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

#[cfg(feature = "rustls-tls")]
lazy_static! {
    static ref CACERTS: RootCertStore = {
        let mut store = RootCertStore::empty();
        for cert in load_native_certs().unwrap_or_else(|_| vec![]) {
            if let Ok(_) = store.add(cert) {}
        }
        store
    };
}

lazy_static! {
    static ref STARTLS_CHECK_REGEX: regex::bytes::Regex =
        regex::bytes::Regex::new(r"\bSTARTTLS\b").unwrap();
}

/// The connection mode we are going to use
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConnectionMode {
    /// Automatically detect what connection mode should be used.
    ///
    /// This will use TLS if the port is 993, and otherwise STARTTLS if available.
    /// If no TLS communication mechanism is available, the connection will fail.
    AutoTls,
    /// Automatically detect what connection mode should be used.
    ///
    /// This will use TLS if the port is 993, and otherwise STARTTLS if available.
    /// It will fallback to a plaintext connection if no TLS option can be used.
    Auto,
    /// A plain unencrypted TCP connection
    Plaintext,
    /// An encrypted TLS connection
    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    Tls,
    /// An eventually-encrypted (i.e., STARTTLS) connection
    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    StartTls,
}

/// A selection for TLS implementation
#[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TlsKind {
    /// Use the NativeTLS backend
    #[cfg(feature = "native-tls")]
    Native,
    /// Use the Rustls backend
    #[cfg(feature = "rustls-tls")]
    Rust,
    /// Use whatever backend is available (uses rustls if both are available)
    Any,
}

/// A convenience builder for [`Client`] structs over various encrypted transports.
///
/// Creating a [`Client`] using TLS is straightforward.
///
/// This will make a TLS connection directly since the port is 993.
/// ```no_run
/// # use imap::ClientBuilder;
/// # {} #[cfg(feature = "native-tls")]
/// # fn main() -> Result<(), imap::Error> {
/// let client = ClientBuilder::new("imap.example.com", 993).connect()?;
/// # Ok(())
/// # }
/// ```
///
/// By default it will detect and use `STARTTLS` if available.
/// ```no_run
/// # use imap::ClientBuilder;
/// # {} #[cfg(feature = "native-tls")]
/// # fn main() -> Result<(), imap::Error> {
/// let client = ClientBuilder::new("imap.example.com", 143).connect()?;
/// # Ok(())
/// # }
/// ```
///
/// To force a certain implementation you can call tls_kind():
/// ```no_run
/// # use imap::ClientBuilder;
/// # {} #[cfg(feature = "rustls-tls")]
/// # fn main() -> Result<(), imap::Error> {
/// let client = ClientBuilder::new("imap.example.com", 993)
///     .tls_kind(imap::TlsKind::Rust).connect()?;
/// # Ok(())
/// # }
/// ```
///
/// To force the use `STARTTLS`, just call `mode()` before connect():
///
/// If the server does not provide STARTTLS this will error out.
/// ```no_run
/// # use imap::ClientBuilder;
/// # {} #[cfg(feature = "rustls-tls")]
/// # fn main() -> Result<(), imap::Error> {
/// use imap::ConnectionMode;
/// let client = ClientBuilder::new("imap.example.com", 993)
///     .mode(ConnectionMode::StartTls)
///     .connect()?;
/// # Ok(())
/// # }
/// ```
/// The returned [`Client`] is unauthenticated; to access session-related methods (through
/// [`Session`](crate::Session)), use [`Client::login`] or [`Client::authenticate`].
#[derive(Clone)]
pub struct ClientBuilder<D>
where
    D: AsRef<str>,
{
    domain: D,
    port: u16,
    mode: ConnectionMode,
    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    tls_kind: TlsKind,
    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    skip_tls_verify: bool,
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
            mode: ConnectionMode::AutoTls,
            #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
            tls_kind: TlsKind::Any,
            #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
            skip_tls_verify: false,
        }
    }

    /// Sets the Connection mode to use for this connection
    pub fn mode(mut self, mode: ConnectionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets the TLS backend to use for this connection.
    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    pub fn tls_kind(mut self, kind: TlsKind) -> Self {
        self.tls_kind = kind;
        self
    }

    /// Controls the use of certificate validation.
    ///
    /// Defaults to `false`.
    ///
    /// # Warning
    ///
    /// You should only use this as a last resort as it allows another server to impersonate the
    /// server you think you're talking to, which would include being able to receive your
    /// credentials.
    ///
    /// See [`native_tls::TlsConnectorBuilder::danger_accept_invalid_certs`],
    /// [`native_tls::TlsConnectorBuilder::danger_accept_invalid_hostnames`],
    /// [`rustls::ClientConfig::dangerous`]
    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    pub fn danger_skip_tls_verify(mut self, skip_tls_verify: bool) -> Self {
        self.skip_tls_verify = skip_tls_verify;
        self
    }

    /// Make a [`Client`] using the configuration.
    ///
    /// ```no_run
    /// # use imap::ClientBuilder;
    /// # {} #[cfg(feature = "rustls-tls")]
    /// # fn main() -> Result<(), imap::Error> {
    /// let client = ClientBuilder::new("imap.example.com", 143).connect()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn connect(&self) -> Result<Client<Connection>> {
        #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
        return self.connect_with(|_domain, tcp| self.build_tls_connection(tcp));
        #[cfg(all(not(feature = "native-tls"), not(feature = "rustls-tls")))]
        return self.connect_with(|_domain, _tcp| -> Result<Connection> {
            return Err(Error::TlsNotConfigured);
        });
    }

    #[allow(unused_variables)]
    fn connect_with<F, C>(&self, handshake: F) -> Result<Client<Connection>>
    where
        F: FnOnce(&str, TcpStream) -> Result<C>,
        C: Read + Write + Send + SetReadTimeout + 'static,
    {
        #[allow(unused_mut)]
        let mut greeting_read = false;
        let tcp = TcpStream::connect((self.domain.as_ref(), self.port))?;

        let stream: Connection = match self.mode {
            ConnectionMode::AutoTls => {
                #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
                if self.port == 993 {
                    Box::new(handshake(self.domain.as_ref(), tcp)?)
                } else {
                    let (stream, upgraded) = self.upgrade_tls(Client::new(tcp), handshake)?;
                    greeting_read = true;

                    if !upgraded {
                        Err(Error::StartTlsNotAvailable)?
                    }
                    stream
                }
                #[cfg(all(not(feature = "native-tls"), not(feature = "rustls-tls")))]
                Err(Error::TlsNotConfigured)?
            }
            ConnectionMode::Auto => {
                #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
                if self.port == 993 {
                    Box::new(handshake(self.domain.as_ref(), tcp)?)
                } else {
                    let (stream, _upgraded) = self.upgrade_tls(Client::new(tcp), handshake)?;
                    greeting_read = true;

                    stream
                }
                #[cfg(all(not(feature = "native-tls"), not(feature = "rustls-tls")))]
                Box::new(tcp)
            }
            ConnectionMode::Plaintext => Box::new(tcp),
            #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
            ConnectionMode::StartTls => {
                let (stream, upgraded) = self.upgrade_tls(Client::new(tcp), handshake)?;
                greeting_read = true;

                if !upgraded {
                    Err(Error::StartTlsNotAvailable)?
                }
                stream
            }
            #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
            ConnectionMode::Tls => Box::new(handshake(self.domain.as_ref(), tcp)?),
        };

        let mut client = Client::new(stream);
        if !greeting_read {
            client.read_greeting()?;
        } else {
            client.greeting_read = true;
        }

        Ok(client)
    }

    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    fn upgrade_tls<F, C>(
        &self,
        mut client: Client<TcpStream>,
        handshake: F,
    ) -> Result<(Connection, bool)>
    where
        F: FnOnce(&str, TcpStream) -> Result<C>,
        C: Read + Write + Send + SetReadTimeout + 'static,
    {
        client.read_greeting()?;

        let capabilities = client.capabilities()?;
        if capabilities.has(&imap_proto::Capability::Atom("STARTTLS".into())) {
            client.run_command_and_check_ok("STARTTLS")?;
            let tcp = client.into_inner()?;
            Ok((Box::new(handshake(self.domain.as_ref(), tcp)?), true))
        } else {
            Ok((Box::new(client.into_inner()?), false))
        }
    }

    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    fn build_tls_connection(&self, tcp: TcpStream) -> Result<Connection> {
        match self.tls_kind {
            #[cfg(feature = "native-tls")]
            TlsKind::Native => self.build_tls_native(tcp),
            #[cfg(feature = "rustls-tls")]
            TlsKind::Rust => self.build_tls_rustls(tcp),
            TlsKind::Any => self.build_tls_any(tcp),
        }
    }

    #[cfg(feature = "rustls-tls")]
    fn build_tls_any(&self, tcp: TcpStream) -> Result<Connection> {
        self.build_tls_rustls(tcp)
    }

    #[cfg(all(not(feature = "rustls-tls"), feature = "native-tls"))]
    fn build_tls_any(&self, tcp: TcpStream) -> Result<Connection> {
        self.build_tls_native(tcp)
    }

    #[cfg(feature = "rustls-tls")]
    fn build_tls_rustls(&self, tcp: TcpStream) -> Result<Connection> {
        let mut config = ClientConfig::builder()
            .with_root_certificates(CACERTS.clone())
            .with_no_client_auth();
        if self.skip_tls_verify {
            config
                .dangerous()
                .set_certificate_verifier(Arc::new(NoCertVerification(
                    Arc::into_inner(
                        rustls::client::WebPkiServerVerifier::builder(Arc::new(CACERTS.clone()))
                            .build()
                            .expect("can construct standard verifier"),
                    )
                    .expect("just constructed, so should only be one"),
                )));
        }
        let ssl_conn: RustlsConnector = config.into();
        Ok(Box::new(ssl_conn.connect(self.domain.as_ref(), tcp)?))
    }

    #[cfg(feature = "native-tls")]
    fn build_tls_native(&self, tcp: TcpStream) -> Result<Connection> {
        let mut builder = NativeTlsConnector::builder();
        if self.skip_tls_verify {
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
        }
        let ssl_conn = builder.build()?;
        Ok(Box::new(NativeTlsConnector::connect(
            &ssl_conn,
            self.domain.as_ref(),
            tcp,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod connection_mode {
        use super::*;

        #[test]
        fn connection_mode_eq() {
            assert_eq!(ConnectionMode::Auto, ConnectionMode::Auto);
        }

        #[test]
        fn connection_mode_ne() {
            assert_ne!(ConnectionMode::Auto, ConnectionMode::AutoTls);
        }
    }

    #[cfg(any(feature = "native-tls", feature = "rustls-tls"))]
    mod tls_kind {
        use super::*;

        #[test]
        fn connection_mode_eq() {
            assert_eq!(TlsKind::Any, TlsKind::Any);
        }

        #[cfg(feature = "native-tls")]
        #[test]
        fn connection_mode_ne_native() {
            assert_ne!(TlsKind::Any, TlsKind::Native);
        }

        #[cfg(feature = "rustls-tls")]
        #[test]
        fn connection_mode_ne_rust() {
            assert_ne!(TlsKind::Any, TlsKind::Rust);
        }
    }

    mod client_builder {
        use super::*;

        #[test]
        fn can_clone() {
            let builder = ClientBuilder::new("imap.example.com", 143);

            let clone = builder.clone();
            assert_eq!(clone.domain, builder.domain);
            assert_eq!(clone.port, builder.port);
        }
    }
}
