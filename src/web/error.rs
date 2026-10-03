use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    ExternalRequest(#[from] reqwest::Error),
    #[error(transparent)]
    Session(#[from] tower_sessions::session::Error),
    #[error(transparent)]
    AxumHttp(#[from] axum::http::Error),
    #[error(transparent)]
    Template(#[from] askama::Error),
    #[error(transparent)]
    SessionSqlx(#[from] tower_sessions_sqlx_store::sqlx::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Ws(#[from] axum::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid oauth state")]
    InvalidOAuthState,
    #[error(transparent)]
    Irc(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("oauth error: {0}")]
    OAuth(String),
    #[error("{0}")]
    Other(String),
    #[error("unauthorized")]
    Unauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!(error = ?self, "request failed");
        let e = match self {
            AppError::InvalidOAuthState => (StatusCode::BAD_REQUEST, "Invalid OAuth state"),
            AppError::ExternalRequest(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "Service unavailable")
            }
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AppError::OAuth(_) => (StatusCode::BAD_GATEWAY, "OAuth authentication failed"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        e.into_response()
    }
}
