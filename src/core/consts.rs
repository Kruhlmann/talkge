use reqwest::Url;

lazy_static::lazy_static! {
    pub static ref TWITCH_OAUTH_URL: Url = Url::parse("https://id.twitch.tv/oauth2/authorize").unwrap();
    pub static ref TWITCH_OAUTH_TOKEN_URL: &'static str = "https://id.twitch.tv/oauth2/token";
    pub static ref TWITCH_HELIX_USERS_URL: &'static str = "https://api.twitch.tv/helix/users";
    pub static ref MAX_VOICE_QUEUE: usize = 5;
}
