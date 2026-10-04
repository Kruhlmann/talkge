use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::{Row, SqlitePool};

use crate::{
    twitch::{TwitchTokenResponse, TwitchUser},
    web::AppError,
};

pub struct TwitchAuthentication {
    pub user: TwitchUser,
    pub token: TwitchTokenResponse,
}

#[derive(Debug, Clone)]
pub struct TwitchCredentials {
    pub user_id: String,
    pub login: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

#[async_trait::async_trait]
pub trait TwitchCredentialsStore: Clone + Send + Sync {
    async fn set(
        &self,
        user_id: &str,
        login: &str,
        access_token: &str,
        refresh_token: &str,
        expires_in: u64,
    ) -> Result<(), AppError>;
    async fn get(&self, user_id: &str) -> Result<Option<TwitchCredentials>, AppError>;
    async fn delete(&self, user_id: &str) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct TwitchCredentialsSqliteStore {
    pool: SqlitePool,
}

impl TwitchCredentialsSqliteStore {
    pub async fn new(pool: SqlitePool) -> Result<Self, AppError> {
        Self::migrate_db(&pool).await?;
        Ok(Self { pool })
    }

    async fn migrate_db(pool: &SqlitePool) -> Result<(), AppError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS twitch_credentials (
                user_id TEXT PRIMARY KEY NOT NULL,
                login TEXT NOT NULL,
                access_token TEXT NOT NULL,
                refresh_token TEXT NOT NULL,
                expires_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl TwitchCredentialsStore for TwitchCredentialsSqliteStore {
    async fn set(
        &self,
        user_id: &str,
        login: &str,
        access_token: &str,
        refresh_token: &str,
        expires_in: u64,
    ) -> Result<(), AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let expires_at = now + expires_in as i64;

        sqlx::query(
            r#"
            INSERT INTO twitch_credentials (
                user_id,
                login,
                access_token,
                refresh_token,
                expires_at
            )
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(user_id)
            DO UPDATE SET
                login = excluded.login,
                access_token = excluded.access_token,
                refresh_token = excluded.refresh_token,
                expires_at = excluded.expires_at
        "#,
        )
        .bind(user_id)
        .bind(login)
        .bind(access_token)
        .bind(refresh_token)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, user_id: &str) -> Result<Option<TwitchCredentials>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT
                user_id,
                login,
                access_token,
                refresh_token,
                expires_at
            FROM twitch_credentials
            WHERE user_id = ?
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| TwitchCredentials {
            user_id: row.get("user_id"),
            login: row.get("login"),
            access_token: row.get("access_token"),
            refresh_token: row.get("refresh_token"),
            expires_at: row.get("expires_at"),
        }))
    }

    async fn delete(&self, user_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM twitch_credentials WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
