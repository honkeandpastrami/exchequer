//! API tokens (design §10 tier 1). Only SHA-256 hashes of secrets are stored.

use domain::TokenId;
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
}

/// The row shape the queries return; IDs become typed below.
struct Raw {
    id: Uuid,
    name: String,
    secret_hash: String,
    created_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
}

impl From<Raw> for TokenRow {
    fn from(r: Raw) -> Self {
        Self {
            id: TokenId::from_uuid(r.id),
            name: r.name,
            secret_hash: r.secret_hash,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
        }
    }
}

/// Fails with a unique violation if the name is taken.
pub async fn insert(
    db: impl PgExecutor<'_>,
    id: TokenId,
    name: &str,
    secret_hash: &str,
) -> Result<TokenRow, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        "INSERT INTO api_token (id, name, secret_hash) VALUES ($1, $2, $3)
         RETURNING id, name, secret_hash, created_at, revoked_at",
        id.as_uuid(),
        name,
        secret_hash,
    )
    .fetch_one(db)
    .await
    .map(TokenRow::from)
}

pub async fn find(db: impl PgExecutor<'_>, id: TokenId) -> Result<Option<TokenRow>, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        "SELECT id, name, secret_hash, created_at, revoked_at FROM api_token WHERE id = $1",
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
        "SELECT id, name, secret_hash, created_at, revoked_at FROM api_token ORDER BY id"
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
           RETURNING old.id, old.name, old.secret_hash, old.created_at, old.revoked_at"#,
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(TokenRow::from))
}
