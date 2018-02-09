use std::net::{TcpStream, ToSocketAddrs};
use native_tls::{TlsConnector, TlsStream};
use std::io::{self, Read, Write};
use std::time::Duration;
use bufstream::BufStream;
use nom::IResult;

use super::types::*;
use super::authenticator::Authenticator;
use super::parse::{parse_authenticate_response, parse_capabilities, parse_fetches, parse_mailbox,
                   parse_names};
use super::error::{Error, ParseError, Result, ValidateError};

static TAG_PREFIX: &'static str = "a";
const INITIAL_TAG: u32 = 0;
const CR: u8 = 0x0d;
const LF: u8 = 0x0a;

macro_rules! quote {
    ($x: expr) => (
        format!("\"{}\"", $x.replace(r"\", r"\\").replace("\"", "\\\""))
    )
}

fn validate_str(value: &str) -> Result<String> {
    let quoted = quote!(value);
    if let Some(_) = quoted.find("\n") {
        return Err(Error::Validate(ValidateError('\n')));
    }
    if let Some(_) = quoted.find("\r") {
        return Err(Error::Validate(ValidateError('\r')));
    }
    Ok(quoted)
}

/// Stream to interface with the IMAP server. This interface is only for the command stream.
#[derive(Debug)]
pub struct Client<T: Read + Write> {
    stream: BufStream<T>,
    tag: u32,
    pub debug: bool,
}

/// `IdleHandle` allows a client to block waiting for changes to the remote mailbox.
///
/// The handle blocks using the IMAP IDLE command specificed in [RFC
/// 2177](https://tools.ietf.org/html/rfc2177).
///
/// As long a the handle is active, the mailbox cannot be otherwise accessed.
#[derive(Debug)]
pub struct IdleHandle<'a, T: Read + Write + 'a> {
    client: &'a mut Client<T>,
    keepalive: Duration,
    done: bool,
}

/// Must be implemented for a transport in order for a `Client` using that transport to support
/// operations with timeouts.
///
/// Examples of where this is useful is for `IdleHandle::wait_keepalive` and
/// `IdleHandle::wait_timeout`.
pub trait SetReadTimeout {
    /// Set the timeout for subsequent reads to the given one.
    ///
    /// If `timeout` is `None`, the read timeout should be removed.
    ///
    /// See also `std::net::TcpStream::set_read_timeout`.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()>;
}

impl<'a, T: Read + Write + 'a> IdleHandle<'a, T> {
    fn new(client: &'a mut Client<T>) -> Result<Self> {
        let mut h = IdleHandle {
            client: client,
            keepalive: Duration::from_secs(29 * 60),
            done: false,
        };
        try!(h.init());
        Ok(h)
    }

    fn init(&mut self) -> Result<()> {
        // https://tools.ietf.org/html/rfc2177
        //
        // The IDLE command takes no arguments.
        try!(self.client.run_command("IDLE"));

        // A tagged response will be sent either
        //
        //   a) if there's an error, or
        //   b) *after* we send DONE
        let mut v = Vec::new();
        try!(self.client.readline(&mut v));
        if v.starts_with(b"+") {
            self.done = false;
            return Ok(());
        }

        self.client.read_response_onto(&mut v)?;
        // We should *only* get a continuation on an error (i.e., it gives BAD or NO).
        unreachable!();
    }

    fn terminate(&mut self) -> Result<()> {
        if !self.done {
            self.done = true;
            try!(self.client.write_line(b"DONE"));
            self.client.read_response().map(|_| ())
        } else {
            Ok(())
        }
    }

    /// Internal helper that doesn't consume self.
    ///
    /// This is necessary so that we can keep using the inner `Client` in `wait_keepalive`.
    fn wait_inner(&mut self) -> Result<()> {
        let mut v = Vec::new();
        match self.client.readline(&mut v).map(|_| ()) {
            Err(Error::Io(ref e))
                if e.kind() == io::ErrorKind::TimedOut || e.kind() == io::ErrorKind::WouldBlock =>
            {
                // we need to refresh the IDLE connection
                try!(self.terminate());
                try!(self.init());
                self.wait_inner()
            }
            r => r,
        }
    }

    /// Block until the selected mailbox changes.
    pub fn wait(mut self) -> Result<()> {
        self.wait_inner()
    }
}

