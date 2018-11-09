/// This will allow plugable authentication mechanisms.
pub trait Authenticator {
    type Response: AsRef<[u8]>;
    fn process(&self, Vec<u8>) -> Self::Response;
}
