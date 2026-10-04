use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{Html, Redirect},
    routing::{get, post},
};
use tower_http::services::ServeDir;
use tower_sessions::{SessionManagerLayer, cookie::SameSite};

use crate::{
    core::{SecureString, TalkgeConfiguration},
    llm::{LlamaClient, TtsClient, VoiceClient},
    twitch::{TwitchClient, TwitchCredentialsStore, TwitchOAuthCallback},
    web::{
        AppError, AppSession, AsyncCanvasAccess, CanvasAccessStore, CanvasWebSocketClient,
        SessionUser,
        templates::{CanvasTemplate, IndexTemplate},
    },
};

pub struct ServerState {
    voice: Arc<VoiceClient>,
    canvas_access: CanvasAccessStore,
    public_url: String,
    twitch_api: TwitchClient,
    twitch_auth: TwitchCredentialsStore,
}

pub struct Server {
    router: Router,
    address: String,
}

impl Server {
    pub async fn new(configuration: TalkgeConfiguration) -> Result<Self, AppError> {
        let session_connect_str = format!("sqlite:{}", configuration.sessions_db_location);
        let session_connect_opt =
            tower_sessions_sqlx_store::sqlx::sqlite::SqliteConnectOptions::from_str(
                &session_connect_str,
            )?
            .create_if_missing(true);
        let session_pool =
            tower_sessions_sqlx_store::sqlx::SqlitePool::connect_with(session_connect_opt).await?;
        let session_store = tower_sessions_sqlx_store::SqliteStore::new(session_pool);
        session_store.migrate().await?;

        let session_layer = SessionManagerLayer::new(session_store)
            .with_secure(!cfg!(debug_assertions))
            .with_same_site(SameSite::Lax)
            .with_name("talkge-session");
        let talkge_connect_str = format!("sqlite:{}", configuration.talkge_db_location);
        let talkge_connect_opt = sqlx::sqlite::SqliteConnectOptions::from_str(&talkge_connect_str)?
            .create_if_missing(true);
        let talkge_pool = sqlx::sqlite::SqlitePool::connect_with(talkge_connect_opt).await?;
        let canvas_access = CanvasAccessStore::new(talkge_pool.clone());
        canvas_access.migrate().await?;

        let twitch_auth = TwitchCredentialsStore::new(talkge_pool.clone());
        twitch_auth.migrate().await?;

        let twitch_api = TwitchClient::new(
            configuration.twitch_client_id,
            configuration.twitch_client_secret,
            configuration.twitch_redirect_url,
        );
        let llama = LlamaClient::new(configuration.llama_url, configuration.llama_model);
        let tts = TtsClient::new(configuration.tts_url, configuration.tts_voice);
        let state = Arc::new(ServerState {
            twitch_api,
            twitch_auth,
            voice: Arc::new(VoiceClient::new(llama, tts)),
            canvas_access,
            public_url: configuration.public_url,
        });
        let router = Router::new()
            .route("/", get(Self::root))
            .route(
                "/canvas/access/regenerate",
                post(Self::regenerate_canvas_access),
            )
            .route("/canvas/{token}", get(Self::canvas))
            .route("/canvas/{token}/ws", get(Self::canvas_ws))
            .route("/auth/login", get(Self::login))
            .route("/auth/logout", get(Self::logout))
            .route("/auth/twitch/callback", get(Self::twitch_oauth_callback))
            .nest_service("/assets", ServeDir::new("public"))
            .with_state(state)
            .layer(session_layer);

        let address = format!("{}:{}", configuration.host, configuration.port,);
        Ok(Self { router, address })
    }

    pub async fn run(self) -> Result<(), AppError> {
        let listener = tokio::net::TcpListener::bind(&self.address).await?;
        tracing::info!(address = self.address, "server running");
        axum::serve(listener, self.router).await?;
        Ok(())
    }

