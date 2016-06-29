/// This will allow plugable authentication mechanisms.
pub trait Authenticator {
    fn process(&self, String) -> String;
}
