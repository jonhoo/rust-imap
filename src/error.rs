//! IMAP error types.

use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;
use std::net::TcpStream;
use std::result;
use std::str::Utf8Error;

use base64::DecodeError;
use bufstream::IntoInnerError as BufError;
use imap_proto::{types::ResponseCode, Response};
#[cfg(feature = "native-tls")]
use native_tls::Error as TlsError;
#[cfg(feature = "native-tls")]
use native_tls::HandshakeError as TlsHandshakeError;
#[cfg(feature = "rustls-tls")]
use rustls_connector::HandshakeError as RustlsHandshakeError;

/// A convenience wrapper around `Result` for `imap::Error`.
pub type Result<T> = result::Result<T, Error>;

/// A BAD response from the server, which indicates an error message from the server.
#[derive(Debug)]
#[non_exhaustive]
pub struct Bad {
    /// Human-redable message included with the Bad response.
    pub information: String,
    /// A more specific error status code included with the Bad response.
    pub code: Option<ResponseCode<'static>>,
}

impl fmt::Display for Bad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.information)
    }
}

/// A NO response from the server, which indicates an operational error message from the server.
#[derive(Debug)]
#[non_exhaustive]
pub struct No {
    /// Human-redable message included with the NO response.
    pub information: String,
    /// A more specific error status code included with the NO response.
    pub code: Option<ResponseCode<'static>>,
}

impl fmt::Display for No {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.information)
    }
}

/// A BYE response from the server, which indicates it is going to hang up on us.
#[derive(Debug)]
#[non_exhaustive]
pub struct Bye {
    /// Human-redable message included with the response.
    pub information: String,
    /// A more specific error status code included with the response.
    pub code: Option<ResponseCode<'static>>,
}

impl fmt::Display for Bye {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.information)
    }
}
/// A set of errors that can occur in the IMAP client
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An `io::Error` that occurred while trying to read or write to a network stream.
    Io(IoError),
    /// An error from the `rustls` library during the TLS handshake.
    #[cfg(feature = "rustls-tls")]
    RustlsHandshake(RustlsHandshakeError<TcpStream>),
    /// An error from the `native_tls` library during the TLS handshake.
    #[cfg(feature = "native-tls")]
    TlsHandshake(TlsHandshakeError<TcpStream>),
    /// An error from the `native_tls` library while managing the socket.
    #[cfg(feature = "native-tls")]
    Tls(TlsError),
    /// A BAD response from the IMAP server.
    Bad(Bad),
    /// A NO response from the IMAP server.
    No(No),
    /// A BYE response from the IMAP server.
    Bye(Bye),
    /// The connection was terminated unexpectedly.
    ConnectionLost,
    /// Error parsing a server response.
    Parse(ParseError),
    /// Command inputs were not valid [IMAP
    /// strings](https://tools.ietf.org/html/rfc3501#section-4.3).
    Validate(ValidateError),
    /// Error appending an e-mail.
    Append,
    /// An unexpected response was received. This could be a response from a command,
    /// or an unsolicited response that could not be converted into a local type in
    /// [`UnsolicitedResponse`](crate::types::UnsolicitedResponse).
    Unexpected(Response<'static>),
    /// In response to a STATUS command, the server sent OK without actually sending any STATUS
    /// responses first.
    MissingStatusResponse,
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Error {
        Error::Io(err)
    }
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Error {
        Error::Parse(err)
    }
}

impl<T> From<BufError<T>> for Error {
    fn from(err: BufError<T>) -> Error {
        Error::Io(err.into())
    }
}

#[cfg(feature = "rustls-tls")]
impl From<RustlsHandshakeError<TcpStream>> for Error {
    fn from(err: RustlsHandshakeError<TcpStream>) -> Error {
        Error::RustlsHandshake(err)
    }
}

#[cfg(feature = "native-tls")]
impl From<TlsHandshakeError<TcpStream>> for Error {
    fn from(err: TlsHandshakeError<TcpStream>) -> Error {
        Error::TlsHandshake(err)
    }
}

#[cfg(feature = "native-tls")]
impl From<TlsError> for Error {
    fn from(err: TlsError) -> Error {
        Error::Tls(err)
    }
}

