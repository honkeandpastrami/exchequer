//! API tokens and the sessions using them (design §10). Only SHA-256 hashes
//! of secrets are stored.

use domain::{PasskeyId, TokenId, UserId};
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

/// How a token came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A browser signed in with a passkey.
    Browser,
    /// `exchequer login`.
    Cli,
    /// Made by hand, for a script.
    Script,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Cli => "cli",
            Self::Script => "script",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "browser" => Self::Browser,
            "cli" => Self::Cli,
            _ => Self::Script,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenRow {
    pub id: TokenId,
    /// A script's token's; the others have none.
    pub name: Option<String>,
    pub kind: Kind,
    pub secret_hash: String,
    pub created_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
    /// Who it belongs to; `None` only for old tokens, all revoked.
    pub user_id: Option<UserId>,
    pub expires_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    /// The passkey a browser signed in with, or the CLI's approver did.
    pub passkey_id: Option<PasskeyId>,
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
    name: Option<String>,
    kind: String,
    secret_hash: String,
    created_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
    user_id: Option<Uuid>,
    expires_at: OffsetDateTime,
    last_used_at: Option<OffsetDateTime>,
    passkey_id: Option<Uuid>,
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
            passkey_id: r.passkey_id.map(PasskeyId::from_uuid),
        }
    }
}

/// What a token is for: a script, by name, or a person signing in, with the
/// passkey they did it with.
#[derive(Debug, Clone, Copy)]
pub enum NewToken<'a> {
    Script(&'a str),
    Browser(Option<PasskeyId>),
    Cli(Option<PasskeyId>),
}

/// Fails with a unique violation if a script's token's name is taken.
pub async fn insert(
    db: impl PgExecutor<'_>,
    id: TokenId,
    new: NewToken<'_>,
    secret_hash: &str,
    user: UserId,
    expires_at: OffsetDateTime,
) -> Result<TokenRow, sqlx::Error> {
    let (kind, name, passkey) = match new {
        NewToken::Script(name) => (Kind::Script, Some(name), None),
        NewToken::Browser(p) => (Kind::Browser, None, p),
        NewToken::Cli(p) => (Kind::Cli, None, p),
    };
    sqlx::query_as!(
        Raw,
        "INSERT INTO api_token (id, name, kind, secret_hash, user_id, expires_at, passkey_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                   last_used_at, passkey_id",
        id.as_uuid(),
        name,
        kind.as_str(),
        secret_hash,
        user.as_uuid(),
        expires_at,
        passkey.map(|p| *p.as_uuid()),
    )
    .fetch_one(db)
    .await
    .map(TokenRow::from)
}

pub async fn find(db: impl PgExecutor<'_>, id: TokenId) -> Result<Option<TokenRow>, sqlx::Error> {
    sqlx::query_as!(
        Raw,
        "SELECT id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                last_used_at, passkey_id
         FROM api_token WHERE id = $1",
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(TokenRow::from))
}

/// Every script's token, oldest first.
pub async fn scripts(db: impl PgExecutor<'_>) -> Result<Vec<TokenRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Raw,
        "SELECT id, name, kind, secret_hash, created_at, revoked_at, user_id, expires_at,
                last_used_at, passkey_id
         FROM api_token WHERE kind = 'script' ORDER BY id"
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
                     old.revoked_at, old.user_id, old.expires_at, old.last_used_at,
                     old.passkey_id"#,
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
                   last_used_at, passkey_id"
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
                   last_used_at, passkey_id",
        user.as_uuid(),
    )
    .fetch_all(db)
    .await?;
    let mut rows: Vec<_> = rows.into_iter().map(TokenRow::from).collect();
    rows.sort_by_key(|r| r.id);
    Ok(rows)
}

/// A client as it describes itself on a request.
#[derive(Debug, Clone, Default)]
pub struct Client {
    /// Its `User-Agent`.
    pub agent: Option<String>,
    /// The host it says it runs on.
    pub host: Option<String>,
    /// The address the request came from.
    pub ip: Option<String>,
}

/// A client using a token that still works.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: Uuid,
    pub token_id: TokenId,
    pub kind: Kind,
    pub user_id: Option<UserId>,
    /// The passkey's name, or the script's token's.
    pub signed_in_with: Option<String>,
    pub started_at: OffsetDateTime,
    pub last_used_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub agent: Option<String>,
    pub host: Option<String>,
    pub ip: Option<String>,
}

struct RawSession {
    id: Uuid,
    token_id: Uuid,
    kind: String,
    user_id: Option<Uuid>,
    signed_in_with: Option<String>,
    started_at: OffsetDateTime,
    last_used_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    agent: Option<String>,
    host: Option<String>,
    ip: Option<String>,
}

