use axum::{extract::FromRequestParts, http::request::Parts};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::web::AppError;

const USER_KEY: &str = "user";
const OAUTH_STATE_KEY: &str = "oauth_state";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: String,
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct AppSession {
    session: Session,
}

impl AppSession {
    pub async fn user(&self) -> Result<Option<SessionUser>, AppError> {
        Ok(self.session.get(USER_KEY).await?)
    }

    pub async fn set_user(&self, user: SessionUser) -> Result<(), AppError> {
        self.session.insert(USER_KEY, user).await?;
        Ok(())
    }

    pub async fn is_logged_in(&self) -> Result<bool, AppError> {
        Ok(self.user().await?.is_some())
    }

    pub async fn oauth_state(&self) -> Result<Option<String>, AppError> {
        Ok(self.session.get(OAUTH_STATE_KEY).await?)
    }

    pub async fn set_oauth_state(&self, state: impl Into<String>) -> Result<(), AppError> {
        self.session.insert(OAUTH_STATE_KEY, state.into()).await?;

        Ok(())
    }

    pub async fn clear_oauth_state(&self) -> Result<(), AppError> {
        self.session.remove::<String>(OAUTH_STATE_KEY).await?;

        Ok(())
    }

    pub async fn verify_oauth_state(&self, actual: &str) -> Result<bool, AppError> {
        let expected = self.oauth_state().await?;
        Ok(expected.as_deref() == Some(actual))
    }

    pub async fn logout(&self) -> Result<(), AppError> {
        self.session.delete().await?;
        Ok(())
    }

    pub fn inner(&self) -> &Session {
        &self.session
    }
}

impl<S> FromRequestParts<S> for AppSession
where
    S: Send + Sync,
{
    type Rejection = <Session as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await?;

        Ok(Self { session })
    }
}
