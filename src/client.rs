use base64;
use bufstream::BufStream;
use native_tls::{TlsConnector, TlsStream};
use nom;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::ops::{Deref, DerefMut};
use std::sync::mpsc;

use super::authenticator::Authenticator;
use super::error::{Error, ParseError, Result, ValidateError};
use super::extensions;
use super::parse::*;
use super::types::*;

static TAG_PREFIX: &'static str = "a";
const INITIAL_TAG: u32 = 0;
const CR: u8 = 0x0d;
const LF: u8 = 0x0a;

macro_rules! quote {
    ($x:expr) => {
        format!("\"{}\"", $x.replace(r"\", r"\\").replace("\"", "\\\""))
    };
}

fn validate_str(value: &str) -> Result<String> {
    let quoted = quote!(value);
    if quoted.find('\n').is_some() {
        return Err(Error::Validate(ValidateError('\n')));
    }
    if quoted.find('\r').is_some() {
        return Err(Error::Validate(ValidateError('\r')));
    }
    Ok(quoted)
}

/// An authenticated IMAP session providing the usual IMAP commands. This type is what you get from
/// a succesful login attempt.
///
/// Note that the server *is* allowed to unilaterally send things to the client for messages in
/// a selected mailbox whose status has changed. See the note on [unilateral server responses
/// in RFC 3501](https://tools.ietf.org/html/rfc3501#section-7). Any such messages are parsed out
/// and sent on `Session::unsolicited_responses`.
// Both `Client` and `Session` deref to [`Connection`](struct.Connection.html), the underlying
// primitives type.
#[derive(Debug)]
pub struct Session<T: Read + Write> {
    conn: Connection<T>,
    unsolicited_responses_tx: mpsc::Sender<UnsolicitedResponse>,

    /// Server responses that are not related to the current command. See also the note on
    /// [unilateral server responses in RFC 3501](https://tools.ietf.org/html/rfc3501#section-7).
    pub unsolicited_responses: mpsc::Receiver<UnsolicitedResponse>,
}

/// An (unauthenticated) handle to talk to an IMAP server. This is what you get when first
/// connecting. A succesfull call to [`Client::login`] or [`Client::authenticate`] will return a
/// [`Session`] instance that provides the usual IMAP methods.
// Both `Client` and `Session` deref to [`Connection`](struct.Connection.html), the underlying
// primitives type.
#[derive(Debug)]
pub struct Client<T: Read + Write> {
    conn: Connection<T>,
}

/// The underlying primitives type. Both `Client`(unauthenticated) and `Session`(after succesful
/// login) use a `Connection` internally for the TCP stream primitives.
#[derive(Debug)]
#[doc(hidden)]
pub struct Connection<T: Read + Write> {
    pub(crate) stream: BufStream<T>,
    tag: u32,

    /// Enable debug mode for this connection so that all client-server interactions are printed to
    /// `STDERR`.
    pub debug: bool,
}

// `Deref` instances are so we can make use of the same underlying primitives in `Client` and
// `Session`
impl<T: Read + Write> Deref for Client<T> {
    type Target = Connection<T>;

    fn deref(&self) -> &Connection<T> {
        &self.conn
    }
}

impl<T: Read + Write> DerefMut for Client<T> {
    fn deref_mut(&mut self) -> &mut Connection<T> {
        &mut self.conn
    }
}

impl<T: Read + Write> Deref for Session<T> {
    type Target = Connection<T>;

    fn deref(&self) -> &Connection<T> {
        &self.conn
    }
}

impl<T: Read + Write> DerefMut for Session<T> {
    fn deref_mut(&mut self) -> &mut Connection<T> {
        &mut self.conn
    }
}

/// Connect to a server using an insecure TCP connection.
///
/// The returned [`Client`] is unauthenticated; to access session-related methods (through
/// [`Session`]), use [`Client::login`] or [`Client::authenticate`].
///
/// Consider using [`connect`] for a secured connection where possible.
/// You can upgrade an insecure client to a secure one using [`Client::secure`].
/// ```rust,no_run
/// # extern crate native_tls;
/// # extern crate imap;
/// # use std::io;
/// # use native_tls::TlsConnector;
/// # fn main() {
/// // a plain, unencrypted TCP connection
/// let client = imap::connect_insecure(("imap.example.org", 143)).unwrap();
///
/// // upgrade to SSL
/// let tls = TlsConnector::builder().build().unwrap();
/// let tls_client = client.secure("imap.example.org", &tls);
/// # }
/// ```
pub fn connect_insecure<A: ToSocketAddrs>(addr: A) -> Result<Client<TcpStream>> {
    match TcpStream::connect(addr) {
        Ok(stream) => {
            let mut socket = Client::new(stream);

            socket.read_greeting()?;
            Ok(socket)
        }
        Err(e) => Err(Error::Io(e)),
    }
}

/// Connect to a server using a TLS-encrypted connection.
///
/// The returned [`Client`] is unauthenticated; to access session-related methods (through
/// [`Session`]), use [`Client::login`] or [`Client::authenticate`].
///
/// The domain must be passed in separately from the `TlsConnector` so that the certificate of the
/// IMAP server can be validated.
///
/// # Examples
///
/// ```no_run
/// # extern crate native_tls;
/// # extern crate imap;
/// # use std::io;
/// # use native_tls::TlsConnector;
/// # fn main() {
/// let tls = TlsConnector::builder().build().unwrap();
/// let client = imap::connect(("imap.example.org", 993), "imap.example.org", &tls).unwrap();
/// # }
/// ```
pub fn connect<A: ToSocketAddrs, S: AsRef<str>>(
    addr: A,
    domain: S,
    ssl_connector: &TlsConnector,
) -> Result<Client<TlsStream<TcpStream>>> {
    match TcpStream::connect(addr) {
        Ok(stream) => {
            let ssl_stream = match TlsConnector::connect(ssl_connector, domain.as_ref(), stream) {
                Ok(s) => s,
                Err(e) => return Err(Error::TlsHandshake(e)),
            };
            let mut socket = Client::new(ssl_stream);

            socket.read_greeting()?;
            Ok(socket)
        }
        Err(e) => Err(Error::Io(e)),
    }
}

impl Client<TcpStream> {
    /// This will upgrade an IMAP client from using a regular TCP connection to use TLS.
    ///
    /// The domain parameter is required to perform hostname verification.
    pub fn secure<S: AsRef<str>>(
        mut self,
        domain: S,
        ssl_connector: &TlsConnector,
    ) -> Result<Client<TlsStream<TcpStream>>> {
        // TODO This needs to be tested
        self.run_command_and_check_ok("STARTTLS")?;
        TlsConnector::connect(
            ssl_connector,
            domain.as_ref(),
            self.conn.stream.into_inner()?,
        )
        .map(Client::new)
        .map_err(Error::TlsHandshake)
    }
}

// As the pattern of returning the unauthenticated `Client` (a.k.a. `self`) back with a login error
// is relatively common, it's abstacted away into a macro here.
//
// Note: 1) using `.map_err(|e| (e, self))` or similar here makes the closure own self, so we can't
//          do that.
//       2) in theory we wouldn't need the second parameter, and could just use the identifier
//          `self` from the surrounding function, but being explicit here seems a lot cleaner.
macro_rules! ok_or_unauth_client_err {
    ($r:expr, $self:expr) => {
        match $r {
            Ok(o) => o,
            Err(e) => return Err((e, $self)),
        }
    };
}

impl<T: Read + Write> Client<T> {
    /// Creates a new client over the given stream.
    ///
    /// This method primarily exists for writing tests that mock the underlying transport, but can
    /// also be used to support IMAP over custom tunnels.
    pub fn new(stream: T) -> Client<T> {
        Client {
            conn: Connection {
                stream: BufStream::new(stream),
                tag: INITIAL_TAG,
                debug: false,
            },
        }
    }

