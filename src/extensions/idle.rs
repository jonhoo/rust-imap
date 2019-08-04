//! Adds support for the IMAP IDLE command specificed in [RFC
//! 2177](https://tools.ietf.org/html/rfc2177).

use client::Session;
use error::{Error, Result};
use native_tls::TlsStream;
use parse;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use unsolicited_responses::UnsolicitedResponseSender;

/// `Handle` allows a client to block waiting for changes to the remote mailbox.
///
/// The handle blocks using the [`IDLE` command](https://tools.ietf.org/html/rfc2177#section-3)
/// specificed in [RFC 2177](https://tools.ietf.org/html/rfc2177) until the underlying server state
/// changes in some way. While idling does inform the client what changes happened on the server,
/// this implementation will currently just block until _anything_ changes, and then notify the
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
pub struct Handle<'a, T: Read + Write + 'a> {
    session: &'a mut Session<T>,
    unsolicited_responses_tx: UnsolicitedResponseSender,
    keepalive: Option<Duration>,
    done: bool,
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
    pub(crate) fn make(
        session: &'a mut Session<T>,
        unsolicited_responses_tx: UnsolicitedResponseSender,
    ) -> Result<Self> {
        let mut h = Handle {
            session,
            unsolicited_responses_tx,
            keepalive: None,
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

    fn terminate(&mut self) -> Result<bool> {
        if !self.done {
            self.done = true;
            self.session.write_line(b"DONE")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Internal helper that doesn't consume self.
    ///
    /// This is necessary so that we can keep using the inner `Session` in `wait_keepalive`.
    fn wait_inner(&mut self) -> Result<()> {
        let mut buffer = Vec::new();
        loop {
            match self.session.readline(&mut buffer).map(|_| ()) {
                Err(Error::Io(ref e))
                    if e.kind() == io::ErrorKind::TimedOut
                        || e.kind() == io::ErrorKind::WouldBlock =>
                {
                    if self.keepalive.is_some() {
                        // we need to refresh the IDLE connection
                        self.terminate()?;
                        self.init()?;
                    }
                }
                Err(err) => return Err(err),
                Ok(_) => {
                    let _ = parse::parse_idle(&buffer, &mut self.unsolicited_responses_tx)?;
                    self.terminate()?;
                    buffer.truncate(0);

                    // Unsolicited responses coming in from the server are not multi-line,
                    // therefore, we don't need to worry about the remaining bytes since
                    // they should always be terminated at a LF.
                    loop {
                        let _ = self.session.readline(&mut buffer)?;
                        let found_ok =
                            parse::parse_idle(&buffer, &mut self.unsolicited_responses_tx)?;
                        if found_ok {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Block until the selected mailbox changes.
    pub fn wait(mut self) -> Result<()> {
        self.wait_inner()
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> Handle<'a, T> {
    /// Set the keep-alive interval to use when `wait_keepalive` is called.
    ///
    /// The interval defaults to 29 minutes as advised by RFC 2177.
    pub fn set_keepalive(&mut self, interval: Duration) {
        self.keepalive = Some(interval);
    }

    /// Block until the selected mailbox changes.
    ///
    /// This method differs from [`Handle::wait`] in that it will periodically refresh the IDLE
    /// connection, to prevent the server from timing out our connection. The keepalive interval is
    /// set to 29 minutes by default, as advised by RFC 2177, but can be changed using
    /// [`Handle::set_keepalive`].
    ///
    /// This is the recommended method to use for waiting.
    pub fn wait_keepalive(mut self) -> Result<()> {
        // The server MAY consider a client inactive if it has an IDLE command
        // running, and if such a server has an inactivity timeout it MAY log
        // the client off implicitly at the end of its timeout period.  Because
        // of that, clients using IDLE are advised to terminate the IDLE and
        // re-issue it at least every 29 minutes to avoid being logged off.
        // This still allows a client to receive immediate mailbox updates even
        // though it need only "poll" at half hour intervals.
        let keepalive = self
            .keepalive
            .get_or_insert_with(|| Duration::from_secs(29 * 60))
            .clone();
        self.wait_timeout(keepalive)
    }

    /// Block until the selected mailbox changes, or until the given amount of time has expired.
    pub fn wait_timeout(mut self, timeout: Duration) -> Result<()> {
        self.session
            .stream
            .get_mut()
            .set_read_timeout(Some(timeout))?;
        let res = self.wait_inner();
        let _ = self.session.stream.get_mut().set_read_timeout(None).is_ok();
        res
    }
}

impl<'a, T: Read + Write + 'a> Drop for Handle<'a, T> {
    fn drop(&mut self) {
        // we don't want to panic here if we can't terminate the Idle
        // If we sent done, then we should suck up the OK.
        if let Ok(true) = self.terminate() {
            // Check status after DONE command.
            let _ = self.session.read_response().is_ok();
        }
    }
}

impl<'a> SetReadTimeout for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        TcpStream::set_read_timeout(self, timeout).map_err(Error::Io)
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> SetReadTimeout for TlsStream<T> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.get_mut().set_read_timeout(timeout)
    }
}