impl<'a, T: SetReadTimeout + Read + Write + 'a> IdleHandle<'a, T> {
    /// Set the keep-alive interval to use when `wait_keepalive` is called.
    ///
    /// The interval defaults to 29 minutes as dictated by RFC 2177.
    pub fn set_keepalive(&mut self, interval: Duration) {
        self.keepalive = interval;
    }

    /// Block until the selected mailbox changes.
    ///
    /// This method differs from `IdleHandle::wait` in that it will periodically refresh the IDLE
    /// connection, to prevent the server from timing out our connection. The keepalive interval is
    /// set to 29 minutes by default, as dictated by RFC 2177, but can be changed using
    /// `set_keepalive`.
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
        let keepalive = self.keepalive;
        self.wait_timeout(keepalive)
    }

    /// Block until the selected mailbox changes, or until the given amount of time has expired.
    pub fn wait_timeout(mut self, timeout: Duration) -> Result<()> {
        try!(self.client.stream.get_mut().set_read_timeout(Some(timeout)));
        let res = self.wait_inner();
        self.client.stream.get_mut().set_read_timeout(None).is_ok();
        res
    }
}

impl<'a, T: Read + Write + 'a> Drop for IdleHandle<'a, T> {
    fn drop(&mut self) {
        // we don't want to panic here if we can't terminate the Idle
        self.terminate().is_ok();
    }
}

impl<'a> SetReadTimeout for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        TcpStream::set_read_timeout(self, timeout).map_err(|e| Error::Io(e))
    }
}

impl<'a> SetReadTimeout for TlsStream<TcpStream> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.get_ref()
            .set_read_timeout(timeout)
            .map_err(|e| Error::Io(e))
    }
}

impl Client<TcpStream> {
    /// Creates a new client.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Client<TcpStream>> {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                let mut socket = Client::new(stream);

                try!(socket.read_greeting());
                Ok(socket)
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    /// This will upgrade a regular TCP connection to use SSL.
    ///
    /// Use the domain parameter for openssl's SNI and hostname verification.
    pub fn secure(
        mut self,
        domain: &str,
        ssl_connector: TlsConnector,
    ) -> Result<Client<TlsStream<TcpStream>>> {
        // TODO This needs to be tested
        self.run_command_and_check_ok("STARTTLS")?;
        TlsConnector::connect(&ssl_connector, domain, try!(self.stream.into_inner()))
            .map(Client::new)
            .map_err(Error::TlsHandshake)
    }
}

impl Client<TlsStream<TcpStream>> {
    /// Creates a client with an SSL wrapper.
    pub fn secure_connect<A: ToSocketAddrs>(
        addr: A,
        domain: &str,
        ssl_connector: TlsConnector,
    ) -> Result<Client<TlsStream<TcpStream>>> {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                let ssl_stream = match TlsConnector::connect(&ssl_connector, domain, stream) {
                    Ok(s) => s,
                    Err(e) => return Err(Error::TlsHandshake(e)),
                };
                let mut socket = Client::new(ssl_stream);

                try!(socket.read_greeting());
                Ok(socket)
            }
            Err(e) => Err(Error::Io(e)),
        }
    }
}

impl<T: Read + Write> Client<T> {
    /// Creates a new client with the underlying stream.
    pub fn new(stream: T) -> Client<T> {
        Client {
            stream: BufStream::new(stream),
            tag: INITIAL_TAG,
            debug: false,
        }
    }

    /// Authenticate will authenticate with the server, using the authenticator given.
    pub fn authenticate<A: Authenticator>(
        &mut self,
        auth_type: &str,
        authenticator: A,
    ) -> Result<()> {
        try!(self.run_command(&format!("AUTHENTICATE {}", auth_type)));
        self.do_auth_handshake(authenticator)
    }