    /// Log in to the IMAP server. Upon success a [`Session`](struct.Session.html) instance is
    /// returned; on error the original `Client` instance is returned in addition to the error.
    /// This is because `login` takes ownership of `self`, so in order to try again (e.g. after
    /// prompting the user for credetials), ownership of the original `Client` needs to be
    /// transferred back to the caller.
    ///
    /// ```rust,no_run
    /// # extern crate imap;
    /// # extern crate native_tls;
    /// # use std::io;
    /// # use native_tls::TlsConnector;
    /// # fn main() {
    /// # let tls_connector = TlsConnector::builder().build().unwrap();
    /// let client = imap::connect(
    ///     ("imap.example.org", 993),
    ///     "imap.example.org",
    ///     &tls_connector).unwrap();
    ///
    /// match client.login("user", "pass") {
    ///     Ok(s) => {
    ///         // you are successfully authenticated!
    ///     },
    ///     Err((e, orig_client)) => {
    ///         eprintln!("error logging in: {}", e);
    ///         // prompt user and try again with orig_client here
    ///         return;
    ///     }
    /// }
    /// # }
    /// ```
    pub fn login<U: AsRef<str>, P: AsRef<str>>(
        mut self,
        username: U,
        password: P,
    ) -> ::std::result::Result<Session<T>, (Error, Client<T>)> {
        let u = ok_or_unauth_client_err!(validate_str(username.as_ref()), self);
        let p = ok_or_unauth_client_err!(validate_str(password.as_ref()), self);
        ok_or_unauth_client_err!(
            self.run_command_and_check_ok(&format!("LOGIN {} {}", u, p)),
            self
        );

        Ok(Session::new(self.conn))
    }

    /// Authenticate with the server using the given custom `authenticator` to handle the server's
    /// challenge.
    ///
    /// ```no_run
    /// extern crate imap;
    /// extern crate native_tls;
    /// use native_tls::TlsConnector;
    ///
    /// struct OAuth2 {
    ///     user: String,
    ///     access_token: String,
    /// }
    ///
    /// impl imap::Authenticator for OAuth2 {
    ///     type Response = String;
    ///     fn process(&self, _: &[u8]) -> Self::Response {
    ///         format!(
    ///             "user={}\x01auth=Bearer {}\x01\x01",
    ///             self.user, self.access_token
    ///         )
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let auth = OAuth2 {
    ///         user: String::from("me@example.com"),
    ///         access_token: String::from("<access_token>"),
    ///     };
    ///     let domain = "imap.example.com";
    ///     let tls = TlsConnector::builder().build().unwrap();
    ///     let client = imap::connect((domain, 993), domain, &tls).unwrap();
    ///     match client.authenticate("XOAUTH2", &auth) {
    ///         Ok(session) => {
    ///             // you are successfully authenticated!
    ///         },
    ///         Err((e, orig_client)) => {
    ///             eprintln!("error authenticating: {}", e);
    ///             // prompt user and try again with orig_client here
    ///             return;
    ///         }
    ///     };
    /// }
    /// ```
    pub fn authenticate<A: Authenticator, S: AsRef<str>>(
        mut self,
        auth_type: S,
        authenticator: &A,
    ) -> ::std::result::Result<Session<T>, (Error, Client<T>)> {
        ok_or_unauth_client_err!(
            self.run_command(&format!("AUTHENTICATE {}", auth_type.as_ref())),
            self
        );
        self.do_auth_handshake(authenticator)
    }

    /// This func does the handshake process once the authenticate command is made.
    fn do_auth_handshake<A: Authenticator>(
        mut self,
        authenticator: &A,
    ) -> ::std::result::Result<Session<T>, (Error, Client<T>)> {
        // TODO Clean up this code
        loop {
            let mut line = Vec::new();
            // explicit match blocks neccessary to convert error to tuple and not bind self too
            // early (see also comment on `login`)
            ok_or_unauth_client_err!(self.readline(&mut line), self);

            if line.starts_with(b"+ ") {
                let data = ok_or_unauth_client_err!(
                    parse_authenticate_response(String::from_utf8(line).unwrap()),
                    self
                );
                let challenge = ok_or_unauth_client_err!(
                    base64::decode(data.as_str())
                        .map_err(|e| Error::Parse(ParseError::Authentication(data, Some(e)))),
                    self
                );
                let raw_response = &authenticator.process(&challenge);
                let auth_response = base64::encode(raw_response);
                ok_or_unauth_client_err!(
                    self.write_line(auth_response.into_bytes().as_slice()),
                    self
                );
            } else {
                ok_or_unauth_client_err!(self.read_response_onto(&mut line), self);
                return Ok(Session::new(self.conn));
            }
        }
    }
}

impl<T: Read + Write> Session<T> {
    // not public, just to avoid duplicating the channel creation code
    fn new(conn: Connection<T>) -> Self {
        let (tx, rx) = mpsc::channel();
        Session {
            conn,
            unsolicited_responses: rx,
            unsolicited_responses_tx: tx,
        }
    }

    /// Selects a mailbox
    ///
    /// The `SELECT` command selects a mailbox so that messages in the mailbox can be accessed.
    /// Note that earlier versions of this protocol only required the FLAGS, EXISTS, and RECENT
    /// untagged data; consequently, client implementations SHOULD implement default behavior for
    /// missing data as discussed with the individual item.
    ///
    /// Only one mailbox can be selected at a time in a connection; simultaneous access to multiple
    /// mailboxes requires multiple connections.  The `SELECT` command automatically deselects any
    /// currently selected mailbox before attempting the new selection. Consequently, if a mailbox
    /// is selected and a `SELECT` command that fails is attempted, no mailbox is selected.
    ///
    /// Note that the server *is* allowed to unilaterally send things to the client for messages in
    /// a selected mailbox whose status has changed. See the note on [unilateral server responses
    /// in RFC 3501](https://tools.ietf.org/html/rfc3501#section-7). This means that if you use
    /// [`Connection::run_command_and_read_response`], you *may* see additional untagged `RECENT`,
    /// `EXISTS`, `FETCH`, and `EXPUNGE` responses. You can get them from the
    /// `unsolicited_responses` channel of the [`Session`](struct.Session.html).
    pub fn select<S: AsRef<str>>(&mut self, mailbox_name: S) -> Result<Mailbox> {
        // TODO: also note READ/WRITE vs READ-only mode!
        self.run_command_and_read_response(&format!(
            "SELECT {}",
            validate_str(mailbox_name.as_ref())?
        ))
        .and_then(|lines| parse_mailbox(&lines[..], &mut self.unsolicited_responses_tx))
    }

    /// The `EXAMINE` command is identical to [`Session::select`] and returns the same output;
    /// however, the selected mailbox is identified as read-only. No changes to the permanent state
    /// of the mailbox, including per-user state, will happen in a mailbox opened with `examine`;
    /// in particular, messagess cannot lose [`Flag::Recent`] in an examined mailbox.
    pub fn examine<S: AsRef<str>>(&mut self, mailbox_name: S) -> Result<Mailbox> {
        self.run_command_and_read_response(&format!(
            "EXAMINE {}",
            validate_str(mailbox_name.as_ref())?
        ))
        .and_then(|lines| parse_mailbox(&lines[..], &mut self.unsolicited_responses_tx))
    }

