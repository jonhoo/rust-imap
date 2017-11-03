use std::io::Error as IoError;
use std::result;
use std::fmt;
use std::error::Error as StdError;
use std::net::TcpStream;
use std::string::FromUtf8Error;

use native_tls::HandshakeError as TlsHandshakeError;
use native_tls::Error as TlsError;
use bufstream::IntoInnerError as BufError;

pub type Result<T> = result::Result<T, Error>;

/// A set of errors that can occur in the IMAP client
#[derive(Debug)]
pub enum Error {
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    Io(IoError),
    /// An error from the `native_tls` library during the TLS handshake.
    TlsHandshake(TlsHandshakeError<TcpStream>),
    /// An error from the `native_tls` library while managing the socket.
    Tls(TlsError),
    /// A BAD response from the IMAP server.
    BadResponse(Vec<String>),
    /// A NO response from the IMAP server.
    NoResponse(Vec<String>),
    /// The connection was terminated unexpectedly.
    ConnectionLost,
    // Error parsing a server response.
    Parse(ParseError),
    // Error validating input data
    Validate(ValidateError),
    // Error appending a mail
    Append,
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Error {
        Error::Io(err)
    }
}

impl<T> From<BufError<T>> for Error {
    fn from(err: BufError<T>) -> Error {
        Error::Io(err.into())
    }
}

impl From<TlsHandshakeError<TcpStream>> for Error {
    fn from(err: TlsHandshakeError<TcpStream>) -> Error {
        Error::TlsHandshake(err)
    }
}

impl From<TlsError> for Error {
    fn from(err: TlsError) -> Error {
        Error::Tls(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref e) => fmt::Display::fmt(e, f),
            Error::Tls(ref e) => fmt::Display::fmt(e, f),
            Error::TlsHandshake(ref e) => fmt::Display::fmt(e, f),
            Error::Validate(ref e) => fmt::Display::fmt(e, f),
            Error::BadResponse(ref data) => write!(
                f,
                "{}: {}",
                &String::from(self.description()),
                &data.join("\n")
            ),
            ref e => f.write_str(e.description()),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref e) => e.description(),
            Error::Tls(ref e) => e.description(),
            Error::TlsHandshake(ref e) => e.description(),
            Error::Parse(ref e) => e.description(),
            Error::Validate(ref e) => e.description(),
            Error::BadResponse(_) => "Bad Response",
            Error::NoResponse(_) => "No Response",
            Error::ConnectionLost => "Connection lost",
            Error::Append => "Could not append mail to mailbox",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::Io(ref e) => Some(e),
            Error::Tls(ref e) => Some(e),
            Error::TlsHandshake(ref e) => Some(e),
            Error::Parse(ParseError::DataNotUtf8(ref e)) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    // Indicates an error parsing the status response. Such as OK, NO, and BAD.
    StatusResponse(Vec<String>),
    // Error parsing the cabability response.
    Capability(Vec<String>),
    // Authentication errors.
    Authentication(String),
    DataNotUtf8(FromUtf8Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ref e => f.write_str(e.description()),
        }
    }
}

impl StdError for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::StatusResponse(_) => "Unable to parse status response",
            ParseError::Capability(_) => "Unable to parse capability response",
            ParseError::Authentication(_) => "Unable to parse authentication response",
            ParseError::DataNotUtf8(_) => "Unable to parse data as UTF-8 text",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            _ => None,
        }
    }
}

// Invalid character found. Expand as needed
#[derive(Debug)]
pub struct ValidateError(pub char);

impl fmt::Display for ValidateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // print character in debug form because invalid ones are often whitespaces
        write!(f, "{}: {:?}", self.description(), self.0)
    }
}

impl StdError for ValidateError {
    fn description(&self) -> &str {
        "Invalid character in input"
    }

    fn cause(&self) -> Option<&StdError> {
        None
    }
}
