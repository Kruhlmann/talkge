use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;

use crate::{core::SecureString, web::AppError};

#[async_trait::async_trait]
pub trait CanvasAccessStore: Clone + Send + Sync {
    async fn regenerate(&self, user_id: &str) -> Result<String, AppError>;
    async fn get_token(&self, user_id: &str) -> Result<Option<String>, AppError>;
    async fn user_id(&self, token: &str) -> Result<Option<String>, AppError>;
    async fn validate(&self, token: &str) -> Result<bool, AppError>;
}

#[derive(Clone)]
pub struct CanvasAccessSqliteStore {
    pool: SqlitePool,
}

impl CanvasAccessSqliteStore {
    pub async fn new(pool: SqlitePool) -> Result<Self, AppError> {
        Self::migrate_db(&pool).await?;
        Ok(Self { pool })
    }

    async fn migrate_db(pool: &SqlitePool) -> Result<(), AppError> {
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS canvas_access (
                user_id TEXT PRIMARY KEY NOT NULL,
                token TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )"#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl CanvasAccessStore for CanvasAccessSqliteStore {
    async fn regenerate(&self, user_id: &str) -> Result<String, AppError> {
        let SecureString(token) = SecureString::new(48);
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        sqlx::query(
            r#"
            INSERT INTO canvas_access (user_id, token, created_at) VALUES (?, ?, ?)
            ON CONFLICT(user_id)
            DO UPDATE SET
                token = excluded.token,
                created_at = excluded.created_at
            "#,
        )
        .bind(user_id)
        .bind(&token)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        Ok(token)
    }

    async fn get_token(&self, user_id: &str) -> Result<Option<String>, AppError> {
        let token = sqlx::query_scalar(r#"SELECT token FROM canvas_access WHERE user_id = ? "#)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(token)
    }

    async fn user_id(&self, token: &str) -> Result<Option<String>, AppError> {
        let user_id = sqlx::query_scalar(r#"SELECT user_id FROM canvas_access WHERE token = ?"#)
            .bind(token)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user_id)
    }

    async fn validate(&self, token: &str) -> Result<bool, AppError> {
        let exists: Option<i64> =
            sqlx::query_scalar(r#"SELECT 1 FROM canvas_access WHERE token = ? LIMIT 1"#)
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;
        Ok(exists.is_some())
    }
}
