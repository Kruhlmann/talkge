use serde::Deserialize;

#[derive(Deserialize)]
pub struct TwitchTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}
