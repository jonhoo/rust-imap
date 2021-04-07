//! Adds support for the IMAP IDLE command specificed in [RFC
//! 2177](https://tools.ietf.org/html/rfc2177).

use crate::client::Session;
use crate::error::{Error, Result};
use crate::parse::parse_idle;
use crate::types::UnsolicitedResponse;
#[cfg(feature = "tls")]
use native_tls::TlsStream;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// `Handle` allows a client to block waiting for changes to the remote mailbox.
///
/// The handle blocks using the [`IDLE` command](https://tools.ietf.org/html/rfc2177#section-3)
/// specificed in [RFC 2177](https://tools.ietf.org/html/rfc2177) until the underlying server state
/// changes in some way.
///
/// Each of the `wait` functions takes a callback function which receives any responses
/// that arrive on the channel while IDLE. The callback function implements whatever
/// logic is needed to handle the IDLE response, and then returns a [`CallbackAction`]
/// to `Continue` or `Stop` listening on the channel.
/// For users that want the IDLE to exit on any change (the behavior proior to version 3.0),
/// a convenience callback function [`stop_on_any`] is provided.
///
/// ```no_run
/// # use native_tls::TlsConnector;
/// use imap::extensions::idle;
/// let ssl_conn = TlsConnector::builder().build().unwrap();
/// let client = imap::connect(("example.com", 993), "example.com", &ssl_conn)
///     .expect("Could not connect to imap server");
/// let mut imap = client.login("user@example.com", "password")
///     .expect("Could not authenticate");
/// imap.select("INBOX")
///     .expect("Could not select mailbox");
///
/// let idle = imap.idle().expect("Could not IDLE");
///
/// // Exit on any mailbox change
/// let result = idle.wait_keepalive(idle::stop_on_any);
/// ```
///
/// Note that the server MAY consider a client inactive if it has an IDLE command running, and if
/// such a server has an inactivity timeout it MAY log the client off implicitly at the end of its
/// timeout period.  Because of that, clients using IDLE are advised to terminate the IDLE and
/// re-issue it at least every 29 minutes to avoid being logged off. [`Handle::wait_keepalive`]
/// does this. This still allows a client to receive immediate mailbox updates even though it need
/// only "poll" at half hour intervals.
///
/// As long as a [`Handle`] is active, the mailbox cannot be otherwise accessed.
#[derive(Debug)]
pub struct Handle<'a, T: Read + Write> {
    session: &'a mut Session<T>,
    keepalive: Duration,
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

/// Return type for IDLE response callbacks. Tells the IDLE connection
/// if it should continue monitoring the connection or not.
#[derive(Debug, PartialEq, Eq)]
pub enum CallbackAction {
    /// Continue receiving responses from the IDLE connection.
    Continue,
    /// Stop receiving responses, and exit the IDLE wait.
    Stop,
}

/// A convenience function to always cause the IDLE handler to exit on any change.
pub fn stop_on_any(_response: UnsolicitedResponse) -> CallbackAction {
    CallbackAction::Stop
}

/// Must be implemented for a transport in order for a `Session` using that transport to support
/// operations with timeouts.
///
/// Examples of where this is useful is for `Handle::wait_keepalive` and
/// `Handle::wait_timeout`.
pub trait SetReadTimeout {
    /// Set the timeout for subsequent reads to the given one.
    ///
    /// If `timeout` is `None`, the read timeout should be removed.
    ///
    /// See also `std::net::TcpStream::set_read_timeout`.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()>;
}

