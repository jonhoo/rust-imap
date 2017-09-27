use std::io::Error as IoError;
use std::result;
use std::fmt;
use std::error::Error as StdError;
use std::net::TcpStream;

use openssl::ssl::HandshakeError as SslError;
use bufstream::IntoInnerError as BufError;

pub type Result<T> = result::Result<T, Error>;

/// A set of errors that can occur in the IMAP client
#[derive(Debug)]
pub enum Error {
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    Io(IoError),
    /// An error from the `openssl` library.
    Ssl(SslError<TcpStream>),
    /// A BAD response from the IMAP server.
    BadResponse(Vec<String>),
    /// A NO response from the IMAP server.
    NoResponse(Vec<String>),
    /// The connection was terminated unexpectedly.
    ConnectionLost,
    // Error parsing a server response.
    Parse(ParseError),
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

impl From<SslError<TcpStream>> for Error {
    fn from(err: SslError<TcpStream>) -> Error {
        Error::Ssl(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref e) => fmt::Display::fmt(e, f),
            Error::Ssl(ref e) => fmt::Display::fmt(e, f),
            ref e => f.write_str(e.description()),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref e) => e.description(),
            Error::Ssl(ref e) => e.description(),
            Error::Parse(ref e) => e.description(),
            Error::BadResponse(_) => "Bad Response",
            Error::NoResponse(_) => "No Response",
            Error::ConnectionLost => "Connection lost",
            Error::Append => "Could not append mail to mailbox",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::Io(ref e) => Some(e),
            Error::Ssl(ref e) => Some(e),
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
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            _ => None,
        }
    }
}
