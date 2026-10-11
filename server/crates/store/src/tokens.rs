//! API tokens (design §10 tier 1). Only argon2 hashes of secrets are stored.

use domain::TokenId;
use sqlx::{PgExecutor, Row};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct TokenRow {
    pub id: TokenId,
    pub name: String,
    pub secret_hash: String,
    pub created_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
}

fn row(r: &sqlx::postgres::PgRow) -> Result<TokenRow, sqlx::Error> {
    Ok(TokenRow {
        id: TokenId::from_uuid(r.try_get("id")?),
        name: r.try_get("name")?,
        secret_hash: r.try_get("secret_hash")?,
        created_at: r.try_get("created_at")?,
        revoked_at: r.try_get("revoked_at")?,
    })
}

/// Fails with a unique violation if the name is taken.
pub async fn insert(
    db: impl PgExecutor<'_>,
    id: TokenId,
    name: &str,
    secret_hash: &str,
) -> Result<TokenRow, sqlx::Error> {
    let r = sqlx::query(
        "INSERT INTO api_token (id, name, secret_hash) VALUES ($1, $2, $3)
         RETURNING id, name, secret_hash, created_at, revoked_at",
    )
    .bind(id.as_uuid())
    .bind(name)
    .bind(secret_hash)
    .fetch_one(db)
    .await?;
    row(&r)
}

pub async fn find(db: impl PgExecutor<'_>, id: TokenId) -> Result<Option<TokenRow>, sqlx::Error> {
    sqlx::query("SELECT id, name, secret_hash, created_at, revoked_at FROM api_token WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_optional(db)
        .await?
        .as_ref()
        .map(row)
        .transpose()
}

/// Every token, oldest first.
pub async fn list(db: impl PgExecutor<'_>) -> Result<Vec<TokenRow>, sqlx::Error> {
    sqlx::query("SELECT id, name, secret_hash, created_at, revoked_at FROM api_token ORDER BY id")
        .fetch_all(db)
        .await?
        .iter()
        .map(row)
        .collect()
}

/// Marks the token revoked; revoking twice keeps the first time. Returns the
/// row as it was before, or `None` if there's no such token.
pub async fn revoke(db: impl PgExecutor<'_>, id: TokenId) -> Result<Option<TokenRow>, sqlx::Error> {
    sqlx::query(
        "UPDATE api_token new SET revoked_at = coalesce(new.revoked_at, now())
         FROM api_token old WHERE new.id = old.id AND new.id = $1
         RETURNING old.id, old.name, old.secret_hash, old.created_at, old.revoked_at",
    )
    .bind(id.as_uuid())
    .fetch_optional(db)
    .await?
    .as_ref()
    .map(row)
    .transpose()
}
