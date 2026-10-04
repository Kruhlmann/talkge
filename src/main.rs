use std::str::FromStr;

use talkge::{
    core::{extract_config_from_environment, setup_logging},
    llm::{LlamaClient, TtsClient, VoiceHttpClient},
    twitch::{TwitchCredentialsSqliteStore, TwitchHttpClient},
    web::{AppError, CanvasAccessSqliteStore, Server},
};

#[tokio::main]
async fn main() {
    setup_logging();
    run()
        .await
        .inspect_err(|error| tracing::error!(?error, "unrecoverable error"))
        .unwrap();
}

async fn run() -> Result<(), AppError> {
    let config = extract_config_from_environment()
        .inspect_err(|error| tracing::error!(?error, "failed to parse environment configuration"))
        .unwrap();
    let talkge_connect_str = format!("sqlite:{}", config.talkge_db_location);
    let talkge_connect_opt =
        sqlx::sqlite::SqliteConnectOptions::from_str(&talkge_connect_str)?.create_if_missing(true);
    let talkge_pool = sqlx::sqlite::SqlitePool::connect_with(talkge_connect_opt).await?;
    let canvas_access = CanvasAccessSqliteStore::new(talkge_pool.clone()).await?;
    let twitch_auth = TwitchCredentialsSqliteStore::new(talkge_pool).await?;
    let twitch_api = TwitchHttpClient::new(
        config.twitch_client_id,
        config.twitch_client_secret,
        config.twitch_redirect_url,
    );
    let llama = LlamaClient::new(config.llama_url, config.llama_model);
    let tts = TtsClient::new(config.tts_url, config.tts_voice);
    let voice = VoiceHttpClient::new(llama, tts);

    let session_connect_str = format!("sqlite:{}", config.sessions_db_location);
    let session_connect_opt =
        tower_sessions_sqlx_store::sqlx::sqlite::SqliteConnectOptions::from_str(
            &session_connect_str,
        )?
        .create_if_missing(true);
    let session_pool =
        tower_sessions_sqlx_store::sqlx::SqlitePool::connect_with(session_connect_opt).await?;
    let session_store = tower_sessions_sqlx_store::SqliteStore::new(session_pool);
    session_store.migrate().await?;

    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(address).await?;
    Server::new(
        config.public_url,
        canvas_access,
        twitch_auth,
        twitch_api,
        voice,
        session_store,
    )
    .await
    .unwrap()
    .run(listener)
    .await
    .unwrap();
    Ok(())
}
