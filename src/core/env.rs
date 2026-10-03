use crate::core::{SecretString, TalkgeConfiguration};

pub fn extract_config_from_environment() -> Result<TalkgeConfiguration, &'static str> {
    let host = std::env::var("SERVER_HOST")
        .map_err(|_| "missing environment variable SERVER_PORT")
        .inspect(|host| tracing::info!(?host, "server host"))?;
    let port: u16 = std::env::var("SERVER_PORT")
        .map_err(|_| "missing environment variable SERVER_PORT")?
        .parse()
        .map_err(|_| "invalid environment variable SERVER_PORT (must be u16)")
        .inspect(|port| tracing::info!(?port, "server port"))?;
    let sessions_db_location = std::env::var("SESSIONS_DB_FILE")
        .map_err(|_| "missing environment variable SESSIONS_DB_FILE")
        .inspect(|file| tracing::info!(?file, "server sessions db file"))?;
    let talkge_db_location = std::env::var("TALKGE_DB_FILE")
        .map_err(|_| "missing environment variable TALKGE_DB_FILE")
        .inspect(|file| tracing::info!(?file, "talkge db file"))?;
    let twitch_client_id = std::env::var("TWITCH_CLIENT_ID")
        .map(SecretString::new)
        .map_err(|_| "missing environment variable TWITCH_CLIENT_ID")
        .inspect(|client_id| tracing::info!(?client_id, "twitch client id"))?;
    let twitch_client_secret = std::env::var("TWITCH_CLIENT_SECRET")
        .map(SecretString::new)
        .map_err(|_| "missing environment variable TWITCH_CLIENT_SECRET")
        .inspect(|client_secret| tracing::info!(?client_secret, "twitch client secret"))?;
    let twitch_redirect_url = std::env::var("TWITCH_REDIRECT_URL")
        .map_err(|_| "missing environment variable TWITCH_REDIRECT_URL")
        .inspect(|url| tracing::info!(?url, "twitch redirect url"))?;
    let llama_url = std::env::var("LLAMA_URL")
        .map_err(|_| "missing environment variable LLAMA_URL")
        .inspect(|url| tracing::info!(?url, "llama url"))?;
    let llama_model = std::env::var("LLAMA_MODEL")
        .map_err(|_| "missing environment variable LLAMA_MODEL")
        .inspect(|model| tracing::info!(?model, "llama model"))?;
    let tts_url = std::env::var("TTS_URL")
        .map_err(|_| "missing environment variable TTS_URL")
        .inspect(|url| tracing::info!(?url, "tts url"))?;
    let tts_voice = std::env::var("TTS_VOICE")
        .map_err(|_| "missing environment variable TTS_VOICE")
        .inspect(|voice| tracing::info!(?voice, "tts voice"))?;
    let public_url = std::env::var("PUBLIC_URL")
        .map_err(|_| "missing environment variable PUBLIC_URL")
        .inspect(|url| tracing::info!(?url, "public url"))?;

    Ok(TalkgeConfiguration {
        host,
        port,
        sessions_db_location,
        talkge_db_location,
        twitch_client_id,
        twitch_client_secret,
        twitch_redirect_url,
        llama_url,
        llama_model,
        tts_url,
        tts_voice,
        public_url,
    })
}
