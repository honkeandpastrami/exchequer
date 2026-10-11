//! Stored responses for idempotency keys (design §5): a client that retries a
//! POST with the same `Idempotency-Key` gets the original response back
//! instead of creating something twice.

use domain::TokenId;
use sqlx::PgExecutor;

/// How long a key is remembered.
pub const RETENTION_HOURS: i32 = 24;

pub struct Stored {
    pub request_hash: String,
    pub status: i16,
    pub body: Option<serde_json::Value>,
}

pub async fn find(
    db: impl PgExecutor<'_>,
    token: TokenId,
    key: &str,
) -> Result<Option<Stored>, sqlx::Error> {
    sqlx::query_as!(
        Stored,
        r#"SELECT request_hash, response_status AS status, response_body AS body
           FROM idempotency_key
           WHERE token_id = $1 AND key = $2
             AND created_at > now() - make_interval(hours => $3)"#,
        token.as_uuid(),
        key,
        RETENTION_HOURS,
    )
    .fetch_optional(db)
    .await
}

/// Saves a response. If two identical requests race, the first one wins.
pub async fn save(
    db: impl PgExecutor<'_>,
    token: TokenId,
    key: &str,
    stored: &Stored,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO idempotency_key (token_id, key, request_hash, response_status, response_body)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (token_id, key) DO UPDATE
             SET request_hash = excluded.request_hash,
                 response_status = excluded.response_status,
                 response_body = excluded.response_body,
                 created_at = now()
             WHERE idempotency_key.created_at <= now() - make_interval(hours => $6)",
        token.as_uuid(),
        key,
        stored.request_hash,
        stored.status,
        stored.body,
        RETENTION_HOURS,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Deletes expired keys. Called when saving, since nothing runs on a timer.
pub async fn prune(db: impl PgExecutor<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM idempotency_key WHERE created_at <= now() - make_interval(hours => $1)",
        RETENTION_HOURS,
    )
    .execute(db)
    .await?;
    Ok(())
}
