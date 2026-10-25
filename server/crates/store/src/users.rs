//! Users, their passkeys and invites (design §10). A passkey is a public key;
//! an invite is stored as the SHA-256 of its secret.

use domain::{PasskeyId, UserId};
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: UserId,
    pub name: String,
    pub created_at: OffsetDateTime,
    pub removed_at: Option<OffsetDateTime>,
    /// Bumped on each rename: the ETag.
    pub version: i64,
}

struct RawUser {
    id: Uuid,
    name: String,
    created_at: OffsetDateTime,
    removed_at: Option<OffsetDateTime>,
    version: i64,
}

impl From<RawUser> for UserRow {
    fn from(r: RawUser) -> Self {
        Self {
            id: UserId::from_uuid(r.id),
            name: r.name,
            created_at: r.created_at,
            removed_at: r.removed_at,
            version: r.version,
        }
    }
}

/// Fails with a unique violation if a user there is has the name.
pub async fn insert(
    db: impl PgExecutor<'_>,
    id: UserId,
    name: &str,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as!(
        RawUser,
        "INSERT INTO app_user (id, name) VALUES ($1, $2)
         RETURNING id, name, created_at, removed_at, version",
        id.as_uuid(),
        name,
    )
    .fetch_one(db)
    .await
    .map(UserRow::from)
}

/// Removed users included.
pub async fn find(db: impl PgExecutor<'_>, id: UserId) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        RawUser,
        "SELECT id, name, created_at, removed_at, version FROM app_user WHERE id = $1",
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(UserRow::from))
}

/// The user there is by this name (not a removed one).
pub async fn find_by_name(
    db: impl PgExecutor<'_>,
    name: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        RawUser,
        "SELECT id, name, created_at, removed_at, version FROM app_user
         WHERE name = $1 AND removed_at IS NULL",
        name,
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(UserRow::from))
}

/// The users who haven't been removed, by name.
pub async fn list(db: impl PgExecutor<'_>) -> Result<Vec<UserRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        RawUser,
        "SELECT id, name, created_at, removed_at, version FROM app_user
         WHERE removed_at IS NULL ORDER BY name"
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(UserRow::from).collect())
}

/// Renames a user who hasn't been removed; `None` if there's no such user.
/// Fails with a unique violation if another user has the name.
pub async fn rename(
    db: impl PgExecutor<'_>,
    id: UserId,
    name: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        RawUser,
        "UPDATE app_user SET name = $2, version = version + 1
         WHERE id = $1 AND removed_at IS NULL
         RETURNING id, name, created_at, removed_at, version",
        id.as_uuid(),
        name,
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(UserRow::from))
}

/// Marks the user removed. Returns the row as it was, or `None` if there's no
/// such user or they were already removed.
pub async fn remove(db: impl PgExecutor<'_>, id: UserId) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        RawUser,
        r#"UPDATE app_user new SET removed_at = now()
           FROM app_user old WHERE new.id = old.id AND new.id = $1 AND new.removed_at IS NULL
           RETURNING old.id, old.name, old.created_at, old.removed_at, old.version"#,
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(UserRow::from))
}

#[derive(Debug, Clone)]
pub struct PasskeyRow {
    pub id: PasskeyId,
    pub user_id: UserId,
    pub name: String,
    /// webauthn-rs's `Passkey`, as JSON.
    pub credential: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
}

struct RawPasskey {
    id: Uuid,
    user_id: Uuid,
    name: String,
    credential: serde_json::Value,
    created_at: OffsetDateTime,
    last_used_at: Option<OffsetDateTime>,
}

impl From<RawPasskey> for PasskeyRow {
    fn from(r: RawPasskey) -> Self {
        Self {
            id: PasskeyId::from_uuid(r.id),
            user_id: UserId::from_uuid(r.user_id),
            name: r.name,
            credential: r.credential,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
        }
    }
}

/// Fails with a unique violation if the credential is already registered.
pub async fn insert_passkey(
    db: impl PgExecutor<'_>,
    id: PasskeyId,
    user: UserId,
    credential_id: &[u8],
    name: &str,
    credential: &serde_json::Value,
) -> Result<PasskeyRow, sqlx::Error> {
    sqlx::query_as!(
        RawPasskey,
        "INSERT INTO passkey (id, user_id, credential_id, name, credential)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, user_id, name, credential, created_at, last_used_at",
        id.as_uuid(),
        user.as_uuid(),
        credential_id,
        name,
        credential,
    )
    .fetch_one(db)
    .await
    .map(PasskeyRow::from)
}