impl<'a> From<Response<'a>> for Error {
    fn from(err: Response<'a>) -> Error {
        Error::Unexpected(err.into_owned())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Io(ref e) => fmt::Display::fmt(e, f),
            #[cfg(feature = "rustls-tls")]
            Error::RustlsHandshake(ref e) => fmt::Display::fmt(e, f),
            #[cfg(feature = "native-tls")]
            Error::Tls(ref e) => fmt::Display::fmt(e, f),
            #[cfg(feature = "native-tls")]
            Error::TlsHandshake(ref e) => fmt::Display::fmt(e, f),
            Error::Validate(ref e) => fmt::Display::fmt(e, f),
            Error::Parse(ref e) => fmt::Display::fmt(e, f),
            Error::No(ref data) => write!(f, "No Response: {}", data),
            Error::Bad(ref data) => write!(f, "Bad Response: {}", data),
            Error::Bye(ref data) => write!(f, "Bye Response: {}", data),
            Error::ConnectionLost => f.write_str("Connection Lost"),
            Error::Append => f.write_str("Could not append mail to mailbox"),
            Error::Unexpected(ref r) => write!(f, "Unexpected Response: {:?}", r),
            Error::MissingStatusResponse => write!(f, "Missing STATUS Response"),
        }
    }
}

impl StdError for Error {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref e) => e.description(),
            #[cfg(feature = "rustls-tls")]
            Error::RustlsHandshake(ref e) => e.description(),
            #[cfg(feature = "native-tls")]
            Error::Tls(ref e) => e.description(),
            #[cfg(feature = "native-tls")]
            Error::TlsHandshake(ref e) => e.description(),
            Error::Parse(ref e) => e.description(),
            Error::Validate(ref e) => e.description(),
            Error::Bad(_) => "Bad Response",
            Error::No(_) => "No Response",
            Error::Bye(_) => "Bye Response",
            Error::ConnectionLost => "Connection lost",
            Error::Append => "Could not append mail to mailbox",
            Error::Unexpected(_) => "Unexpected Response",
            Error::MissingStatusResponse => "Missing STATUS Response",
        }
    }

    fn cause(&self) -> Option<&dyn StdError> {
        match *self {
            Error::Io(ref e) => Some(e),
            #[cfg(feature = "rustls-tls")]
            Error::RustlsHandshake(ref e) => Some(e),
            #[cfg(feature = "native-tls")]
            Error::Tls(ref e) => Some(e),
            #[cfg(feature = "native-tls")]
            Error::TlsHandshake(ref e) => Some(e),
            Error::Parse(ParseError::DataNotUtf8(_, ref e)) => Some(e),
            _ => None,
        }
    }
}

/// An error occured while trying to parse a server response.
#[derive(Debug)]
pub enum ParseError {
    /// Indicates an error parsing the status response. Such as OK, NO, and BAD.
    Invalid(Vec<u8>),
    /// The client could not find or decode the server's authentication challenge.
    Authentication(String, Option<DecodeError>),
    /// The client received data that was not UTF-8 encoded.
    DataNotUtf8(Vec<u8>, Utf8Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ParseError::Invalid(_) => f.write_str("Unable to parse status response"),
            ParseError::Authentication(_, _) => {
                f.write_str("Unable to parse authentication response")
            }
            ParseError::DataNotUtf8(_, _) => f.write_str("Unable to parse data as UTF-8 text"),
        }
    }
}

impl StdError for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::Invalid(_) => "Unable to parse status response",
            ParseError::Authentication(_, _) => "Unable to parse authentication response",
            ParseError::DataNotUtf8(_, _) => "Unable to parse data as UTF-8 text",
        }
    }

    fn cause(&self) -> Option<&dyn StdError> {
        match *self {
            ParseError::Authentication(_, Some(ref e)) => Some(e),
            _ => None,
        }
    }
}

/// An [invalid character](https://tools.ietf.org/html/rfc3501#section-4.3) was found in a command
/// argument.
#[derive(Debug)]
pub struct ValidateError {
    /// the synopsis of the invalid command
    pub command_synopsis: String,
    /// the name of the invalid argument
    pub argument: String,
    /// the invalid character contained in the argument
    pub offending_char: char,
}

impl fmt::Display for ValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // print character in debug form because invalid ones are often whitespaces
        write!(
            f,
            "Invalid character {:?} in argument '{}' of command '{}'",
            self.offending_char, self.argument, self.command_synopsis
        )
    }
}

impl StdError for ValidateError {
    fn description(&self) -> &str {
        "Invalid character in command argument"
    }

    fn cause(&self) -> Option<&dyn StdError> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_error_display() {
        assert_eq!(
            ValidateError {
                command_synopsis: "COMMAND arg1 arg2".to_owned(),
                argument: "arg2".to_string(),
                offending_char: '\n'
            }
            .to_string(),
            "Invalid character '\\n' in argument 'arg2' of command 'COMMAND arg1 arg2'"
        );
    }
}
