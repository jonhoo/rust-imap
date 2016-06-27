pub trait Authenticator {
    fn process(&self, String) -> String;
}
