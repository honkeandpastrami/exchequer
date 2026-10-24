//! Sign-ins in progress (design §10): passkey ceremonies, and the CLI's
//! device sign-ins. Both are short-lived and used once.

use domain::UserId;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Register,
    SignIn,
}

impl Kind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::SignIn => "sign_in",
        }
    }
}

/// Forgets every ceremony and device sign-in that has expired.
pub async fn prune(db: &mut sqlx::PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM auth_ceremony WHERE expires_at <= now()")
        .execute(&mut *db)
        .await?;
    sqlx::query!("DELETE FROM device_sign_in WHERE expires_at <= now()")
        .execute(&mut *db)
        .await?;
    Ok(())
}

pub async fn start(
    db: impl PgExecutor<'_>,
    id: Uuid,
    kind: Kind,
    invite: Option<Uuid>,
    state: &serde_json::Value,
    expires_at: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO auth_ceremony (id, kind, invite_id, state, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
        id,
        kind.as_str(),
        invite,
        state,
        expires_at,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Takes the ceremony out, so it can't be finished twice: its invite and
/// state, or `None` if there's no such ceremony of this kind or it expired.
pub async fn take(
    db: impl PgExecutor<'_>,
    id: Uuid,
    kind: Kind,
) -> Result<Option<(Option<Uuid>, serde_json::Value)>, sqlx::Error> {
    sqlx::query!(
        "DELETE FROM auth_ceremony WHERE id = $1 AND kind = $2 AND expires_at > now()
         RETURNING invite_id, state",
        id,
        kind.as_str(),
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(|r| (r.invite_id, r.state)))
}

#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub id: Uuid,
    pub secret_hash: String,
    pub user_code: String,
    pub client: String,
    /// How long the CLI's token will last.
    pub ttl_days: i32,
    pub expires_at: OffsetDateTime,
    pub approved_by: Option<UserId>,
}

struct RawDevice {
    id: Uuid,
    secret_hash: String,
    user_code: String,
    client: String,
    ttl_days: i32,
    expires_at: OffsetDateTime,
    approved_by: Option<Uuid>,
}

impl From<RawDevice> for DeviceRow {
    fn from(r: RawDevice) -> Self {
        Self {
            id: r.id,
            secret_hash: r.secret_hash,
            user_code: r.user_code,
            client: r.client,
            ttl_days: r.ttl_days,
            expires_at: r.expires_at,
            approved_by: r.approved_by.map(UserId::from_uuid),
        }
    }
}

/// Fails with a unique violation if the user code is taken.
pub async fn start_device(
    db: impl PgExecutor<'_>,
    id: Uuid,
    secret_hash: &str,
    user_code: &str,
    client: &str,
    ttl_days: i32,
    expires_at: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO device_sign_in (id, secret_hash, user_code, client, ttl_days, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        id,
        secret_hash,
        user_code,
        client,
        ttl_days,
        expires_at,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The unexpired device sign-in, by the ID in its device code.
pub async fn device(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<DeviceRow>, sqlx::Error> {
    sqlx::query_as!(
        RawDevice,
        "SELECT id, secret_hash, user_code, client, ttl_days, expires_at, approved_by
         FROM device_sign_in WHERE id = $1 AND expires_at > now()",
        id,
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(DeviceRow::from))
}

/// The unexpired device sign-in waiting for approval under this user code.
pub async fn pending_device(
    db: impl PgExecutor<'_>,
    user_code: &str,
) -> Result<Option<DeviceRow>, sqlx::Error> {
    sqlx::query_as!(
        RawDevice,
        "SELECT id, secret_hash, user_code, client, ttl_days, expires_at, approved_by
         FROM device_sign_in
         WHERE user_code = $1 AND expires_at > now() AND approved_by IS NULL",
        user_code,
    )
    .fetch_optional(db)
    .await
    .map(|r| r.map(DeviceRow::from))
}

/// `false` if it's no longer waiting (approved already, or expired).
pub async fn approve_device(
    db: impl PgExecutor<'_>,
    user_code: &str,
    user: UserId,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        "UPDATE device_sign_in SET approved_by = $2
         WHERE user_code = $1 AND expires_at > now() AND approved_by IS NULL",
        user_code,
        user.as_uuid(),
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Takes an approved device sign-in out, so its token is handed out once.
pub async fn take_device(db: impl PgExecutor<'_>, id: Uuid) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        "DELETE FROM device_sign_in
         WHERE id = $1 AND expires_at > now() AND approved_by IS NOT NULL",
        id,
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}
