//! API tokens (design §10). Only SHA-256 hashes of secrets are stored.

use domain::{TokenId, UserId};
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

/// How a token came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A browser signed in with a passkey.
    Session,
    /// `exchequer login`.
    Cli,
    /// Made by hand, for a script.
    Script,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Cli => "cli",
            Self::Script => "script",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "session" => Self::Session,
            "cli" => Self::Cli,
            _ => Self::Script,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenRow {
    pub id: TokenId,
    pub name: String,
    pub kind: Kind,
    pub secret_hash: String,
    pub created_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
    /// Who it belongs to; `None` only for old tokens, all revoked.
    pub user_id: Option<UserId>,
    pub expires_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
}

impl TokenRow {
    /// Neither revoked nor expired.
    #[must_use]
    pub fn active(&self, now: OffsetDateTime) -> bool {
        self.revoked_at.is_none() && self.expires_at > now
    }
}

/// The row shape the queries return; IDs become typed below.
struct Raw {
    id: Uuid,
    name: String,
    kind: String,
    secret_hash: String,
    created_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
    user_id: Option<Uuid>,
    expires_at: OffsetDateTime,
    last_used_at: Option<OffsetDateTime>,
}

impl From<Raw> for TokenRow {
    fn from(r: Raw) -> Self {
        Self {
            id: TokenId::from_uuid(r.id),
            name: r.name,
            kind: Kind::parse(&r.kind),
            secret_hash: r.secret_hash,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
            user_id: r.user_id.map(UserId::from_uuid),
            expires_at: r.expires_at,
            last_used_at: r.last_used_at,
        }
    }
}

/// Fails with a unique violation if the name is taken.
pub async fn insert(
    db: impl PgExecutor<'_>,
    id: TokenId,
    name: &str,
    kind: Kind,
    secret_hash: &str,
    user: UserId,
    expires_at: OffsetDateTime,
) -> Result<TokenRow, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        "INSERT INTO api_token (id, name, kind, secret_hash, user_id, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                   last_used_at",
        id.as_uuid(),
        name,
        kind.as_str(),
        secret_hash,
        user.as_uuid(),
        expires_at,
    )
    .fetch_one(db)
    .await
    .map(TokenRow::from)
}

pub async fn find(db: impl PgExecutor<'_>, id: TokenId) -> Result<Option<TokenRow>, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        "SELECT id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                last_used_at
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
        "SELECT id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                last_used_at
         FROM api_token ORDER BY id"
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(TokenRow::from).collect())
}

/// Notes that the token was just used. To the minute: a busy client writes
/// once a minute, not once a request.
pub async fn used(db: impl PgExecutor<'_>, id: TokenId) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE api_token SET last_used_at = now()
         WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')",
        id.as_uuid(),
    )
    .execute(db)
    .await?;
    Ok(())
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
           RETURNING old.id, old.name, old.kind, old.secret_hash, old.created_at,
                     old.revoked_at, old.user_id, old.expires_at, old.last_used_at"#,
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(TokenRow::from))
}

/// Revokes every token still working, everyone's; returns them as revoked.
pub async fn revoke_all(db: impl PgExecutor<'_>) -> Result<Vec<TokenRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Raw,
        "UPDATE api_token SET revoked_at = now()
         WHERE revoked_at IS NULL AND expires_at > now()
         RETURNING id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                   last_used_at"
    )
    .fetch_all(db)
    .await?;
    let mut rows: Vec<_> = rows.into_iter().map(TokenRow::from).collect();
    rows.sort_by_key(|r| r.id);
    Ok(rows)
}

/// Revokes every token of the user still working; returns them as revoked.
pub async fn revoke_all_of(
    db: impl PgExecutor<'_>,
    user: UserId,
) -> Result<Vec<TokenRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Raw,
        "UPDATE api_token SET revoked_at = now()
         WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
         RETURNING id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                   last_used_at",
        user.as_uuid(),
    )
    .fetch_all(db)
    .await?;
    let mut rows: Vec<_> = rows.into_iter().map(TokenRow::from).collect();
    rows.sort_by_key(|r| r.id);
    Ok(rows)
}

/// How many of the user's tokens still work.
pub async fn working_of(db: impl PgExecutor<'_>, user: UserId) -> Result<i64, sqlx::Error> {
    let n = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM api_token
           WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()"#,
        user.as_uuid(),
    )
    .fetch_one(db)
    .await?;
    Ok(n)
}