/// Every passkey of users who haven't been removed, oldest first.
pub async fn passkeys(db: impl PgExecutor<'_>) -> Result<Vec<PasskeyRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        RawPasskey,
        "SELECT p.id, p.user_id, p.name, p.credential, p.created_at, p.last_used_at
         FROM passkey p JOIN app_user u ON u.id = p.user_id
         WHERE u.removed_at IS NULL ORDER BY p.id"
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(PasskeyRow::from).collect())
}

/// One user's credential IDs: what a new registration must not repeat.
pub async fn credential_ids(
    db: impl PgExecutor<'_>,
    user: UserId,
) -> Result<Vec<Vec<u8>>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT credential_id FROM passkey WHERE user_id = $1 ORDER BY id",
        user.as_uuid()
    )
    .fetch_all(db)
    .await
}

/// The passkey with this credential ID, if its user hasn't been removed. Locked
/// until the transaction ends, so two sign-ins can't race on its counter.
pub async fn passkey_by_credential(
    db: impl PgExecutor<'_>,
    credential_id: &[u8],
) -> Result<Option<PasskeyRow>, sqlx::Error> {
    sqlx::query_as!(
        RawPasskey,
        "SELECT p.id, p.user_id, p.name, p.credential, p.created_at, p.last_used_at
         FROM passkey p JOIN app_user u ON u.id = p.user_id
         WHERE p.credential_id = $1 AND u.removed_at IS NULL
         FOR UPDATE OF p",
        credential_id,
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(PasskeyRow::from))
}

/// Records a sign-in with the passkey, and its updated counter.
pub async fn passkey_used(
    db: impl PgExecutor<'_>,
    id: PasskeyId,
    credential: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE passkey SET credential = $2, last_used_at = now() WHERE id = $1",
        id.as_uuid(),
        credential,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Returns the deleted row, or `None` if there's no such passkey.
pub async fn delete_passkey(
    db: impl PgExecutor<'_>,
    id: PasskeyId,
) -> Result<Option<PasskeyRow>, sqlx::Error> {
    sqlx::query_as!(
        RawPasskey,
        "DELETE FROM passkey WHERE id = $1
         RETURNING id, user_id, name, credential, created_at, last_used_at",
        id.as_uuid(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(PasskeyRow::from))
}

/// Deletes the user's passkeys, but `keep`; returns the deleted ones.
pub async fn delete_passkeys_of(
    db: impl PgExecutor<'_>,
    user: UserId,
    keep: Option<PasskeyId>,
) -> Result<Vec<PasskeyRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        RawPasskey,
        "DELETE FROM passkey WHERE user_id = $1 AND id IS DISTINCT FROM $2
         RETURNING id, user_id, name, credential, created_at, last_used_at",
        user.as_uuid(),
        keep.map(|k| *k.as_uuid()),
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(PasskeyRow::from).collect())
}

#[derive(Debug, Clone)]
pub struct InviteRow {
    pub id: Uuid,
    pub user_id: UserId,
    pub secret_hash: String,
    pub expires_at: OffsetDateTime,
    /// Using it removes the user's other passkeys and revokes their tokens.
    pub replace: bool,
}

pub async fn insert_invite(
    db: impl PgExecutor<'_>,
    id: Uuid,
    user: UserId,
    secret_hash: &str,
    expires_at: OffsetDateTime,
    replace: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO invite (id, user_id, secret_hash, expires_at, replace)
         VALUES ($1, $2, $3, $4, $5)",
        id,
        user.as_uuid(),
        secret_hash,
        expires_at,
        replace,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The invite, if it's unused, unexpired and its user hasn't been removed.
pub async fn open_invite(
    db: impl PgExecutor<'_>,
    id: Uuid,
) -> Result<Option<InviteRow>, sqlx::Error> {
    sqlx::query!(
        "SELECT i.id, i.user_id, i.secret_hash, i.expires_at, i.replace
         FROM invite i JOIN app_user u ON u.id = i.user_id
         WHERE i.id = $1 AND i.used_at IS NULL AND i.expires_at > now()
           AND u.removed_at IS NULL",
        id,
    )
    .fetch_optional(db)
    .await
    .map(|r| {
        r.map(|r| InviteRow {
            id: r.id,
            user_id: UserId::from_uuid(r.user_id),
            secret_hash: r.secret_hash,
            expires_at: r.expires_at,
            replace: r.replace,
        })
    })
}

/// Uses the invite up; `false` if it was used, expired or its user removed
/// in the meantime.
pub async fn use_invite(db: impl PgExecutor<'_>, id: Uuid) -> Result<bool, sqlx::Error> {
    let used = sqlx::query!(
        "UPDATE invite i SET used_at = now() FROM app_user u
         WHERE i.id = $1 AND u.id = i.user_id AND i.used_at IS NULL
           AND i.expires_at > now() AND u.removed_at IS NULL",
        id,
    )
    .execute(db)
    .await?;
    Ok(used.rows_affected() == 1)
}