    /// Fetch retreives data associated with a set of messages in the mailbox.
    ///
    /// Note that the server *is* allowed to unilaterally include `FETCH` responses for other
    /// messages in the selected mailbox whose status has changed. See the note on [unilateral
    /// server responses in RFC 3501](https://tools.ietf.org/html/rfc3501#section-7).
    ///
    /// `query` is a list of "data items" (space-separated in parentheses if `>1`). There are three
    /// "macro items" which specify commonly-used sets of data items, and can be used instead of
    /// data items.  A macro must be used by itself, and not in conjunction with other macros or
    /// data items. They are:
    ///
    ///  - `ALL`: equivalent to: `(FLAGS INTERNALDATE RFC822.SIZE ENVELOPE)`
    ///  - `FAST`: equivalent to: `(FLAGS INTERNALDATE RFC822.SIZE)`
    ///
    /// The currently defined data items that can be fetched are listen [in the
    /// RFC](https://tools.ietf.org/html/rfc3501#section-6.4.5), but here are some common ones:
    ///
    ///  - `FLAGS`: The flags that are set for this message.
    ///  - `INTERNALDATE`: The internal date of the message.
    ///  - `BODY[<section>]`:
    ///
    ///     The text of a particular body section.  The section specification is a set of zero or
    ///     more part specifiers delimited by periods.  A part specifier is either a part number
    ///     (see RFC) or one of the following: `HEADER`, `HEADER.FIELDS`, `HEADER.FIELDS.NOT`,
    ///     `MIME`, and `TEXT`.  An empty section specification (i.e., `BODY[]`) refers to the
    ///     entire message, including the header.
    ///
    ///     The `HEADER`, `HEADER.FIELDS`, and `HEADER.FIELDS.NOT` part specifiers refer to the
    ///     [RFC-2822](https://tools.ietf.org/html/rfc2822) header of the message or of an
    ///     encapsulated [MIME-IMT](https://tools.ietf.org/html/rfc2046)
    ///     MESSAGE/[RFC822](https://tools.ietf.org/html/rfc822) message. `HEADER.FIELDS` and
    ///     `HEADER.FIELDS.NOT` are followed by a list of field-name (as defined in
    ///     [RFC-2822](https://tools.ietf.org/html/rfc2822)) names, and return a subset of the
    ///     header.  The subset returned by `HEADER.FIELDS` contains only those header fields with
    ///     a field-name that matches one of the names in the list; similarly, the subset returned
    ///     by `HEADER.FIELDS.NOT` contains only the header fields with a non-matching field-name.
    ///     The field-matching is case-insensitive but otherwise exact.  Subsetting does not
    ///     exclude the [RFC-2822](https://tools.ietf.org/html/rfc2822) delimiting blank line
    ///     between the header and the body; the blank line is included in all header fetches,
    ///     except in the case of a message which has no body and no blank line.
    ///
    ///     The `MIME` part specifier refers to the [MIME-IMB](https://tools.ietf.org/html/rfc2045)
    ///     header for this part.
    ///
    ///     The `TEXT` part specifier refers to the text body of the message,
    ///     omitting the [RFC-2822](https://tools.ietf.org/html/rfc2822) header.
    ///
    ///     [`Flag::Seen`] is implicitly set when `BODY` is fetched; if this causes the flags to
    ///     change, they will generally be included as part of the `FETCH` responses.
    ///  - `BODY.PEEK[<section>]`: An alternate form of `BODY[<section>]` that does not implicitly
    ///    set [`Flag::Seen`].
    ///  - `ENVELOPE`: The envelope structure of the message.  This is computed by the server by
    ///    parsing the [RFC-2822](https://tools.ietf.org/html/rfc2822) header into the component
    ///    parts, defaulting various fields as necessary.
    ///  - `RFC822`: Functionally equivalent to `BODY[]`.
    ///  - `RFC822.HEADER`: Functionally equivalent to `BODY.PEEK[HEADER]`.
    ///  - `RFC822.SIZE`: The [RFC-2822](https://tools.ietf.org/html/rfc2822) size of the message.
    ///  - `UID`: The unique identifier for the message.
    pub fn fetch<S1, S2>(&mut self, sequence_set: S1, query: S2) -> ZeroCopyResult<Vec<Fetch>>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        self.run_command_and_read_response(&format!(
            "FETCH {} {}",
            sequence_set.as_ref(),
            query.as_ref()
        ))
        .and_then(|lines| parse_fetches(lines, &mut self.unsolicited_responses_tx))
    }