impl<'a, T: Read + Write + 'a> Handle<'a, T> {
    pub(crate) fn make(session: &'a mut Session<T>) -> Result<Self> {
        let mut h = Handle {
            session,
            keepalive: Duration::from_secs(29 * 60),
            done: false,
        };
        h.init()?;
        Ok(h)
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
    /// This is necessary so that we can keep using the inner `Session` in `wait_keepalive`.
    fn wait_inner<F>(&mut self, reconnect: bool, mut callback: F) -> Result<WaitOutcome>
    where
        F: FnMut(UnsolicitedResponse) -> CallbackAction,
    {
        let mut v = Vec::new();
        let result = loop {
            let rest = match self.session.readline(&mut v) {
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
                        (_rest, Some(Err(r))) => break Err(r),
                        (rest, Some(Ok(response))) => {
                            if let CallbackAction::Stop = callback(response) {
                                break Ok(WaitOutcome::MailboxChanged);
                            }
                            rest
                        }
                        (rest, None) => rest,
                    }
                }
                Err(r) => break Err(r),
            };

            // Update remaining data with unparsed data if needed.
            if rest.is_empty() {
                v.clear();
            } else if rest.len() != v.len() {
                let used = v.len() - rest.len();
                v.drain(0..used);
            }
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

    /// Block until the given callback returns `Stop`, or until a response
    /// arrives that is not explicitly handled by [`UnsolicitedResponse`].
    pub fn wait<F>(mut self, callback: F) -> Result<()>
    where
        F: FnMut(UnsolicitedResponse) -> CallbackAction,
    {
        self.wait_inner(true, callback).map(|_| ())
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> Handle<'a, T> {
    /// Set the keep-alive interval to use when `wait_keepalive` is called.
    ///
    /// The interval defaults to 29 minutes as dictated by RFC 2177.
    pub fn set_keepalive(&mut self, interval: Duration) {
        self.keepalive = interval;
    }

    /// Block until the given callback returns `Stop`, or until a response
    /// arrives that is not explicitly handled by [`UnsolicitedResponse`].
    ///
    /// This method differs from [`Handle::wait`] in that it will periodically refresh the IDLE
    /// connection, to prevent the server from timing out our connection. The keepalive interval is
    /// set to 29 minutes by default, as dictated by RFC 2177, but can be changed using
    /// [`Handle::set_keepalive`].
    ///
    /// This is the recommended method to use for waiting.
    pub fn wait_keepalive<F>(self, callback: F) -> Result<()>
    where
        F: FnMut(UnsolicitedResponse) -> CallbackAction,
    {
        // The server MAY consider a client inactive if it has an IDLE command
        // running, and if such a server has an inactivity timeout it MAY log
        // the client off implicitly at the end of its timeout period.  Because
        // of that, clients using IDLE are advised to terminate the IDLE and
        // re-issue it at least every 29 minutes to avoid being logged off.
        // This still allows a client to receive immediate mailbox updates even
        // though it need only "poll" at half hour intervals.
        let keepalive = self.keepalive;
        self.timed_wait(keepalive, true, callback).map(|_| ())
    }

    /// Block until the given given amount of time has elapsed, the given callback
    /// returns `Stop`, or until a response arrives that is not explicitly handled
    /// by [`UnsolicitedResponse`].
    pub fn wait_with_timeout<F>(self, timeout: Duration, callback: F) -> Result<WaitOutcome>
    where
        F: FnMut(UnsolicitedResponse) -> CallbackAction,
    {
        self.timed_wait(timeout, false, callback)
    }

    fn timed_wait<F>(
        mut self,
        timeout: Duration,
        reconnect: bool,
        callback: F,
    ) -> Result<WaitOutcome>
    where
        F: FnMut(UnsolicitedResponse) -> CallbackAction,
    {
        self.session
            .stream
            .get_mut()
            .set_read_timeout(Some(timeout))?;
        let res = self.wait_inner(reconnect, callback);
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

impl<'a> SetReadTimeout for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        TcpStream::set_read_timeout(self, timeout).map_err(Error::Io)
    }
}

#[cfg(feature = "tls")]
impl<'a> SetReadTimeout for TlsStream<TcpStream> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.get_ref().set_read_timeout(timeout).map_err(Error::Io)
    }
}