    async fn root(
        session: AppSession,
        State(state): State<Arc<ServerState>>,
    ) -> Result<Html<String>, AppError> {
        let user = session.user().await?;
        let canvas_url = if let Some(user) = &user {
            state
                .canvas_access
                .get_token(&user.id)
                .await?
                .map(|token| format!("{}/canvas/{}", state.public_url, token))
        } else {
            None
        };
        let template = IndexTemplate::new(user, canvas_url);
        Ok(Html(template.render()?))
    }

    async fn logout(session: AppSession) -> Result<Redirect, AppError> {
        if let Some(user) = session.user().await? {
            tracing::info!(user = user.username, "logging out user");
        }
        session.logout().await?;
        Ok(Redirect::to("/"))
    }

    async fn login(
        session: AppSession,
        State(state): State<Arc<ServerState>>,
    ) -> Result<Redirect, AppError> {
        if session.is_logged_in().await? {
            return Ok(Redirect::to("/"));
        }
        let SecureString(oauth_state) = SecureString::new(32);
        session.set_oauth_state(&oauth_state).await?;
        let url = state.twitch_api.authorization_url(&oauth_state);
        Ok(Redirect::temporary(&url))
    }

    async fn canvas_ws(
        Path(token): Path<String>,
        State(state): State<Arc<ServerState>>,
        ws: WebSocketUpgrade,
    ) -> Result<axum::response::Response, AppError> {
        let user_id = state
            .canvas_access
            .user_id(&token)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let twitch_credentials = state.twitch_auth.clone();

        Ok(ws.on_upgrade(move |socket| async move {
            let client = CanvasWebSocketClient::new(
                socket,
                user_id,
                twitch_credentials,
                Arc::clone(&state.voice),
            );

            if let Err(error) = client.run().await {
                tracing::error!(?error, "canvas websocket failed");
            }
        }))
    }

    async fn canvas(
        Path(token): Path<String>,
        State(state): State<Arc<ServerState>>,
    ) -> Result<Html<String>, AppError> {
        if !state.canvas_access.validate(&token).await? {
            return Err(AppError::Unauthorized);
        }

        let template = CanvasTemplate { token };

        Ok(Html(template.render()?))
    }

    async fn regenerate_canvas_access(
        session: AppSession,
        State(state): State<Arc<ServerState>>,
    ) -> Result<Redirect, AppError> {
        let user = session.user().await?.ok_or(AppError::Unauthorized)?;
        state.canvas_access.regenerate(&user.id).await?;
        Ok(Redirect::to("/"))
    }

    async fn twitch_oauth_callback(
        session: AppSession,
        State(state): State<Arc<ServerState>>,
        Query(callback): Query<TwitchOAuthCallback>,
    ) -> Result<Redirect, AppError> {
        match callback {
            TwitchOAuthCallback::Success {
                code,
                state: oauth_state,
            } => {
                if !session.verify_oauth_state(&oauth_state).await? {
                    return Err(AppError::InvalidOAuthState);
                }

                session.clear_oauth_state().await?;

                let authentication = state.twitch_api.authenticate(&code).await?;

                state
                    .twitch_auth
                    .set(
                        &authentication.user.id,
                        &authentication.user.login,
                        &authentication.token.access_token,
                        &authentication.token.refresh_token,
                        authentication.token.expires_in,
                    )
                    .await?;

                session
                    .set_user(SessionUser {
                        id: authentication.user.id.clone(),
                        username: authentication.user.display_name.clone(),
                    })
                    .await?;

                tracing::info!(user = authentication.user.display_name, "logged in user");
            }

            TwitchOAuthCallback::Error {
                error,
                error_description,
                state,
            } => {
                tracing::error!(
                    ?error,
                    ?error_description,
                    ?state,
                    "twitch oauth callback error"
                );

                return Err(AppError::OAuth(error_description));
            }
        }

        Ok(Redirect::to("/"))
    }
}