    /// Equivalent to [`Session::fetch`], except that all identifiers in `uid_set` are
    /// [`Uid`]s. See also the [`UID` command](https://tools.ietf.org/html/rfc3501#section-6.4.8).
    pub fn uid_fetch<S1, S2>(&mut self, uid_set: S1, query: S2) -> ZeroCopyResult<Vec<Fetch>>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        self.run_command_and_read_response(&format!(
            "UID FETCH {} {}",
            uid_set.as_ref(),
            query.as_ref()
        ))
        .and_then(|lines| parse_fetches(lines, &mut self.unsolicited_responses_tx))
    }

    /// Noop always succeeds, and it does nothing.
    pub fn noop(&mut self) -> Result<()> {
        self.run_command_and_read_response("NOOP")
            .and_then(|lines| parse_noop(lines, &mut self.unsolicited_responses_tx))
    }

    /// Logout informs the server that the client is done with the connection.
    pub fn logout(&mut self) -> Result<()> {
        self.run_command_and_check_ok("LOGOUT")
    }

    /// The [`CREATE` command](https://tools.ietf.org/html/rfc3501#section-6.3.3) creates a mailbox
    /// with the given name.  `Ok` is returned only if a new mailbox with that name has been
    /// created.  It is an error to attempt to create `INBOX` or a mailbox with a name that
    /// refers to an extant mailbox.  Any error in creation will return [`Error::No`].
    ///
    /// If the mailbox name is suffixed with the server's hierarchy separator character (as
    /// returned from the server by [`Session::list`]), this is a declaration that the client
    /// intends to create mailbox names under this name in the hierarchy.  Servers that do not
    /// require this declaration will ignore the declaration.  In any case, the name created is
    /// without the trailing hierarchy delimiter.
    ///
    /// If the server's hierarchy separator character appears elsewhere in the name, the server
    /// will generally create any superior hierarchical names that are needed for the `CREATE`
    /// command to be successfully completed.  In other words, an attempt to create `foo/bar/zap`
    /// on a server in which `/` is the hierarchy separator character will usually create `foo/`
    /// and `foo/bar/` if they do not already exist.
    ///
    /// If a new mailbox is created with the same name as a mailbox which was deleted, its unique
    /// identifiers will be greater than any unique identifiers used in the previous incarnation of
    /// the mailbox UNLESS the new incarnation has a different unique identifier validity value.
    /// See the description of the [`UID`
    /// command](https://tools.ietf.org/html/rfc3501#section-6.4.8) for more detail.
    pub fn create<S: AsRef<str>>(&mut self, mailbox_name: S) -> Result<()> {
        self.run_command_and_check_ok(&format!("CREATE {}", validate_str(mailbox_name.as_ref())?))
    }

    /// The [`DELETE` command](https://tools.ietf.org/html/rfc3501#section-6.3.4) permanently
    /// removes the mailbox with the given name.  `Ok` is returned only if the mailbox has been
    /// deleted.  It is an error to attempt to delete `INBOX` or a mailbox name that does not
    /// exist.
    ///
    /// The `DELETE` command will not remove inferior hierarchical names. For example, if a mailbox
    /// `foo` has an inferior `foo.bar` (assuming `.` is the hierarchy delimiter character),
    /// removing `foo` will not remove `foo.bar`.  It is an error to attempt to delete a name that
    /// has inferior hierarchical names and also has [`NameAttribute::NoSelect`].
    ///
    /// It is permitted to delete a name that has inferior hierarchical names and does not have
    /// [`NameAttribute::NoSelect`].  In this case, all messages in that mailbox are removed, and
    /// the name will acquire [`NameAttribute::NoSelect`].
    ///
    /// The value of the highest-used unique identifier of the deleted mailbox will be preserved so
    /// that a new mailbox created with the same name will not reuse the identifiers of the former
    /// incarnation, UNLESS the new incarnation has a different unique identifier validity value.
    /// See the description of the [`UID`
    /// command](https://tools.ietf.org/html/rfc3501#section-6.4.8) for more detail.
    pub fn delete<S: AsRef<str>>(&mut self, mailbox_name: S) -> Result<()> {
        self.run_command_and_check_ok(&format!("DELETE {}", validate_str(mailbox_name.as_ref())?))
    }

    /// The [`RENAME` command](https://tools.ietf.org/html/rfc3501#section-6.3.5) changes the name
    /// of a mailbox.  `Ok` is returned only if the mailbox has been renamed.  It is an error to
    /// attempt to rename from a mailbox name that does not exist or to a mailbox name that already
    /// exists.  Any error in renaming will return [`Error::No`].
    ///
    /// If the name has inferior hierarchical names, then the inferior hierarchical names will also
    /// be renamed.  For example, a rename of `foo` to `zap` will rename `foo/bar` (assuming `/` is
    /// the hierarchy delimiter character) to `zap/bar`.
    ///
    /// If the server's hierarchy separator character appears in the name, the server will
    /// generally create any superior hierarchical names that are needed for the `RENAME` command
    /// to complete successfully.  In other words, an attempt to rename `foo/bar/zap` to
    /// `baz/rag/zowie` on a server in which `/` is the hierarchy separator character will
    /// generally create `baz/` and `baz/rag/` if they do not already exist.
    ///
    /// The value of the highest-used unique identifier of the old mailbox name will be preserved
    /// so that a new mailbox created with the same name will not reuse the identifiers of the
    /// former incarnation, UNLESS the new incarnation has a different unique identifier validity
    /// value. See the description of the [`UID`
    /// command](https://tools.ietf.org/html/rfc3501#section-6.4.8) for more detail.
    ///
    /// Renaming `INBOX` is permitted, and has special behavior.  It moves all messages in `INBOX`
    /// to a new mailbox with the given name, leaving `INBOX` empty.  If the server implementation
    /// supports inferior hierarchical names of `INBOX`, these are unaffected by a rename of
    /// `INBOX`.
    pub fn rename<S1: AsRef<str>, S2: AsRef<str>>(&mut self, from: S1, to: S2) -> Result<()> {
        self.run_command_and_check_ok(&format!(
            "RENAME {} {}",
            quote!(from.as_ref()),
            quote!(to.as_ref())
        ))
    }

    /// The [`SUBSCRIBE` command](https://tools.ietf.org/html/rfc3501#section-6.3.6) adds the
    /// specified mailbox name to the server's set of "active" or "subscribed" mailboxes as
    /// returned by [`Session::lsub`].  This command returns `Ok` only if the subscription is
    /// successful.
    ///
    /// The server may validate the mailbox argument to `SUBSCRIBE` to verify that it exists.
    /// However, it will not unilaterally remove an existing mailbox name from the subscription
    /// list even if a mailbox by that name no longer exists.
    pub fn subscribe<S: AsRef<str>>(&mut self, mailbox: S) -> Result<()> {
        self.run_command_and_check_ok(&format!("SUBSCRIBE {}", quote!(mailbox.as_ref())))
    }

    /// The [`UNSUBSCRIBE` command](https://tools.ietf.org/html/rfc3501#section-6.3.7) removes the
    /// specified mailbox name from the server's set of "active" or "subscribed" mailboxes as
    /// returned by [`Session::lsub`].  This command returns `Ok` only if the unsubscription is
    /// successful.
    pub fn unsubscribe<S: AsRef<str>>(&mut self, mailbox: S) -> Result<()> {
        self.run_command_and_check_ok(&format!("UNSUBSCRIBE {}", quote!(mailbox.as_ref())))
    }

    /// The [`CAPABILITY` command](https://tools.ietf.org/html/rfc3501#section-6.1.1) requests a
    /// listing of capabilities that the server supports.  The server will include "IMAP4rev1" as
    /// one of the listed capabilities. See [`Capabilities`] for further details.
    pub fn capabilities(&mut self) -> ZeroCopyResult<Capabilities> {
        self.run_command_and_read_response("CAPABILITY")
            .and_then(|lines| parse_capabilities(lines, &mut self.unsolicited_responses_tx))
    }

    /// The [`EXPUNGE` command](https://tools.ietf.org/html/rfc3501#section-6.4.3) permanently
    /// removes all messages that have [`Flag::Deleted`] set from the currently selected mailbox.
    /// The message sequence number of each message that is removed is returned.
    pub fn expunge(&mut self) -> Result<Vec<Seq>> {
        self.run_command_and_read_response("EXPUNGE")
            .and_then(|lines| parse_expunge(lines, &mut self.unsolicited_responses_tx))
    }

    /// The [`UID EXPUNGE` command](https://tools.ietf.org/html/rfc4315#section-2.1) permanently
    /// removes all messages that both have [`Flag::Deleted`] set and have a [`Uid`] that is
    /// included in the specified sequence set from the currently selected mailbox.  If a message
    /// either does not have [`Flag::Deleted`] set or has a [`Uid`] that is not included in the
    /// specified sequence set, it is not affected.
    ///
    /// This command is particularly useful for disconnected use clients. By using [`uid_expunge`]
    /// instead of [`expunge`] when resynchronizing with the server, the client can ensure that it
    /// does not inadvertantly remove any messages that have been marked as [`Flag::Deleted`] by
    /// other clients between the time that the client was last connected and the time the client
    /// resynchronizes.
    ///
    /// This command requires that the server supports [RFC
    /// 4315](https://tools.ietf.org/html/rfc4315) as indicated by the `UIDPLUS` capability (see
    /// [`Session::capabilities`]). If the server does not support the `UIDPLUS` capability, the
    /// client should fall back to using [`Session::store`] to temporarily remove [`Flag::Deleted`]
    /// from messages it does not want to remove, then invoking [`Session::expunge`].  Finally, the
    /// client should use [`Session::store`] to restore [`Flag::Deleted`] on the messages in which
    /// it was temporarily removed.
    ///
    /// Alternatively, the client may fall back to using just [`Session::expunge`], risking the
    /// unintended removal of some messages.
    pub fn uid_expunge<S: AsRef<str>>(&mut self, uid_set: S) -> Result<Vec<Uid>> {
        self.run_command_and_read_response(&format!("UID EXPUNGE {}", uid_set.as_ref()))
            .and_then(|lines| parse_expunge(lines, &mut self.unsolicited_responses_tx))
    }

    /// The [`CHECK` command](https://tools.ietf.org/html/rfc3501#section-6.4.1) requests a
    /// checkpoint of the currently selected mailbox.  A checkpoint refers to any
    /// implementation-dependent housekeeping associated with the mailbox (e.g., resolving the
    /// server's in-memory state of the mailbox with the state on its disk) that is not normally
    /// executed as part of each command.  A checkpoint MAY take a non-instantaneous amount of real
    /// time to complete.  If a server implementation has no such housekeeping considerations,
    /// [`Session::check`] is equivalent to [`Session::noop`].
    ///
    /// There is no guarantee that an `EXISTS` untagged response will happen as a result of
    /// `CHECK`.  [`Session::noop`] SHOULD be used for new message polling.
    pub fn check(&mut self) -> Result<()> {
        self.run_command_and_check_ok("CHECK")
    }

    /// The [`CLOSE` command](https://tools.ietf.org/html/rfc3501#section-6.4.2) permanently
    /// removes all messages that have [`Flag::Deleted`] set from the currently selected mailbox,
    /// and returns to the authenticated state from the selected state.  No `EXPUNGE` responses are
    /// sent.
    ///
    /// No messages are removed, and no error is given, if the mailbox is selected by
    /// [`Session::examine`] or is otherwise selected read-only.
    ///
    /// Even if a mailbox is selected, [`Session::select`], [`Session::examine`], or
    /// [`Session::logout`] command MAY be issued without previously invoking [`Session::close`].
    /// [`Session::select`], [`Session::examine`], and [`Session::logout`] implicitly close the
    /// currently selected mailbox without doing an expunge.  However, when many messages are
    /// deleted, a `CLOSE-LOGOUT` or `CLOSE-SELECT` sequence is considerably faster than an
    /// `EXPUNGE-LOGOUT` or `EXPUNGE-SELECT` because no `EXPUNGE` responses (which the client would
    /// probably ignore) are sent.
    pub fn close(&mut self) -> Result<()> {
        self.run_command_and_check_ok("CLOSE")
    }

    /// The [`STORE` command](https://tools.ietf.org/html/rfc3501#section-6.4.6) alters data
    /// associated with a message in the mailbox.  Normally, `STORE` will return the updated value
    /// of the data with an untagged FETCH response.  A suffix of `.SILENT` in `query` prevents the
    /// untagged `FETCH`, and the server assumes that the client has determined the updated value
    /// itself or does not care about the updated value.
    ///
    /// The currently defined data items that can be stored are:
    ///
    ///  - `FLAGS <flag list>`:
    ///
    ///    Replace the flags for the message (other than [`Flag::Recent`]) with the argument.  The
    ///    new value of the flags is returned as if a `FETCH` of those flags was done.
    ///
    ///  - `FLAGS.SILENT <flag list>`: Equivalent to `FLAGS`, but without returning a new value.
    ///
    ///  - `+FLAGS <flag list>`
    ///
    ///    Add the argument to the flags for the message.  The new value of the flags is returned
    ///    as if a `FETCH` of those flags was done.
    ///  - `+FLAGS.SILENT <flag list>`: Equivalent to `+FLAGS`, but without returning a new value.
    ///
    ///  - `-FLAGS <flag list>`
    ///
    ///    Remove the argument from the flags for the message.  The new value of the flags is
    ///    returned as if a `FETCH` of those flags was done.
    ///
    ///  - `-FLAGS.SILENT <flag list>`: Equivalent to `-FLAGS`, but without returning a new value.
    ///
    /// In all cases, `<flag list>` is a space-separated list enclosed in parentheses.
    ///
    /// # Examples
    ///
    /// Delete a message:
    ///
    /// ```rust,no_run
    /// # extern crate imap;
    /// # use imap::{self, Session};
    /// # use std::net::TcpStream;
    /// fn delete(seq: imap::types::Seq, s: &mut Session<TcpStream>) -> imap::error::Result<()> {
    ///     s.store(format!("{}", seq), "+FLAGS (\\Deleted)")?;
    ///     s.expunge()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn store<S1, S2>(&mut self, sequence_set: S1, query: S2) -> ZeroCopyResult<Vec<Fetch>>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        self.run_command_and_read_response(&format!(
            "STORE {} {}",
            sequence_set.as_ref(),
            query.as_ref()
        ))
        .and_then(|lines| parse_fetches(lines, &mut self.unsolicited_responses_tx))
    }

    /// Equivalent to [`Session::store`], except that all identifiers in `sequence_set` are
    /// [`Uid`]s. See also the [`UID` command](https://tools.ietf.org/html/rfc3501#section-6.4.8).
    pub fn uid_store<S1, S2>(&mut self, uid_set: S1, query: S2) -> ZeroCopyResult<Vec<Fetch>>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        self.run_command_and_read_response(&format!(
            "UID STORE {} {}",
            uid_set.as_ref(),
            query.as_ref()
        ))
        .and_then(|lines| parse_fetches(lines, &mut self.unsolicited_responses_tx))
    }

    /// The [`COPY` command](https://tools.ietf.org/html/rfc3501#section-6.4.7) copies the
    /// specified message(s) to the end of the specified destination mailbox.  The flags and
    /// internal date of the message(s) will generally be preserved, and [`Flag::Recent`] will
    /// generally be set, in the copy.
    ///
    /// If the `COPY` command is unsuccessful for any reason, the server restores the destination
    /// mailbox to its state before the `COPY` attempt.
    pub fn copy<S1: AsRef<str>, S2: AsRef<str>>(
        &mut self,
        sequence_set: S1,
        mailbox_name: S2,
    ) -> Result<()> {
        self.run_command_and_check_ok(&format!(
            "COPY {} {}",
            sequence_set.as_ref(),
            mailbox_name.as_ref()
        ))
    }

    /// Equivalent to [`Session::copy`], except that all identifiers in `sequence_set` are
    /// [`Uid`]s. See also the [`UID` command](https://tools.ietf.org/html/rfc3501#section-6.4.8).
    pub fn uid_copy<S1: AsRef<str>, S2: AsRef<str>>(
        &mut self,
        uid_set: S1,
        mailbox_name: S2,
    ) -> Result<()> {
        self.run_command_and_check_ok(&format!(
            "UID COPY {} {}",
            uid_set.as_ref(),
            mailbox_name.as_ref()
        ))
    }

    /// The [`MOVE` command](https://tools.ietf.org/html/rfc6851#section-3.1) takes two
    /// arguments: a sequence set and a named mailbox. Each message included in the set is moved,
    /// rather than copied, from the selected (source) mailbox to the named (target) mailbox.
    ///
    /// This means that a new message is created in the target mailbox with a
    /// new [`Uid`], the original message is removed from the source mailbox, and
    /// it appears to the client as a single action.  This has the same
    /// effect for each message as this sequence:
    ///
    ///   1. COPY
    ///   2. STORE +FLAGS.SILENT \DELETED
    ///   3. EXPUNGE
    ///
    /// This command requires that the server supports [RFC
    /// 6851](https://tools.ietf.org/html/rfc6851) as indicated by the `MOVE` capability (see
    /// [`Session::capabilities`]).
    ///
    /// Although the effect of the `MOVE` is the same as the preceding steps, the semantics are not
    /// identical: The intermediate states produced by those steps do not occur, and the response
    /// codes are different.  In particular, though the `COPY` and `EXPUNGE` response codes will be
    /// returned, response codes for a `store` will not be generated and [`Flag::Deleted`] will not
    /// be set for any message.
    ///
    /// Because a `MOVE` applies to a set of messages, it might fail partway through the set.
    /// Regardless of whether the command is successful in moving the entire set, each individual
    /// message will either be moved or unaffected.  The server will leave each message in a state
    /// where it is in at least one of the source or target mailboxes (no message can be lost or
    /// orphaned).  The server will generally not leave any message in both mailboxes (it would be
    /// bad for a partial failure to result in a bunch of duplicate messages).  This is true even
    /// if the server returns with [`Error::No`].
    pub fn mv<S1: AsRef<str>, S2: AsRef<str>>(
        &mut self,
        sequence_set: S1,
        mailbox_name: S2,
    ) -> Result<()> {
        self.run_command_and_check_ok(&format!(
            "MOVE {} {}",
            sequence_set.as_ref(),
            validate_str(mailbox_name.as_ref())?
        ))
    }

    /// Equivalent to [`Session::copy`], except that all identifiers in `sequence_set` are
    /// [`Uid`]s. See also the [`UID` command](https://tools.ietf.org/html/rfc3501#section-6.4.8)
    /// and the [semantics of `MOVE` and `UID
    /// MOVE`](https://tools.ietf.org/html/rfc6851#section-3.3).
    pub fn uid_mv<S1: AsRef<str>, S2: AsRef<str>>(
        &mut self,
        uid_set: S1,
        mailbox_name: S2,
    ) -> Result<()> {
        self.run_command_and_check_ok(&format!(
            "UID MOVE {} {}",
            uid_set.as_ref(),
            validate_str(mailbox_name.as_ref())?
        ))
    }

    /// The [`LIST` command](https://tools.ietf.org/html/rfc3501#section-6.3.8) returns a subset of
    /// names from the complete set of all names available to the client.  It returns the name
    /// attributes, hierarchy delimiter, and name of each such name; see [`Name`] for more detail.
    ///
    /// If `reference_name` is `None` (or `""`), the currently selected mailbox is used.
    /// The returned mailbox names must match the supplied `mailbox_pattern`.  A non-empty
    /// reference name argument is the name of a mailbox or a level of mailbox hierarchy, and
    /// indicates the context in which the mailbox name is interpreted.
    ///
    /// If `mailbox_pattern` is `None` (or `""`), it is a special request to return the hierarchy
    /// delimiter and the root name of the name given in the reference.  The value returned as the
    /// root MAY be the empty string if the reference is non-rooted or is an empty string.  In all
    /// cases, a hierarchy delimiter (or `NIL` if there is no hierarchy) is returned.  This permits
    /// a client to get the hierarchy delimiter (or find out that the mailbox names are flat) even
    /// when no mailboxes by that name currently exist.
    ///
    /// The reference and mailbox name arguments are interpreted into a canonical form that
    /// represents an unambiguous left-to-right hierarchy.  The returned mailbox names will be in
    /// the interpreted form.
    ///
    /// The character `*` is a wildcard, and matches zero or more characters at this position.  The
    /// character `%` is similar to `*`, but it does not match a hierarchy delimiter.  If the `%`
    /// wildcard is the last character of a mailbox name argument, matching levels of hierarchy are
    /// also returned.  If these levels of hierarchy are not also selectable mailboxes, they are
    /// returned with [`NameAttribute::NoSelect`].
    ///
    /// The special name `INBOX` is included if `INBOX` is supported by this server for this user
    /// and if the uppercase string `INBOX` matches the interpreted reference and mailbox name
    /// arguments with wildcards.  The criteria for omitting `INBOX` is whether `SELECT INBOX` will
    /// return failure; it is not relevant whether the user's real `INBOX` resides on this or some
    /// other server.
    pub fn list(
        &mut self,
        reference_name: Option<&str>,
        mailbox_pattern: Option<&str>,
    ) -> ZeroCopyResult<Vec<Name>> {
        self.run_command_and_read_response(&format!(
            "LIST {} {}",
            quote!(reference_name.unwrap_or("")),
            mailbox_pattern.unwrap_or("\"\"")
        ))
        .and_then(|lines| parse_names(lines, &mut self.unsolicited_responses_tx))
    }

    /// The [`LSUB` command](https://tools.ietf.org/html/rfc3501#section-6.3.9) returns a subset of
    /// names from the set of names that the user has declared as being "active" or "subscribed".
    /// The arguments to this method the same as for [`Session::list`].
    ///
    /// The returned [`Name`]s MAY contain different mailbox flags from response to
    /// [`Session::list`].  If this should happen, the flags returned by [`Session::list`] are
    /// considered more authoritative.
    ///
    /// A special situation occurs when invoking `lsub` with the `%` wildcard. Consider what
    /// happens if `foo/bar` (with a hierarchy delimiter of `/`) is subscribed but `foo` is not.  A
    /// `%` wildcard to `lsub` must return `foo`, not `foo/bar`, and it will be flagged with
    /// [`NameAttribute::NoSelect`].
    ///
    /// The server will not unilaterally remove an existing mailbox name from the subscription list
    /// even if a mailbox by that name no longer exists.
    pub fn lsub(
        &mut self,
        reference_name: Option<&str>,
        mailbox_pattern: Option<&str>,
    ) -> ZeroCopyResult<Vec<Name>> {
        self.run_command_and_read_response(&format!(
            "LSUB {} {}",
            quote!(reference_name.unwrap_or("")),
            mailbox_pattern.unwrap_or("")
        ))
        .and_then(|lines| parse_names(lines, &mut self.unsolicited_responses_tx))
    }

    /// The [`STATUS` command](https://tools.ietf.org/html/rfc3501#section-6.3.10) requests the
    /// status of the indicated mailbox. It does not change the currently selected mailbox, nor
    /// does it affect the state of any messages in the queried mailbox (in particular, `status`
    /// will not cause messages to lose [`Flag::Recent`]).
    ///
    /// `status` provides an alternative to opening a second [`Session`] and using
    /// [`Session::examine`] on a mailbox to query that mailbox's status without deselecting the
    /// current mailbox in the first `Session`.
    ///
    /// Unlike [`Session::list`], `status` is not guaranteed to be fast in its response.  Under
    /// certain circumstances, it can be quite slow.  In some implementations, the server is
    /// obliged to open the mailbox read-only internally to obtain certain status information.
    /// Also unlike [`Session::list`], `status` does not accept wildcards.
    ///
    /// > Note: `status` is intended to access the status of mailboxes other than the currently
    /// > selected mailbox.  Because `status` can cause the mailbox to be opened internally, and
    /// > because this information is available by other means on the selected mailbox, `status`
    /// > SHOULD NOT be used on the currently selected mailbox.
    ///
    /// The STATUS command MUST NOT be used as a "check for new messages in the selected mailbox"
    /// operation (refer to sections [7](https://tools.ietf.org/html/rfc3501#section-7),
    /// [7.3.1](https://tools.ietf.org/html/rfc3501#section-7.3.1), and
    /// [7.3.2](https://tools.ietf.org/html/rfc3501#section-7.3.2) for more information about the
    /// proper method for new message checking).
    ///
    /// The currently defined status data items that can be requested are:
    ///
    ///  - `MESSAGES`: The number of messages in the mailbox.
    ///  - `RECENT`: The number of messages with [`Flag::Recent`] set.
    ///  - `UIDNEXT`: The next [`Uid`] of the mailbox.
    ///  - `UIDVALIDITY`: The unique identifier validity value of the mailbox (see [`Uid`]).
    ///  - `UNSEEN`: The number of messages which do not have [`Flag::Seen`] set.
    ///
    /// `data_times` is a space-separated list enclosed in parentheses.
    pub fn status<S1: AsRef<str>, S2: AsRef<str>>(
        &mut self,
        mailbox_name: S1,
        data_items: S2,
    ) -> Result<Mailbox> {
        self.run_command_and_read_response(&format!(
            "STATUS {} {}",
            validate_str(mailbox_name.as_ref())?,
            data_items.as_ref()
        ))
        .and_then(|lines| parse_mailbox(&lines[..], &mut self.unsolicited_responses_tx))
    }

    /// This method returns a handle that lets you use the [`IDLE`
    /// command](https://tools.ietf.org/html/rfc2177#section-3) to listen for changes to the
    /// currently selected mailbox.
    ///
    /// It's often more desirable to have the server transmit updates to the client in real time.
    /// This allows a user to see new mail immediately.  It also helps some real-time applications
    /// based on IMAP, which might otherwise need to poll extremely often (such as every few
    /// seconds).  While the spec actually does allow a server to push `EXISTS` responses
    /// aysynchronously, a client can't expect this behaviour and must poll.  This method provides
    /// you with such a mechanism.
    ///
    /// `idle` may be used with any server that returns `IDLE` as one of the supported capabilities
    /// (see [`Session::capabilities`]). If the server does not advertise the `IDLE` capability,
    /// the client MUST NOT use `idle` and must instead poll for mailbox updates.  In particular,
    /// the client MUST continue to be able to accept unsolicited untagged responses to ANY
    /// command, as specified in the base IMAP specification.
    ///
    /// See [`extensions::idle::Handle`] for details.
    pub fn idle(&mut self) -> Result<extensions::idle::Handle<T>> {
        extensions::idle::Handle::make(self)
    }

    /// The [`APPEND` command](https://tools.ietf.org/html/rfc3501#section-6.3.11) appends
    /// `content` as a new message to the end of the specified destination `mailbox`.  This
    /// argument SHOULD be in the format of an [RFC-2822](https://tools.ietf.org/html/rfc2822)
    /// message.
    ///
    /// > Note: There MAY be exceptions, e.g., draft messages, in which required RFC-2822 header
    /// > lines are omitted in the message literal argument to `append`.  The full implications of
    /// > doing so MUST be understood and carefully weighed.
    ///
    /// If the append is unsuccessful for any reason, the mailbox is restored to its state before
    /// the append attempt; no partial appending will happen.
    ///
    /// If the destination `mailbox` does not exist, the server returns an error, and does not
    /// automatically create the mailbox.
    ///
    /// If the mailbox is currently selected, the normal new message actions will generally occur.
    /// Specifically, the server will generally notify the client immediately via an untagged
    /// `EXISTS` response.  If the server does not do so, the client MAY issue a `NOOP` command (or
    /// failing that, a `CHECK` command) after one or more `APPEND` commands.
    pub fn append<S: AsRef<str>, B: AsRef<[u8]>>(&mut self, mailbox: S, content: B) -> Result<()> {
        let content = content.as_ref();
        self.run_command(&format!(
            "APPEND \"{}\" {{{}}}",
            mailbox.as_ref(),
            content.len()
        ))?;
        let mut v = Vec::new();
        self.readline(&mut v)?;
        if !v.starts_with(b"+") {
            return Err(Error::Append);
        }
        self.stream.write_all(content)?;
        self.stream.write_all(b"\r\n")?;
        self.stream.flush()?;
        self.read_response().map(|_| ())
    }

    /// The [`SEARCH` command](https://tools.ietf.org/html/rfc3501#section-6.4.4) searches the
    /// mailbox for messages that match the given `query`.  `query` consist of one or more search
    /// keys separated by spaces.  The response from the server contains a listing of [`Seq`]s
    /// corresponding to those messages that match the searching criteria.
    ///
    /// When multiple search keys are specified, the result is the intersection of all the messages
    /// that match those keys.  Or, in other words, only messages that match *all* the keys. For
    /// example, the criteria
    ///
    /// ```text
    /// DELETED FROM "SMITH" SINCE 1-Feb-1994
    /// ```
    ///
    /// refers to all deleted messages from Smith that were placed in the mailbox since February 1,
    /// 1994.  A search key can also be a parenthesized list of one or more search keys (e.g., for
    /// use with the `OR` and `NOT` keys).
    ///
    /// In all search keys that use strings, a message matches the key if the string is a substring
    /// of the field.  The matching is case-insensitive.
    ///
    /// Below is a selection of common search keys.  The full list can be found in the
    /// specification of the [`SEARCH command`](https://tools.ietf.org/html/rfc3501#section-6.4.4).
    ///
    ///  - `NEW`: Messages that have [`Flag::Recent`] set but not [`Flag::Seen`]. This is functionally equivalent to `(RECENT UNSEEN)`.
    ///  - `OLD`: Messages that do not have [`Flag::Recent`] set.  This is functionally equivalent to `NOT RECENT` (as opposed to `NOT NEW`).
    ///  - `RECENT`: Messages that have [`Flag::Recent`] set.
    ///  - `ANSWERED`: Messages with [`Flag::Answered`] set.
    ///  - `DELETED`: Messages with [`Flag::Deleted`] set.
    ///  - `DRAFT`: Messages with [`Flag::Draft`] set.
    ///  - `FLAGGED`: Messages with [`Flag::Flagged`] set.
    ///  - `SEEN`: Messages that have [`Flag::Seen`] set.
    ///  - `<sequence set>`: Messages with message sequence numbers corresponding to the specified message sequence number set.
    ///  - `UID <sequence set>`: Messages with [`Uid`] corresponding to the specified unique identifier set.  Sequence set ranges are permitted.
    ///
    ///  - `SUBJECT <string>`: Messages that contain the specified string in the envelope structure's `SUBJECT` field.
    ///  - `BODY <string>`: Messages that contain the specified string in the body of the message.
    ///  - `FROM <string>`: Messages that contain the specified string in the envelope structure's `FROM` field.
    ///  - `TO <string>`: Messages that contain the specified string in the envelope structure's `TO` field.
    ///
    ///  - `NOT <search-key>`: Messages that do not match the specified search key.
    ///  - `OR <search-key1> <search-key2>`: Messages that match either search key.
    ///
    ///  - `BEFORE <date>`: Messages whose internal date (disregarding time and timezone) is earlier than the specified date.
    ///  - `SINCE <date>`: Messages whose internal date (disregarding time and timezone) is within or later than the specified date.
    pub fn search<S: AsRef<str>>(&mut self, query: S) -> Result<HashSet<Seq>> {
        self.run_command_and_read_response(&format!("SEARCH {}", query.as_ref()))
            .and_then(|lines| parse_ids(&lines, &mut self.unsolicited_responses_tx))
    }

    /// Equivalent to [`Session::search`], except that the returned identifiers
    /// are [`Uid`] instead of [`Seq`]. See also the [`UID`
    /// command](https://tools.ietf.org/html/rfc3501#section-6.4.8).
    pub fn uid_search<S: AsRef<str>>(&mut self, query: S) -> Result<HashSet<Uid>> {
        self.run_command_and_read_response(&format!("UID SEARCH {}", query.as_ref()))
            .and_then(|lines| parse_ids(&lines, &mut self.unsolicited_responses_tx))
    }

    // these are only here because they are public interface, the rest is in `Connection`
    /// Runs a command and checks if it returns OK.
    pub fn run_command_and_check_ok<S: AsRef<str>>(&mut self, command: S) -> Result<()> {
        self.run_command_and_read_response(command).map(|_| ())
    }

    /// Runs any command passed to it.
    pub fn run_command<S: AsRef<str>>(&mut self, untagged_command: S) -> Result<()> {
        self.conn.run_command(untagged_command.as_ref())
    }

    /// Run a raw IMAP command and read back its response.
    ///
    /// Note that the server *is* allowed to unilaterally send things to the client for messages in
    /// a selected mailbox whose status has changed. See the note on [unilateral server responses
    /// in RFC 3501](https://tools.ietf.org/html/rfc3501#section-7). This means that you *may* see
    /// additional untagged `RECENT`, `EXISTS`, `FETCH`, and `EXPUNGE` responses!
    pub fn run_command_and_read_response<S: AsRef<str>>(
        &mut self,
        untagged_command: S,
    ) -> Result<Vec<u8>> {
        self.conn
            .run_command_and_read_response(untagged_command.as_ref())
    }
}

