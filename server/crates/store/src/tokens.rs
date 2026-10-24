//! API tokens (design §10). Only SHA-256 hashes of secrets are stored.

use domain::{TokenId, UserId};
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TokenRow {
    pub id: TokenId,
    pub name: String,
    pub secret_hash: String,
    pub created_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
    /// Who it belongs to; `None` for a script's token.
    pub user_id: Option<UserId>,
    /// Sessions expire; other tokens last until revoked.
    pub expires_at: Option<OffsetDateTime>,
}

impl TokenRow {
    /// Neither revoked nor expired.
    #[must_use]
    pub fn active(&self, now: OffsetDateTime) -> bool {
        self.revoked_at.is_none() && self.expires_at.is_none_or(|e| e > now)
    }
}

/// The row shape the queries return; IDs become typed below.
struct Raw {
    id: Uuid,
    name: String,
    secret_hash: String,
    created_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
    user_id: Option<Uuid>,
    expires_at: Option<OffsetDateTime>,
}

impl From<Raw> for TokenRow {
    fn from(r: Raw) -> Self {
        Self {
            id: TokenId::from_uuid(r.id),
            name: r.name,
            secret_hash: r.secret_hash,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
            user_id: r.user_id.map(UserId::from_uuid),
            expires_at: r.expires_at,
        }
    }
}

/// Fails with a unique violation if the name is taken.
pub async fn insert(
    db: impl PgExecutor<'_>,
    id: TokenId,
    name: &str,
    secret_hash: &str,
    user: Option<UserId>,
    expires_at: Option<OffsetDateTime>,
) -> Result<TokenRow, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        "INSERT INTO api_token (id, name, secret_hash, user_id, expires_at)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, secret_hash, created_at, revoked_at, user_id, expires_at",
        id.as_uuid(),
        name,
        secret_hash,
        user.map(|u| *u.as_uuid()),
        expires_at,
    )
    .fetch_one(db)
    .await
    .map(TokenRow::from)
}

pub async fn find(db: impl PgExecutor<'_>, id: TokenId) -> Result<Option<TokenRow>, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        "SELECT id, name, secret_hash, created_at, revoked_at, user_id, expires_at
         FROM api_token WHERE id = $1",
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(TokenRow::from))
}

/// Every token, oldest first.
pub async fn list(db: impl PgExecutor<'_>) -> Result<Vec<TokenRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Raw,
        "SELECT id, name, secret_hash, created_at, revoked_at, user_id, expires_at
         FROM api_token ORDER BY id"
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(TokenRow::from).collect())
}

/// Marks the token revoked; revoking twice keeps the first time. Returns the
/// row as it was before, or `None` if there's no such token.
pub async fn revoke(db: impl PgExecutor<'_>, id: TokenId) -> Result<Option<TokenRow>, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        // `old` is the same row before the update (Postgres 17 has no OLD in
        // RETURNING).
        r#"UPDATE api_token new SET revoked_at = coalesce(new.revoked_at, now())
           FROM api_token old WHERE new.id = old.id AND new.id = $1
           RETURNING old.id, old.name, old.secret_hash, old.created_at, old.revoked_at,
                     old.user_id, old.expires_at"#,
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(TokenRow::from))
}

/// Revokes every token of the user; returns how many were still active.
pub async fn revoke_all_of(db: impl PgExecutor<'_>, user: UserId) -> Result<u64, sqlx::Error> {
    let done = sqlx::query!(
        "UPDATE api_token SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        user.as_uuid(),
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}
