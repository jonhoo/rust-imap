//! Adds support for the IMAP IDLE command specificed in [RFC
//! 2177](https://tools.ietf.org/html/rfc2177).

use client::Session;
use error::{Error, Result};
use fallible_iterator::FallibleIterator;
use native_tls::TlsStream;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use types::UnsolicitedResponse;
use parse;


trait OnDrop<'a> {
    fn callback(&self);
}

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
    unsolicited_responses_tx: mpsc::Sender<UnsolicitedResponse>,
    start_time: Instant,
    keepalive: Duration,
    done: bool,
}


/// 'IdleIterator' allows a client to iterate over unsolicited responses during an IDLE operation.
///
/// As long as a [`IdleIterator`] is active, the mailbox cannot be otherwise accessed.
pub struct IdleIterator<'a, T: Read + Write + 'a> {
    handle: Handle<'a, T>,
    buffer: Vec<u8>,
}

impl<'a, T: Read + Write + 'a> IdleIterator<'a, T> {
    fn new(handle: Handle<'a, T>) -> IdleIterator<'a, T> {
        IdleIterator { handle, buffer: Vec::new() }
    }
}

/// 'TimeoutIdleIterator' allows a client to iterate over unsolicited responses during an IDLE operation.
///
/// As long as a [`TimeoutIdleIterator`] is active, the mailbox cannot be otherwise accessed.
pub struct TimeoutIdleIterator<'a, T: SetReadTimeout + Read + Write + 'a> {
    handle: Handle<'a, T>,
    buffer: Vec<u8>,
    should_keepalive: bool,
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> TimeoutIdleIterator<'a, T> {
    fn new(handle: Handle<'a, T>, keepalive: bool) -> TimeoutIdleIterator<'a, T> {
        TimeoutIdleIterator { handle, buffer: Vec::new(), should_keepalive: keepalive }
    }
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
    pub(crate) fn make(session: &'a mut Session<T>, unsolicited_responses_tx: mpsc::Sender<UnsolicitedResponse>) -> Result<Self> {
        let mut h = Handle {
            session,
            unsolicited_responses_tx,
            start_time: Instant::now(),
            keepalive: Duration::from_secs(29 * 60),
            done: false,
        };
        h.init()?;
        Ok(h)
    }

    fn init(&mut self) -> Result<()> {
        self.start_time = Instant::now();

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

    /// Returns an iterator over unsolicited responses.
    ///
    /// The iteration will stop if an error occurs.
    pub fn iter(self) -> IdleIterator<'a, T> {
        IdleIterator::new(self)
    }

    /// Block until the selected mailbox changes.
    pub fn wait(self) -> Result<()> {
        self.iter().next()?;
        Ok(())
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> Handle<'a, T> {
    /// Set the keep-alive interval to use when `wait_keepalive` is called.
    ///
    /// The interval defaults to 29 minutes as dictated by RFC 2177.
    pub fn set_keepalive(&mut self, interval: Duration) {
        self.keepalive = interval;
    }

    /// Returns an iterator over unsolicited responses.
    ///
    /// This method differs from [`Handle::iter`] in that it will periodically refresh the IDLE
    /// connection, to prevent the server from timing out our connection. The keepalive interval is
    /// set to 29 minutes by default, as dictated by RFC 2177, but can be changed using
    /// [`Handle::set_keepalive`].
    ///
    /// This is the recommended method to use for iterating.
    pub fn iter_keepalive(self) -> Result<TimeoutIdleIterator<'a, T>> {
        Ok(TimeoutIdleIterator::new(self, true))
    }

    /// Returns an iterator over unsolicited respones.
    ///
    /// The iteration will stop when the given amount of time has expired.
    pub fn iter_timeout(mut self, timeout: Duration) -> Result<TimeoutIdleIterator<'a, T>> {
        self.keepalive = timeout;
        Ok(TimeoutIdleIterator::new(self, false))
    }

