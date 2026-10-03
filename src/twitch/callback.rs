use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TwitchOAuthCallback {
    Success {
        code: String,
        state: String,
    },
    Error {
        error: String,
        error_description: String,
        state: String,
    },
}
