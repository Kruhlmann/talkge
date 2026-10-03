use talkge::{
    core::{extract_config_from_environment, setup_logging},
    web::Server,
};

#[tokio::main]
async fn main() {
    setup_logging();
    let config = extract_config_from_environment()
        .inspect_err(|error| tracing::error!(?error, "failed to parse environment configuration"))
        .unwrap();
    Server::new(config).await.unwrap().run().await.unwrap();
}