    /// This func does the handshake process once the authenticate command is made.
    fn do_auth_handshake<A: Authenticator>(&mut self, authenticator: A) -> Result<()> {
        // TODO Clean up this code
        loop {
            let mut line = Vec::new();
            try!(self.readline(&mut line));

            if line.starts_with(b"+") {
                let data = try!(parse_authenticate_response(
                    String::from_utf8(line).unwrap()
                ));
                let auth_response = authenticator.process(data);

                try!(self.write_line(auth_response.into_bytes().as_slice()))
            } else {
                return self.read_response_onto(&mut line).map(|_| ());
            }
        }
    }

    /// Log in to the IMAP server.
    pub fn login(&mut self, username: &str, password: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!(
            "LOGIN {} {}",
            validate_str(username)?,
            validate_str(password)?
        ))
    }

    /// Selects a mailbox
    pub fn select(&mut self, mailbox_name: &str) -> Result<Mailbox> {
        self.run_command_and_read_response(&format!("SELECT {}", validate_str(mailbox_name)?))
            .and_then(|lines| parse_mailbox(&lines[..]))
    }

    /// Examine is identical to Select, but the selected mailbox is identified as read-only
    pub fn examine(&mut self, mailbox_name: &str) -> Result<Mailbox> {
        self.run_command_and_read_response(&format!("EXAMINE {}", validate_str(mailbox_name)?))
            .and_then(|lines| parse_mailbox(&lines[..]))
    }

    /// Fetch retreives data associated with a message in the mailbox.
    pub fn fetch(&mut self, sequence_set: &str, query: &str) -> ZeroCopyResult<Vec<Fetch>> {
        self.run_command_and_read_response(&format!("FETCH {} {}", sequence_set, query))
            .and_then(|lines| parse_fetches(lines))
    }

    pub fn uid_fetch(&mut self, uid_set: &str, query: &str) -> ZeroCopyResult<Vec<Fetch>> {
        self.run_command_and_read_response(&format!("UID FETCH {} {}", uid_set, query))
            .and_then(|lines| parse_fetches(lines))
    }

    /// Noop always succeeds, and it does nothing.
    pub fn noop(&mut self) -> Result<()> {
        self.run_command_and_check_ok("NOOP")
    }

    /// Logout informs the server that the client is done with the connection.
    pub fn logout(&mut self) -> Result<()> {
        self.run_command_and_check_ok("LOGOUT")
    }

    /// Create creates a mailbox with the given name.
    pub fn create(&mut self, mailbox_name: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!("CREATE {}", validate_str(mailbox_name)?))
    }

    /// Delete permanently removes the mailbox with the given name.
    pub fn delete(&mut self, mailbox_name: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!("DELETE {}", validate_str(mailbox_name)?))
    }

    /// Rename changes the name of a mailbox.
    pub fn rename(&mut self, current_mailbox_name: &str, new_mailbox_name: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!(
            "RENAME {} {}",
            quote!(current_mailbox_name),
            quote!(new_mailbox_name)
        ))
    }

    /// Subscribe adds the specified mailbox name to the server's set of "active" or "subscribed"
    /// mailboxes as returned by the LSUB command.
    pub fn subscribe(&mut self, mailbox: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!("SUBSCRIBE {}", quote!(mailbox)))
    }

    /// Unsubscribe removes the specified mailbox name from the server's set of
    /// "active" or "subscribed mailboxes as returned by the LSUB command.
    pub fn unsubscribe(&mut self, mailbox: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!("UNSUBSCRIBE {}", quote!(mailbox)))
    }

    /// Capability requests a listing of capabilities that the server supports.
    pub fn capabilities(&mut self) -> ZeroCopyResult<Capabilities> {
        self.run_command_and_read_response(&format!("CAPABILITY"))
            .and_then(|lines| parse_capabilities(lines))
    }

    /// Expunge permanently removes all messages that have the \Deleted flag set from the currently
    /// selected mailbox.
    pub fn expunge(&mut self) -> Result<()> {
        self.run_command_and_check_ok("EXPUNGE")
    }

    /// Check requests a checkpoint of the currently selected mailbox.
    pub fn check(&mut self) -> Result<()> {
        self.run_command_and_check_ok("CHECK")
    }

    /// Close permanently removes all messages that have the \Deleted flag set from the currently
    /// selected mailbox, and returns to the authenticated state from the selected state.
    pub fn close(&mut self) -> Result<()> {
        self.run_command_and_check_ok("CLOSE")
    }

    /// Store alters data associated with a message in the mailbox.
    pub fn store(&mut self, sequence_set: &str, query: &str) -> ZeroCopyResult<Vec<Fetch>> {
        self.run_command_and_read_response(&format!("STORE {} {}", sequence_set, query))
            .and_then(|lines| parse_fetches(lines))
    }

    pub fn uid_store(&mut self, uid_set: &str, query: &str) -> ZeroCopyResult<Vec<Fetch>> {
        self.run_command_and_read_response(&format!("UID STORE {} {}", uid_set, query))
            .and_then(|lines| parse_fetches(lines))
    }

    /// Copy copies the specified message to the end of the specified destination mailbox.
    pub fn copy(&mut self, sequence_set: &str, mailbox_name: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!("COPY {} {}", sequence_set, mailbox_name))
    }

    pub fn uid_copy(&mut self, uid_set: &str, mailbox_name: &str) -> Result<()> {
        self.run_command_and_check_ok(&format!("UID COPY {} {}", uid_set, mailbox_name))
    }

    /// The LIST command returns a subset of names from the complete set
    /// of all names available to the client.
    pub fn list(
        &mut self,
        reference_name: &str,
        mailbox_search_pattern: &str,
    ) -> ZeroCopyResult<Vec<Name>> {
        self.run_command_and_read_response(&format!(
            "LIST {} {}",
            quote!(reference_name),
            mailbox_search_pattern
        )).and_then(|lines| parse_names(lines))
    }

    /// The LSUB command returns a subset of names from the set of names
    /// that the user has declared as being "active" or "subscribed".
    pub fn lsub(
        &mut self,
        reference_name: &str,
        mailbox_search_pattern: &str,
    ) -> ZeroCopyResult<Vec<Name>> {
        self.run_command_and_read_response(&format!(
            "LSUB {} {}",
            quote!(reference_name),
            mailbox_search_pattern
        )).and_then(|lines| parse_names(lines))
    }

    /// The STATUS command requests the status of the indicated mailbox.
    pub fn status(&mut self, mailbox_name: &str, status_data_items: &str) -> Result<Mailbox> {
        self.run_command_and_read_response(&format!(
            "STATUS {} {}",
            validate_str(mailbox_name)?,
            status_data_items
        )).and_then(|lines| parse_mailbox(&lines[..]))
    }

    /// Returns a handle that can be used to block until the state of the currently selected
    /// mailbox changes.
    pub fn idle(&mut self) -> Result<IdleHandle<T>> {
        IdleHandle::new(self)
    }

    /// The APPEND command adds a mail to a mailbox.
    pub fn append(&mut self, folder: &str, content: &[u8]) -> Result<()> {
        try!(self.run_command(
            &format!("APPEND \"{}\" {{{}}}", folder, content.len())
        ));
        let mut v = Vec::new();
        try!(self.readline(&mut v));
        if !v.starts_with(b"+") {
            return Err(Error::Append);
        }
        try!(self.stream.write_all(content));
        try!(self.stream.write_all(b"\r\n"));
        try!(self.stream.flush());
        self.read_response().map(|_| ())
    }

    /// Runs a command and checks if it returns OK.
    pub fn run_command_and_check_ok(&mut self, command: &str) -> Result<()> {
        self.run_command_and_read_response(command).map(|_| ())
    }

    /// Runs any command passed to it.
    pub fn run_command(&mut self, untagged_command: &str) -> Result<()> {
        let command = self.create_command(untagged_command.to_string());
        self.write_line(command.into_bytes().as_slice())
    }

    pub fn run_command_and_read_response(&mut self, untagged_command: &str) -> Result<Vec<u8>> {
        try!(self.run_command(untagged_command));
        self.read_response()
    }

    fn read_response(&mut self) -> Result<Vec<u8>> {
        let mut v = Vec::new();
        self.read_response_onto(&mut v)?;
        Ok(v)
    }

    fn read_response_onto(&mut self, data: &mut Vec<u8>) -> Result<()> {
        let mut continue_from = None;
        let mut try_first = !data.is_empty();
        let match_tag = format!("{}{}", TAG_PREFIX, self.tag);
        loop {
            let line_start = if try_first {
                try_first = false;
                0
            } else {
                let start_new = data.len();
                try!(self.readline(data));
                continue_from.take().unwrap_or(start_new)
            };

            let break_with = {
                use imap_proto::{parse_response, Response, Status};
                let line = &data[line_start..];

                match parse_response(line) {
                    IResult::Done(
                        _,
                        Response::Done {
                            tag,
                            status,
                            information,
                            ..
                        },
                    ) => {
                        assert_eq!(tag.as_bytes(), match_tag.as_bytes());
                        Some(match status {
                            Status::Bad | Status::No => {
                                Err((status, information.map(|s| s.to_string())))
                            }
                            Status::Ok => Ok(()),
                            status => Err((status, None)),
                        })
                    }
                    IResult::Done(..) => None,
                    IResult::Incomplete(..) => {
                        continue_from = Some(line_start);
                        None
                    }
                    _ => Some(Err((Status::Bye, None))),
                }
            };

            match break_with {
                Some(Ok(_)) => {
                    data.truncate(line_start);
                    break Ok(());
                }
                Some(Err((status, expl))) => {
                    use imap_proto::Status;
                    match status {
                        Status::Bad => {
                            break Err(Error::BadResponse(
                                expl.unwrap_or("no explanation given".to_string()),
                            ))
                        }
                        Status::No => {
                            break Err(Error::NoResponse(
                                expl.unwrap_or("no explanation given".to_string()),
                            ))
                        }
                        _ => break Err(Error::Parse(ParseError::Invalid(data.split_off(0)))),
                    }
                }
                None => {}
            }
        }
    }

    fn read_greeting(&mut self) -> Result<()> {
        let mut v = Vec::new();
        try!(self.readline(&mut v));
        Ok(())
    }

    fn readline(&mut self, into: &mut Vec<u8>) -> Result<usize> {
        use std::io::BufRead;
        let read = try!(self.stream.read_until(LF, into));
        if read == 0 {
            return Err(Error::ConnectionLost);
        }

        if self.debug {
            // Remove CRLF
            let len = into.len();
            let line = &into[(len - read - 2)..(len - 2)];
            print!("S: {}\n", String::from_utf8_lossy(line));
        }

        Ok(read)
    }

    fn create_command(&mut self, command: String) -> String {
        self.tag += 1;
        let command = format!("{}{} {}", TAG_PREFIX, self.tag, command);
        return command;
    }

    fn write_line(&mut self, buf: &[u8]) -> Result<()> {
        try!(self.stream.write_all(buf));
        try!(self.stream.write_all(&[CR, LF]));
        try!(self.stream.flush());
        if self.debug {
            print!("C: {}\n", String::from_utf8(buf.to_vec()).unwrap());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::mock_stream::MockStream;
    use super::super::error::Result;

    #[test]
    fn read_response() {
        let response = "a0 OK Logged in.\r\n";
        let mock_stream = MockStream::new(response.as_bytes().to_vec());
        let mut client = Client::new(mock_stream);
        let actual_response = client.read_response().unwrap();
        assert_eq!(Vec::<u8>::new(), actual_response);
    }


    #[test]
    fn fetch_body() {
        let response = "a0 OK Logged in.\r\n\
                        * 2 FETCH (BODY[TEXT] {3}\r\nfoo)\r\n\
                        a0 OK FETCH completed\r\n";
        let mock_stream = MockStream::new(response.as_bytes().to_vec());
        let mut client = Client::new(mock_stream);
        client.read_response().unwrap();
        client.read_response().unwrap();
    }


    #[test]
    fn read_greeting() {
        let greeting = "* OK Dovecot ready.\r\n";
        let mock_stream = MockStream::new(greeting.as_bytes().to_vec());
        let mut client = Client::new(mock_stream);
        client.read_greeting().unwrap();
    }

    #[test]
    fn readline_delay_read() {
        let greeting = "* OK Dovecot ready.\r\n";
        let expected_response: String = greeting.to_string();
        let mock_stream = MockStream::default()
            .with_buf(greeting.as_bytes().to_vec())
            .with_delay();
        let mut client = Client::new(mock_stream);
        let mut v = Vec::new();
        client.readline(&mut v).unwrap();
        let actual_response = String::from_utf8(v).unwrap();
        assert_eq!(expected_response, actual_response);
    }

    #[test]
    fn readline_eof() {
        let mock_stream = MockStream::default().with_eof();
        let mut client = Client::new(mock_stream);
        let mut v = Vec::new();
        if let Err(Error::ConnectionLost) = client.readline(&mut v) {
        } else {
            unreachable!("EOF read did not return connection lost");
        }
    }

    #[test]
    #[should_panic]
    fn readline_err() {
        // TODO Check the error test
        let mock_stream = MockStream::default().with_err();
        let mut client = Client::new(mock_stream);
        let mut v = Vec::new();
        client.readline(&mut v).unwrap();
    }

    #[test]
    fn create_command() {
        let base_command = "CHECK";
        let mock_stream = MockStream::default();
        let mut imap_stream = Client::new(mock_stream);

        let expected_command = format!("a1 {}", base_command);
        let command = imap_stream.create_command(String::from(base_command));
        assert!(
            command == expected_command,
            "expected command doesn't equal actual command"
        );

        let expected_command2 = format!("a2 {}", base_command);
        let command2 = imap_stream.create_command(String::from(base_command));
        assert!(
            command2 == expected_command2,
            "expected command doesn't equal actual command"
        );
    }

    #[test]
    fn login() {
        let response = b"a1 OK Logged in\r\n".to_vec();
        let username = "username";
        let password = "password";
        let command = format!("a1 LOGIN {} {}\r\n", quote!(username), quote!(password));
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.login(username, password).unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid login command"
        );
    }

    #[test]
    fn logout() {
        let response = b"a1 OK Logout completed.\r\n".to_vec();
        let command = format!("a1 LOGOUT\r\n");
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.logout().unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid logout command"
        );
    }

    #[test]
    fn rename() {
        let response = b"a1 OK RENAME completed\r\n".to_vec();
        let current_mailbox_name = "INBOX";
        let new_mailbox_name = "NEWINBOX";
        let command = format!(
            "a1 RENAME {} {}\r\n",
            quote!(current_mailbox_name),
            quote!(new_mailbox_name)
        );
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client
            .rename(current_mailbox_name, new_mailbox_name)
            .unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid rename command"
        );
    }

    #[test]
    fn subscribe() {
        let response = b"a1 OK SUBSCRIBE completed\r\n".to_vec();
        let mailbox = "INBOX";
        let command = format!("a1 SUBSCRIBE {}\r\n", quote!(mailbox));
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.subscribe(mailbox).unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid subscribe command"
        );
    }

    #[test]
    fn unsubscribe() {
        let response = b"a1 OK UNSUBSCRIBE completed\r\n".to_vec();
        let mailbox = "INBOX";
        let command = format!("a1 UNSUBSCRIBE {}\r\n", quote!(mailbox));
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.unsubscribe(mailbox).unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid unsubscribe command"
        );
    }

    #[test]
    fn expunge() {
        let response = b"a1 OK EXPUNGE completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.expunge().unwrap();
        assert!(
            client.stream.get_ref().written_buf == b"a1 EXPUNGE\r\n".to_vec(),
            "Invalid expunge command"
        );
    }

    #[test]
    fn check() {
        let response = b"a1 OK CHECK completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.check().unwrap();
        assert!(
            client.stream.get_ref().written_buf == b"a1 CHECK\r\n".to_vec(),
            "Invalid check command"
        );
    }

    #[test]
    fn examine() {
        let response = b"* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
            * OK [PERMANENTFLAGS ()] Read-only mailbox.\r\n\
            * 1 EXISTS\r\n\
            * 1 RECENT\r\n\
            * OK [UNSEEN 1] First unseen.\r\n\
            * OK [UIDVALIDITY 1257842737] UIDs valid\r\n\
            * OK [UIDNEXT 2] Predicted next UID\r\n\
            a1 OK [READ-ONLY] Select completed.\r\n"
            .to_vec();
        let expected_mailbox = Mailbox {
            flags: vec![
                "\\Answered".to_string(),
                "\\Flagged".to_string(),
                "\\Deleted".to_string(),
                "\\Seen".to_string(),
                "\\Draft".to_string(),
            ],
            exists: 1,
            recent: 1,
            unseen: Some(1),
            permanent_flags: vec![],
            uid_next: Some(2),
            uid_validity: Some(1257842737),
        };
        let mailbox_name = "INBOX";
        let command = format!("a1 EXAMINE {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        let mailbox = client.examine(mailbox_name).unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid examine command"
        );
        assert_eq!(mailbox, expected_mailbox);
    }

    #[test]
    fn select() {
        let response = b"* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
            * OK [PERMANENTFLAGS (\\* \\Answered \\Flagged \\Deleted \\Draft \\Seen)] \
              Read-only mailbox.\r\n\
            * 1 EXISTS\r\n\
            * 1 RECENT\r\n\
            * OK [UNSEEN 1] First unseen.\r\n\
            * OK [UIDVALIDITY 1257842737] UIDs valid\r\n\
            * OK [UIDNEXT 2] Predicted next UID\r\n\
            a1 OK [READ-ONLY] Select completed.\r\n"
            .to_vec();
        let expected_mailbox = Mailbox {
            flags: vec![
                "\\Answered".to_string(),
                "\\Flagged".to_string(),
                "\\Deleted".to_string(),
                "\\Seen".to_string(),
                "\\Draft".to_string(),
            ],
            exists: 1,
            recent: 1,
            unseen: Some(1),
            permanent_flags: vec![
                "\\*".to_string(),
                "\\Answered".to_string(),
                "\\Flagged".to_string(),
                "\\Deleted".to_string(),
                "\\Draft".to_string(),
                "\\Seen".to_string(),
            ],
            uid_next: Some(2),
            uid_validity: Some(1257842737),
        };
        let mailbox_name = "INBOX";
        let command = format!("a1 SELECT {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        let mailbox = client.select(mailbox_name).unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid select command"
        );
        assert_eq!(mailbox, expected_mailbox);
    }

    #[test]
    fn capability() {
        let response = b"* CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n\
            a1 OK CAPABILITY completed\r\n"
            .to_vec();
        let expected_capabilities = vec!["IMAP4rev1", "STARTTLS", "AUTH=GSSAPI", "LOGINDISABLED"];
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        let capabilities = client.capabilities().unwrap();
        assert!(
            client.stream.get_ref().written_buf == b"a1 CAPABILITY\r\n".to_vec(),
            "Invalid capability command"
        );
        assert_eq!(capabilities.len(), 4);
        for e in expected_capabilities {
            assert!(capabilities.has(e));
        }
    }

    #[test]
    fn create() {
        let response = b"a1 OK CREATE completed\r\n".to_vec();
        let mailbox_name = "INBOX";
        let command = format!("a1 CREATE {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.create(mailbox_name).unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid create command"
        );
    }

    #[test]
    fn delete() {
        let response = b"a1 OK DELETE completed\r\n".to_vec();
        let mailbox_name = "INBOX";
        let command = format!("a1 DELETE {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.delete(mailbox_name).unwrap();
        assert!(
            client.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid delete command"
        );
    }

    #[test]
    fn noop() {
        let response = b"a1 OK NOOP completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.noop().unwrap();
        assert!(
            client.stream.get_ref().written_buf == b"a1 NOOP\r\n".to_vec(),
            "Invalid noop command"
        );
    }

    #[test]
    fn close() {
        let response = b"a1 OK CLOSE completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut client = Client::new(mock_stream);
        client.close().unwrap();
        assert!(
            client.stream.get_ref().written_buf == b"a1 CLOSE\r\n".to_vec(),
            "Invalid close command"
        );
    }

    #[test]
    fn store() {
        generic_store(" ", |c, set, query| c.store(set, query));
    }

    #[test]
    fn uid_store() {
        generic_store(" UID ", |c, set, query| c.uid_store(set, query));
    }

    fn generic_store<F, T>(prefix: &str, op: F)
    where
        F: FnOnce(&mut Client<MockStream>, &str, &str) -> Result<T>,
    {
        let res = "* 2 FETCH (FLAGS (\\Deleted \\Seen))\r\n\
                   * 3 FETCH (FLAGS (\\Deleted))\r\n\
                   * 4 FETCH (FLAGS (\\Deleted \\Flagged \\Seen))\r\n\
                   a1 OK STORE completed\r\n";

        generic_with_uid(res, "STORE", "2.4", "+FLAGS (\\Deleted)", prefix, op);
    }

    #[test]
    fn copy() {
        generic_copy(" ", |c, set, query| c.copy(set, query))
    }

    #[test]
    fn uid_copy() {
        generic_copy(" UID ", |c, set, query| c.uid_copy(set, query))
    }

    fn generic_copy<F, T>(prefix: &str, op: F)
    where
        F: FnOnce(&mut Client<MockStream>, &str, &str) -> Result<T>,
    {
        generic_with_uid(
            "OK COPY completed\r\n",
            "COPY",
            "2:4",
            "MEETING",
            prefix,
            op,
        );
    }

    #[test]
    fn fetch() {
        generic_fetch(" ", |c, seq, query| c.fetch(seq, query))
    }

    #[test]
    fn uid_fetch() {
        generic_fetch(" UID ", |c, seq, query| c.uid_fetch(seq, query))
    }

    fn generic_fetch<F, T>(prefix: &str, op: F)
    where
        F: FnOnce(&mut Client<MockStream>, &str, &str) -> Result<T>,
    {
        generic_with_uid("OK FETCH completed\r\n", "FETCH", "1", "BODY[]", prefix, op);
    }

    fn generic_with_uid<F, T>(res: &str, cmd: &str, seq: &str, query: &str, prefix: &str, op: F)
    where
        F: FnOnce(&mut Client<MockStream>, &str, &str) -> Result<T>,
    {
        let resp = format!("a1 {}\r\n", res).as_bytes().to_vec();
        let line = format!("a1{}{} {} {}\r\n", prefix, cmd, seq, query);
        let mut client = Client::new(MockStream::new(resp));
        let _ = op(&mut client, seq, query);
        assert!(
            client.stream.get_ref().written_buf == line.as_bytes().to_vec(),
            "Invalid command"
        );
    }

    #[test]
    fn quote_backslash() {
        assert_eq!("\"test\\\\text\"", quote!(r"test\text"));
    }

    #[test]
    fn quote_dquote() {
        assert_eq!("\"test\\\"text\"", quote!("test\"text"));
    }

    #[test]
    fn validate_random() {
        assert_eq!(
            "\"~iCQ_k;>[&\\\"sVCvUW`e<<P!wJ\"",
            &validate_str("~iCQ_k;>[&\"sVCvUW`e<<P!wJ").unwrap()
        );
    }

    #[test]
    fn validate_newline() {
        if let Err(ref e) = validate_str("test\nstring") {
            if let &Error::Validate(ref ve) = e {
                if ve.0 == '\n' {
                    return;
                }
            }
            panic!("Wrong error: {:?}", e);
        }
        panic!("No error");
    }

    #[test]
    #[allow(unreachable_patterns)]
    fn validate_carriage_return() {
        if let Err(ref e) = validate_str("test\rstring") {
            if let &Error::Validate(ref ve) = e {
                if ve.0 == '\r' {
                    return;
                }
            }
            panic!("Wrong error: {:?}", e);
        }
        panic!("No error");
    }
}
