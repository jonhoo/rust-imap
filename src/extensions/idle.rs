//! Adds support for the IMAP IDLE command specificed in [RFC
//! 2177](https://tools.ietf.org/html/rfc2177).

use crate::client::Session;
use crate::error::{Error, Result};
use crate::parse::parse_idle;
use crate::types::UnsolicitedResponse;
use crate::Connection;
#[cfg(feature = "native-tls")]
use native_tls::TlsStream;
#[cfg(feature = "rustls-tls")]
use rustls_connector::TlsStream as RustlsStream;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::ops::DerefMut;
use std::time::Duration;

/// `Handle` allows a client to block waiting for changes to the remote mailbox.
///
/// The handle blocks using the [`IDLE` command](https://tools.ietf.org/html/rfc2177#section-3)
/// specificed in [RFC 2177](https://tools.ietf.org/html/rfc2177) until the underlying server state
/// changes in some way.
///
/// The `wait_while` function takes a callback function which receives any responses
/// that arrive on the channel while IDLE. The callback function implements whatever
/// logic is needed to handle the IDLE response, and then returns a boolean
/// to continue idling (`true`) or stop (`false`).
///
/// For users that want the IDLE to exit on any change (the behavior proior to version 3.0),
/// a convenience callback function [`stop_on_any`] is provided.
///
/// ```no_run
/// use imap::extensions::idle;
/// # #[cfg(feature = "native-tls")]
/// # {
/// let client = imap::ClientBuilder::new("example.com", 993).connect()
///     .expect("Could not connect to imap server");
/// let mut imap = client.login("user@example.com", "password")
///     .expect("Could not authenticate");
/// imap.select("INBOX")
///     .expect("Could not select mailbox");
///
/// // Exit on any mailbox change. By default, connections will be periodically
/// // refreshed in the background.
/// let result = imap.idle().wait_while(idle::stop_on_any);
/// # }
/// ```
///
/// Note that the server MAY consider a client inactive if it has an IDLE command running, and if
/// such a server has an inactivity timeout it MAY log the client off implicitly at the end of its
/// timeout period. Because of that, clients using IDLE are advised to terminate the IDLE and
/// re-issue it at least every 29 minutes to avoid being logged off. This is done by default, but
/// can be disabled by calling [`Handle::keepalive`]
///
/// As long as a [`Handle`] is active, the mailbox cannot be otherwise accessed.
#[derive(Debug)]
pub struct Handle<'a, T: Read + Write> {
    session: &'a mut Session<T>,
    timeout: Duration,
    keepalive: bool,
    done: bool,
}

/// The result of a wait on a [`Handle`]
#[derive(Debug, PartialEq, Eq)]
pub enum WaitOutcome {
    /// The wait timed out
    TimedOut,
    /// The mailbox was modified
    MailboxChanged,
}

/// A convenience function to always cause the IDLE handler to exit on any change.
pub fn stop_on_any(_response: UnsolicitedResponse) -> bool {
    false
}

/// Must be implemented for a transport in order for a `Session` to use IDLE.
pub trait SetReadTimeout {
    /// Set the timeout for subsequent reads to the given one.
    ///
    /// If `timeout` is `None`, the read timeout should be removed.
    ///
    /// See also `std::net::TcpStream::set_read_timeout`.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()>;
}

