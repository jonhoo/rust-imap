/// This trait allows for pluggable authentication schemes. It is used by `Client::authenticate` to
/// [authenticate using SASL](https://tools.ietf.org/html/rfc3501#section-6.2.2).
pub trait Authenticator {
    /// The type of the response to the challenge. This will usually be a `Vec<u8>` or `String`.
    type Response: AsRef<[u8]>;

    /// Each base64-decoded server challenge is passed to `process`.
    /// The returned byte-string is base64-encoded and then sent back to the server.
    fn process(&self, challenge: &[u8]) -> Self::Response;
}
