/// This will allow plugable authentication mechanisms.
///
/// This trait is used by `Client::authenticate` to [authenticate
/// using SASL](https://tools.ietf.org/html/rfc3501#section-6.2.2).
pub trait Authenticator {
    /// Type of the response to the challenge. This will usually be a
    /// `Vec<u8>` or `String`. It must not be Base64 encoded: the
    /// library will do it.
    type Response: AsRef<[u8]>;
    /// For each server challenge is passed to `process`. The library
    /// has already decoded the Base64 string into bytes.
    ///
    /// The `process` function should return its response, not Base64
    /// encoded: the library will do it.
    fn process(&self, &[u8]) -> Self::Response;
}