impl<'a, T: Read + Write + 'a> Handle<'a, T> {
    pub(crate) fn make(session: &'a mut Session<T>) -> Self {
        Handle {
            session,
            timeout: Duration::from_secs(29 * 60),
            keepalive: true,
            done: false,
        }
    }

    fn init(&mut self) -> Result<()> {
        // https://tools.ietf.org/html/rfc2177
        //
        // The IDLE command takes no arguments.
        self.session.run_command("IDLE")?;

        // A tagged response will be sent either
        //
        //   a) if there's an error, or
        //   b) *after* we send DONE
        let mut v = Vec::new();
        self.session.readline(&mut v)?;
        if v.starts_with(b"+") {
            self.done = false;
            return Ok(());
        }

        self.session.read_response_onto(&mut v)?;
        // We should *only* get a continuation on an error (i.e., it gives BAD or NO).
        unreachable!();
    }

    fn terminate(&mut self) -> Result<()> {
        if !self.done {
            self.done = true;
            self.session.write_line(b"DONE")?;
            self.session.read_response().map(|_| ())
        } else {
            Ok(())
        }
    }

    /// Internal helper that doesn't consume self.
    ///
    /// This is necessary so that we can keep using the inner `Session` in `wait_while`.
    fn wait_inner<F>(&mut self, reconnect: bool, mut callback: F) -> Result<WaitOutcome>
    where
        F: FnMut(UnsolicitedResponse) -> bool,
    {
        let mut v = Vec::new();
        let result = loop {
            match self.session.readline(&mut v) {
                Err(Error::Io(ref e))
                    if e.kind() == io::ErrorKind::TimedOut
                        || e.kind() == io::ErrorKind::WouldBlock =>
                {
                    break Ok(WaitOutcome::TimedOut);
                }
                Ok(_len) => {
                    //  Handle Dovecot's imap_idle_notify_interval message
                    if v.eq_ignore_ascii_case(b"* OK Still here\r\n") {
                        v.clear();
                        continue;
                    }
                    match parse_idle(&v) {
                        // Something went wrong parsing.
                        (_rest, Some(Err(r))) => break Err(r),
                        // Complete response. We expect rest to be empty.
                        (rest, Some(Ok(response))) => {
                            if !callback(response) {
                                break Ok(WaitOutcome::MailboxChanged);
                            }

                            // Assert on partial parse in debug builds - we expect
                            // to always parse all or none of the input buffer.
                            // On release builds, we still do the right thing.
                            debug_assert!(
                                rest.is_empty(),
                                "Unexpected partial parse: input: {:?}, output: {:?}",
                                v,
                                rest,
                            );

                            if rest.is_empty() {
                                v.clear();
                            } else {
                                let used = v.len() - rest.len();
                                v.drain(0..used);
                            }
                        }
                        // Incomplete parse - do nothing and read more.
                        (_rest, None) => {}
                    }
                }
                Err(r) => break Err(r),
            };
        };

        // Reconnect on timeout if needed
        match (reconnect, result) {
            (true, Ok(WaitOutcome::TimedOut)) => {
                self.terminate()?;
                self.init()?;
                self.wait_inner(reconnect, callback)
            }
            (_, result) => result,
        }
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> Handle<'a, T> {
    /// Set the timeout duration on the connection. This will also set the frequency
    /// at which the connection is refreshed.
    ///
    /// The interval defaults to 29 minutes as given in RFC 2177.
    pub fn timeout(&mut self, interval: Duration) -> &mut Self {
        self.timeout = interval;
        self
    }

    /// Do not continuously refresh the IDLE connection in the background.
    ///
    /// By default, connections will periodically be refreshed in the background using the
    /// timeout duration set by [`Handle::timeout`]. If you do not want this behaviour, call
    /// this function and the connection will simply IDLE until `wait_while` returns or
    /// the timeout expires.
    pub fn keepalive(&mut self, keepalive: bool) -> &mut Self {
        self.keepalive = keepalive;
        self
    }

    /// Block until the given callback returns `false`, or until a response
    /// arrives that is not explicitly handled by [`UnsolicitedResponse`].
    pub fn wait_while<F>(&mut self, callback: F) -> Result<WaitOutcome>
    where
        F: FnMut(UnsolicitedResponse) -> bool,
    {
        self.init()?;
        // The server MAY consider a client inactive if it has an IDLE command
        // running, and if such a server has an inactivity timeout it MAY log
        // the client off implicitly at the end of its timeout period.  Because
        // of that, clients using IDLE are advised to terminate the IDLE and
        // re-issue it at least every 29 minutes to avoid being logged off.
        // This still allows a client to receive immediate mailbox updates even
        // though it need only "poll" at half hour intervals.
        self.session
            .stream
            .get_mut()
            .set_read_timeout(Some(self.timeout))?;
        let res = self.wait_inner(self.keepalive, callback);
        let _ = self.session.stream.get_mut().set_read_timeout(None).is_ok();
        res
    }
}

impl<'a, T: Read + Write + 'a> Drop for Handle<'a, T> {
    fn drop(&mut self) {
        // we don't want to panic here if we can't terminate the Idle
        let _ = self.terminate().is_ok();
    }
}

impl SetReadTimeout for Connection {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.deref_mut().set_read_timeout(timeout)
    }
}

impl SetReadTimeout for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        TcpStream::set_read_timeout(self, timeout).map_err(Error::Io)
    }
}

#[cfg(feature = "native-tls")]
impl SetReadTimeout for TlsStream<TcpStream> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.get_ref().set_read_timeout(timeout).map_err(Error::Io)
    }
}

#[cfg(feature = "rustls-tls")]
impl SetReadTimeout for RustlsStream<TcpStream> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.get_ref().set_read_timeout(timeout).map_err(Error::Io)
    }
}
