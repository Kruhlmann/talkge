use crate::core::SecretString;

pub struct TalkgeConfiguration {
    pub host: String,
    pub port: u16,
    pub sessions_db_location: String,
    pub talkge_db_location: String,
    pub twitch_client_id: SecretString,
    pub twitch_client_secret: SecretString,
    pub twitch_redirect_url: String,
    pub llama_url: String,
    pub llama_model: String,
    pub tts_url: String,
    pub tts_voice: String,
    pub public_url: String,
}