    /// Block until the selected mailbox changes.
    ///
    /// This method differs from [`Handle::wait`] in that it will periodically refresh the IDLE
    /// connection, to prevent the server from timing out our connection. The keepalive interval is
    /// set to 29 minutes by default, as dictated by RFC 2177, but can be changed using
    /// [`Handle::set_keepalive`].
    ///
    /// This is the recommended method to use for waiting.
    pub fn wait_keepalive(self) -> Result<()> {
        // The server MAY consider a client inactive if it has an IDLE command
        // running, and if such a server has an inactivity timeout it MAY log
        // the client off implicitly at the end of its timeout period.  Because
        // of that, clients using IDLE are advised to terminate the IDLE and
        // re-issue it at least every 29 minutes to avoid being logged off.
        // This still allows a client to receive immediate mailbox updates even
        // though it need only "poll" at half hour intervals.
        self.iter_keepalive()?.next()?;
        Ok(())
    }

    /// Block until the selected mailbox changes, or until the given amount of time has expired.
    pub fn wait_timeout(self, timeout: Duration) -> Result<()> {
        self.iter_timeout(timeout)?.next()?;
        Ok(())
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> Drop for TimeoutIdleIterator<'a, T> {
    fn drop(&mut self) {
        self.handle.session.stream.get_mut().set_read_timeout(None).is_ok();
    }
}

impl<'a, T: Read + Write + 'a> Drop for Handle<'a, T> {
    fn drop(&mut self) {
        // we don't want to panic here if we can't terminate the Idle
        self.terminate().is_ok();
    }
}

impl<'a, T: Read + Write + 'a> FallibleIterator for IdleIterator<'a, T> {
    type Item = UnsolicitedResponse;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        loop {
            // The receiver can not be disconnected. If this fails, the channel is empty.
            if let Ok(u) = self.handle.session.unsolicited_responses.try_recv() {
                return Ok(Some(u));
            }
            self.handle.session.readline(&mut self.buffer)?;
            let pos;
            {
                let rest = parse::parse_idle(&self.buffer[..],
                                         &mut self.handle.unsolicited_responses_tx)?;
                // Get what we need from rest before dropping it so that self.buffer can
                // be borrowed as mutable again.
                // offset_from() is nightly only.
                pos = (rest.as_ptr() as usize) - (self.buffer.as_ptr() as usize);
            }
            self.buffer.drain(0..pos);
        }
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> FallibleIterator for TimeoutIdleIterator<'a, T> {
    type Item = UnsolicitedResponse;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        loop {
            // The receiver can not be disconnected. If this fails, the channel is empty.
            if let Ok(u) = self.handle.session.unsolicited_responses.try_recv() {
                return Ok(Some(u));
            }

            let elapsed = self.handle.start_time.elapsed();
            if elapsed >= self.handle.keepalive {
                return Err(Error::Io(io::Error::from(io::ErrorKind::TimedOut)));
            }

            let new_timeout = self.handle.keepalive - elapsed;
            self.handle.session.stream.get_mut().set_read_timeout(Some(new_timeout))?;
            match self.handle.session.readline(&mut self.buffer) {
                Ok(_) => {}
                Err(Error::Io(e)) => {
                    if e.kind() == io::ErrorKind::TimedOut {
                        if let Err(e) = self.handle.terminate() {
                            return Err(e);
                        }
                        if self.should_keepalive {
                            if let Err(e) = self.handle.init() {
                                return Err(e);
                            }
                            continue;
                        } else {
                            return Ok(None);
                        }
                    } else {
                        return Err(Error::Io(e));
                    }
                }
                Err(e) => { return Err(e); }
            }

            let pos;
            {
                let rest = parse::parse_idle(&self.buffer[..],
                                         &mut self.handle.unsolicited_responses_tx)?;
                // Get what we need from rest before dropping it so that self.buffer can
                // be borrowed as mutable again.
                // offset_from() is nightly only.
                pos = (rest.as_ptr() as usize) - (self.buffer.as_ptr() as usize);
            }
            self.buffer.drain(0..pos);
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