impl<T: Read + Write> Connection<T> {
    fn read_greeting(&mut self) -> Result<()> {
        let mut v = Vec::new();
        self.readline(&mut v)?;
        Ok(())
    }

    fn run_command_and_check_ok(&mut self, command: &str) -> Result<()> {
        self.run_command_and_read_response(command).map(|_| ())
    }

    fn run_command(&mut self, untagged_command: &str) -> Result<()> {
        let command = self.create_command(untagged_command);
        self.write_line(command.into_bytes().as_slice())
    }

    fn run_command_and_read_response(&mut self, untagged_command: &str) -> Result<Vec<u8>> {
        self.run_command(untagged_command)?;
        self.read_response()
    }

    pub(crate) fn read_response(&mut self) -> Result<Vec<u8>> {
        let mut v = Vec::new();
        self.read_response_onto(&mut v)?;
        Ok(v)
    }

    pub(crate) fn read_response_onto(&mut self, data: &mut Vec<u8>) -> Result<()> {
        let mut continue_from = None;
        let mut try_first = !data.is_empty();
        let match_tag = format!("{}{}", TAG_PREFIX, self.tag);
        loop {
            let line_start = if try_first {
                try_first = false;
                0
            } else {
                let start_new = data.len();
                self.readline(data)?;
                continue_from.take().unwrap_or(start_new)
            };

            let break_with = {
                use imap_proto::{parse_response, Response, Status};
                let line = &data[line_start..];

                match parse_response(line) {
                    Ok((
                        _,
                        Response::Done {
                            tag,
                            status,
                            information,
                            ..
                        },
                    )) => {
                        assert_eq!(tag.as_bytes(), match_tag.as_bytes());
                        Some(match status {
                            Status::Bad | Status::No => {
                                Err((status, information.map(ToString::to_string)))
                            }
                            Status::Ok => Ok(()),
                            status => Err((status, None)),
                        })
                    }
                    Ok((..)) => None,
                    Err(nom::Err::Incomplete(..)) => {
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
                            break Err(Error::Bad(
                                expl.unwrap_or_else(|| "no explanation given".to_string()),
                            ));
                        }
                        Status::No => {
                            break Err(Error::No(
                                expl.unwrap_or_else(|| "no explanation given".to_string()),
                            ));
                        }
                        _ => break Err(Error::Parse(ParseError::Invalid(data.split_off(0)))),
                    }
                }
                None => {}
            }
        }
    }

    pub(crate) fn readline(&mut self, into: &mut Vec<u8>) -> Result<usize> {
        use std::io::BufRead;
        let read = self.stream.read_until(LF, into)?;
        if read == 0 {
            return Err(Error::ConnectionLost);
        }

        if self.debug {
            // Remove CRLF
            let len = into.len();
            let line = &into[(len - read)..(len - 2)];
            eprint!("S: {}\n", String::from_utf8_lossy(line));
        }

        Ok(read)
    }

    fn create_command(&mut self, command: &str) -> String {
        self.tag += 1;
        format!("{}{} {}", TAG_PREFIX, self.tag, command)
    }

    pub(crate) fn write_line(&mut self, buf: &[u8]) -> Result<()> {
        self.stream.write_all(buf)?;
        self.stream.write_all(&[CR, LF])?;
        self.stream.flush()?;
        if self.debug {
            eprint!("C: {}\n", String::from_utf8(buf.to_vec()).unwrap());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::error::Result;
    use super::super::mock_stream::MockStream;
    use super::*;

    macro_rules! mock_session {
        ($s:expr) => {
            Session::new(Client::new($s).conn)
        };
    }

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
        let mut session = mock_session!(mock_stream);
        session.read_response().unwrap();
        session.read_response().unwrap();
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
        let command = imap_stream.create_command(&base_command);
        assert!(
            command == expected_command,
            "expected command doesn't equal actual command"
        );

        let expected_command2 = format!("a2 {}", base_command);
        let command2 = imap_stream.create_command(&base_command);
        assert!(
            command2 == expected_command2,
            "expected command doesn't equal actual command"
        );
    }

    #[test]
    fn authenticate() {
        let response = b"+ YmFy\r\n\
                         a1 OK Logged in\r\n"
            .to_vec();
        let command = "a1 AUTHENTICATE PLAIN\r\n\
                       Zm9v\r\n";
        let mock_stream = MockStream::new(response);
        let client = Client::new(mock_stream);
        enum Authenticate {
            Auth,
        };
        impl Authenticator for Authenticate {
            type Response = Vec<u8>;
            fn process(&self, challenge: &[u8]) -> Self::Response {
                assert!(challenge == b"bar", "Invalid authenticate challenge");
                b"foo".to_vec()
            }
        }
        let session = client.authenticate("PLAIN", &Authenticate::Auth).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid authenticate command"
        );
    }

    #[test]
    fn login() {
        let response = b"a1 OK Logged in\r\n".to_vec();
        let username = "username";
        let password = "password";
        let command = format!("a1 LOGIN {} {}\r\n", quote!(username), quote!(password));
        let mock_stream = MockStream::new(response);
        let client = Client::new(mock_stream);
        let session = client.login(username, password).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid login command"
        );
    }

    #[test]
    fn logout() {
        let response = b"a1 OK Logout completed.\r\n".to_vec();
        let command = format!("a1 LOGOUT\r\n");
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.logout().unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
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
        let mut session = mock_session!(mock_stream);
        session
            .rename(current_mailbox_name, new_mailbox_name)
            .unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid rename command"
        );
    }

    #[test]
    fn subscribe() {
        let response = b"a1 OK SUBSCRIBE completed\r\n".to_vec();
        let mailbox = "INBOX";
        let command = format!("a1 SUBSCRIBE {}\r\n", quote!(mailbox));
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.subscribe(mailbox).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid subscribe command"
        );
    }

    #[test]
    fn unsubscribe() {
        let response = b"a1 OK UNSUBSCRIBE completed\r\n".to_vec();
        let mailbox = "INBOX";
        let command = format!("a1 UNSUBSCRIBE {}\r\n", quote!(mailbox));
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.unsubscribe(mailbox).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid unsubscribe command"
        );
    }

    #[test]
    fn expunge() {
        let response = b"a1 OK EXPUNGE completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.expunge().unwrap();
        assert!(
            session.stream.get_ref().written_buf == b"a1 EXPUNGE\r\n".to_vec(),
            "Invalid expunge command"
        );
    }

    #[test]
    fn uid_expunge() {
        let response = b"* 2 EXPUNGE\r\n\
            * 3 EXPUNGE\r\n\
            * 4 EXPUNGE\r\n\
            a1 OK UID EXPUNGE completed\r\n"
            .to_vec();
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.uid_expunge("2:4").unwrap();
        assert!(
            session.stream.get_ref().written_buf == b"a1 UID EXPUNGE 2:4\r\n".to_vec(),
            "Invalid expunge command"
        );
    }

    #[test]
    fn check() {
        let response = b"a1 OK CHECK completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.check().unwrap();
        assert!(
            session.stream.get_ref().written_buf == b"a1 CHECK\r\n".to_vec(),
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
                Flag::Answered,
                Flag::Flagged,
                Flag::Deleted,
                Flag::Seen,
                Flag::Draft,
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
        let mut session = mock_session!(mock_stream);
        let mailbox = session.examine(mailbox_name).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
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
                Flag::Answered,
                Flag::Flagged,
                Flag::Deleted,
                Flag::Seen,
                Flag::Draft,
            ],
            exists: 1,
            recent: 1,
            unseen: Some(1),
            permanent_flags: vec![
                Flag::MayCreate,
                Flag::Answered,
                Flag::Flagged,
                Flag::Deleted,
                Flag::Draft,
                Flag::Seen,
            ],
            uid_next: Some(2),
            uid_validity: Some(1257842737),
        };
        let mailbox_name = "INBOX";
        let command = format!("a1 SELECT {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        let mailbox = session.select(mailbox_name).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid select command"
        );
        assert_eq!(mailbox, expected_mailbox);
    }

    #[test]
    fn search() {
        let response = b"* SEARCH 1 2 3 4 5\r\n\
            a1 OK Search completed\r\n"
            .to_vec();
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        let ids = session.search("Unseen").unwrap();
        let ids: HashSet<u32> = ids.iter().cloned().collect();
        assert!(
            session.stream.get_ref().written_buf == b"a1 SEARCH Unseen\r\n".to_vec(),
            "Invalid search command"
        );
        assert_eq!(ids, [1, 2, 3, 4, 5].iter().cloned().collect());
    }

    #[test]
    fn uid_search() {
        let response = b"* SEARCH 1 2 3 4 5\r\n\
            a1 OK Search completed\r\n"
            .to_vec();
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        let ids = session.uid_search("Unseen").unwrap();
        let ids: HashSet<Uid> = ids.iter().cloned().collect();
        assert!(
            session.stream.get_ref().written_buf == b"a1 UID SEARCH Unseen\r\n".to_vec(),
            "Invalid search command"
        );
        assert_eq!(ids, [1, 2, 3, 4, 5].iter().cloned().collect());
    }

    #[test]
    fn capability() {
        let response = b"* CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n\
            a1 OK CAPABILITY completed\r\n"
            .to_vec();
        let expected_capabilities = vec!["IMAP4rev1", "STARTTLS", "AUTH=GSSAPI", "LOGINDISABLED"];
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        let capabilities = session.capabilities().unwrap();
        assert!(
            session.stream.get_ref().written_buf == b"a1 CAPABILITY\r\n".to_vec(),
            "Invalid capability command"
        );
        assert_eq!(capabilities.len(), 4);
        for e in expected_capabilities {
            assert!(capabilities.has_str(e));
        }
    }

    #[test]
    fn create() {
        let response = b"a1 OK CREATE completed\r\n".to_vec();
        let mailbox_name = "INBOX";
        let command = format!("a1 CREATE {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.create(mailbox_name).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid create command"
        );
    }

    #[test]
    fn delete() {
        let response = b"a1 OK DELETE completed\r\n".to_vec();
        let mailbox_name = "INBOX";
        let command = format!("a1 DELETE {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.delete(mailbox_name).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid delete command"
        );
    }

    #[test]
    fn noop() {
        let response = b"a1 OK NOOP completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.noop().unwrap();
        assert!(
            session.stream.get_ref().written_buf == b"a1 NOOP\r\n".to_vec(),
            "Invalid noop command"
        );
    }

    #[test]
    fn close() {
        let response = b"a1 OK CLOSE completed\r\n".to_vec();
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.close().unwrap();
        assert!(
            session.stream.get_ref().written_buf == b"a1 CLOSE\r\n".to_vec(),
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
        F: FnOnce(&mut Session<MockStream>, &str, &str) -> Result<T>,
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
        F: FnOnce(&mut Session<MockStream>, &str, &str) -> Result<T>,
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
    fn mv() {
        let response = b"* OK [COPYUID 1511554416 142,399 41:42] Moved UIDs.\r\n\
            * 2 EXPUNGE\r\n\
            * 1 EXPUNGE\r\n\
            a1 OK Move completed\r\n"
            .to_vec();
        let mailbox_name = "MEETING";
        let command = format!("a1 MOVE 1:2 {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.mv("1:2", mailbox_name).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid move command"
        );
    }

    #[test]
    fn uid_mv() {
        let response = b"* OK [COPYUID 1511554416 142,399 41:42] Moved UIDs.\r\n\
            * 2 EXPUNGE\r\n\
            * 1 EXPUNGE\r\n\
            a1 OK Move completed\r\n"
            .to_vec();
        let mailbox_name = "MEETING";
        let command = format!("a1 UID MOVE 41:42 {}\r\n", quote!(mailbox_name));
        let mock_stream = MockStream::new(response);
        let mut session = mock_session!(mock_stream);
        session.uid_mv("41:42", mailbox_name).unwrap();
        assert!(
            session.stream.get_ref().written_buf == command.as_bytes().to_vec(),
            "Invalid uid move command"
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
        F: FnOnce(&mut Session<MockStream>, &str, &str) -> Result<T>,
    {
        generic_with_uid("OK FETCH completed\r\n", "FETCH", "1", "BODY[]", prefix, op);
    }

    fn generic_with_uid<F, T>(res: &str, cmd: &str, seq: &str, query: &str, prefix: &str, op: F)
    where
        F: FnOnce(&mut Session<MockStream>, &str, &str) -> Result<T>,
    {
        let resp = format!("a1 {}\r\n", res).as_bytes().to_vec();
        let line = format!("a1{}{} {} {}\r\n", prefix, cmd, seq, query);
        let mut session = mock_session!(MockStream::new(resp));
        let _ = op(&mut session, seq, query);
        assert!(
            session.stream.get_ref().written_buf == line.as_bytes().to_vec(),
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
