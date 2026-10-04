use std::sync::Arc;

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{Html, Redirect},
    routing::{get, post},
    serve::{Listener, Serve},
};
use tower_http::services::ServeDir;
use tower_sessions::{SessionManagerLayer, SessionStore, cookie::SameSite};

use crate::{
    core::SecureString,
    llm::VoiceClient,
    twitch::{TwitchClient, TwitchCredentialsStore, TwitchOAuthCallback},
    web::{
        AppError, AppSession, CanvasAccessStore, CanvasWebSocketClient, SessionUser,
        templates::{CanvasTemplate, IndexTemplate},
    },
};

pub struct ServerState<C, T, A, V> {
    public_url: String,
    voice: Arc<V>,
    canvas_access: C,
    twitch_api: A,
    twitch_auth: T,
}

pub struct Server {
    router: Router,
}

impl Server {
    pub async fn new<C, T, A, V, S>(
        public_url: String,
        canvas_access: C,
        twitch_auth: T,
        twitch_api: A,
        voice: V,
        session_store: S,
    ) -> Result<Self, AppError>
    where
        C: CanvasAccessStore + 'static,
        T: TwitchCredentialsStore + 'static,
        A: TwitchClient + 'static,
        V: VoiceClient + 'static,
        S: SessionStore + Clone,
    {
        let session_layer = SessionManagerLayer::new(session_store)
            .with_secure(!cfg!(debug_assertions))
            .with_same_site(SameSite::Lax)
            .with_name("talkge-session");

        let state = Arc::new(ServerState {
            twitch_api,
            twitch_auth,
            voice: Arc::new(voice),
            canvas_access,
            public_url,
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

        Ok(Self { router })
    }

    pub async fn run<T: Listener>(self, listener: T) -> Result<(), AppError>
    where
        Serve<T, Router, Router>: std::future::IntoFuture<Output = std::io::Result<()>>,
    {
        tracing::info!("server running");
        axum::serve(listener, self.router).await?;
        Ok(())
    }

    async fn root<C, T, A, V>(
        session: AppSession,
        State(state): State<Arc<ServerState<C, T, A, V>>>,
    ) -> Result<Html<String>, AppError>
    where
        C: CanvasAccessStore,
    {
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

    async fn login<C, T, A, V>(
        session: AppSession,
        State(state): State<Arc<ServerState<C, T, A, V>>>,
    ) -> Result<Redirect, AppError>
    where
        A: TwitchClient,
    {
        if session.is_logged_in().await? {
            return Ok(Redirect::to("/"));
        }
        let SecureString(oauth_state) = SecureString::new(32);
        session.set_oauth_state(&oauth_state).await?;
        let url = state.twitch_api.authorization_url(&oauth_state);
        Ok(Redirect::temporary(&url))
    }

    async fn canvas_ws<C, T, A, V>(
        Path(token): Path<String>,
        State(state): State<Arc<ServerState<C, T, A, V>>>,
        ws: WebSocketUpgrade,
    ) -> Result<axum::response::Response, AppError>
    where
        C: CanvasAccessStore + 'static,
        T: TwitchCredentialsStore + 'static,
        A: TwitchClient + 'static,
        V: VoiceClient + 'static,
    {
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

    async fn canvas<C, T, A, V>(
        Path(token): Path<String>,
        State(state): State<Arc<ServerState<C, T, A, V>>>,
    ) -> Result<Html<String>, AppError>
    where
        C: CanvasAccessStore,
    {
        if !state.canvas_access.validate(&token).await? {
            return Err(AppError::Unauthorized);
        }
        let template = CanvasTemplate { token };
        Ok(Html(template.render()?))
    }

    async fn regenerate_canvas_access<C, T, A, V>(
        session: AppSession,
        State(state): State<Arc<ServerState<C, T, A, V>>>,
    ) -> Result<Redirect, AppError>
    where
        C: CanvasAccessStore,
    {
        let user = session.user().await?.ok_or(AppError::Unauthorized)?;
        state.canvas_access.regenerate(&user.id).await?;
        Ok(Redirect::to("/"))
    }

    async fn twitch_oauth_callback<C, T, A, V>(
        session: AppSession,
        State(state): State<Arc<ServerState<C, T, A, V>>>,
        Query(callback): Query<TwitchOAuthCallback>,
    ) -> Result<Redirect, AppError>
    where
        T: TwitchCredentialsStore,
        A: TwitchClient,
    {
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
