use rand::{RngExt, distr::Alphanumeric};

#[derive(Debug, Clone)]
pub struct SecureString(pub String);

impl SecureString {
    pub fn new(len: usize) -> Self {
        let s = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(len)
            .map(char::from)
            .collect();
        Self(s)
    }
}