impl From<RawSession> for SessionRow {
    fn from(r: RawSession) -> Self {
        Self {
            id: r.id,
            token_id: TokenId::from_uuid(r.token_id),
            kind: Kind::parse(&r.kind),
            user_id: r.user_id.map(UserId::from_uuid),
            signed_in_with: r.signed_in_with,
            started_at: r.started_at,
            last_used_at: r.last_used_at,
            expires_at: r.expires_at,
            agent: r.agent,
            host: r.host,
            ip: r.ip,
        }
    }
}

/// What tells a script's sessions apart: its tool (the `User-Agent`'s first
/// word, without its version) and host. A browser or the CLI is one session
/// per token, whatever it reports.
fn session_key(kind: Kind, client: &Client) -> String {
    if kind != Kind::Script {
        return String::new();
    }
    let tool = client
        .agent
        .as_deref()
        .and_then(|a| a.split_whitespace().next())
        .map(|w| w.split('/').next().unwrap_or(w).to_lowercase());
    format!(
        "{}@{}",
        tool.unwrap_or_default(),
        client.host.as_deref().unwrap_or_default()
    )
}

/// Notes that the client used the token, and returns its session's ID: a
/// new one for a client not seen before. To the minute, as `used`, unless
/// the client looks different.
pub async fn session_used(
    db: impl PgExecutor<'_>,
    token: TokenId,
    kind: Kind,
    client: &Client,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar!(
        r#"WITH up AS (
               INSERT INTO token_session (id, token_id, key, agent, host, ip)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (token_id, key) DO UPDATE
                   SET agent = excluded.agent, host = excluded.host, ip = excluded.ip,
                       last_used_at = now()
                   WHERE token_session.last_used_at < now() - interval '1 minute'
                      OR (token_session.agent, token_session.host, token_session.ip)
                         IS DISTINCT FROM (excluded.agent, excluded.host, excluded.ip)
               RETURNING id
           )
           SELECT id AS "id!" FROM up
           UNION ALL
           SELECT id FROM token_session WHERE token_id = $2 AND key = $3
           LIMIT 1"#,
        Uuid::now_v7(),
        token.as_uuid(),
        session_key(kind, client),
        client.agent,
        client.host,
        client.ip,
    )
    .fetch_one(db)
    .await
}

/// The sessions whose tokens still work, last used first.
pub async fn sessions(db: impl PgExecutor<'_>) -> Result<Vec<SessionRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        RawSession,
        r#"SELECT s.id, s.token_id, t.kind, t.user_id,
                  CASE WHEN t.kind = 'script' THEN t.name ELSE p.name END AS signed_in_with,
                  CASE WHEN t.kind = 'script' THEN s.started_at ELSE t.created_at END
                      AS "started_at!",
                  s.last_used_at, t.expires_at, s.agent, s.host, s.ip
           FROM token_session s
           JOIN api_token t ON t.id = s.token_id
           LEFT JOIN passkey p ON p.id = t.passkey_id
           WHERE t.revoked_at IS NULL AND t.expires_at > now()
           ORDER BY s.last_used_at DESC, s.id"#
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(SessionRow::from).collect())
}

/// A session whose token still works.
pub async fn session(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<SessionRow>, sqlx::Error> {
    sqlx::query_as!(
        RawSession,
        r#"SELECT s.id, s.token_id, t.kind, t.user_id,
                  CASE WHEN t.kind = 'script' THEN t.name ELSE p.name END AS signed_in_with,
                  CASE WHEN t.kind = 'script' THEN s.started_at ELSE t.created_at END
                      AS "started_at!",
                  s.last_used_at, t.expires_at, s.agent, s.host, s.ip
           FROM token_session s
           JOIN api_token t ON t.id = s.token_id
           LEFT JOIN passkey p ON p.id = t.passkey_id
           WHERE s.id = $1 AND t.revoked_at IS NULL AND t.expires_at > now()"#,
        id,
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(SessionRow::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scripts_sessions_are_its_tools_and_hosts() {
        let client = |agent: &str, host: &str| Client {
            agent: Some(agent.into()),
            host: Some(host.into()),
            ip: None,
        };
        assert_eq!(
            session_key(Kind::Script, &client("Backup/1.2 python-httpx/0.28", "nas")),
            "backup@nas"
        );
        assert_eq!(
            session_key(Kind::Script, &client("backup/1.3", "nas")),
            session_key(Kind::Script, &client("Backup/1.2", "nas")),
            "a new version is the same session"
        );
        assert_eq!(session_key(Kind::Script, &Client::default()), "@");
        assert_eq!(
            session_key(Kind::Cli, &client("exchequer-cli/1.0", "laptop")),
            ""
        );
    }
}
