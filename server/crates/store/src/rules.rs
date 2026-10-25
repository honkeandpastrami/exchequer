//! The rules script's versions (design §8). Versions are only ever added.

use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RuleScriptRow {
    pub version: i32,
    pub source: String,
    pub note: Option<String>,
    pub token_id: Option<Uuid>,
    /// The name of the user whose token uploaded it.
    pub author: Option<String>,
    pub created_at: OffsetDateTime,
}

/// The newest version.
pub async fn active(db: impl PgExecutor<'_>) -> Result<Option<RuleScriptRow>, sqlx::Error> {
    sqlx::query_as!(
        RuleScriptRow,
        r#"SELECT r.version, r.source, r.note, r.token_id, u.name AS "author?", r.created_at
           FROM rule_script r LEFT JOIN api_token t ON t.id = r.token_id
           LEFT JOIN app_user u ON u.id = t.user_id
           ORDER BY r.version DESC LIMIT 1"#
    )
    .fetch_optional(db)
    .await
}

pub async fn get(
    db: impl PgExecutor<'_>,
    version: i32,
) -> Result<Option<RuleScriptRow>, sqlx::Error> {
    sqlx::query_as!(
        RuleScriptRow,
        r#"SELECT r.version, r.source, r.note, r.token_id, u.name AS "author?", r.created_at
           FROM rule_script r LEFT JOIN api_token t ON t.id = r.token_id
           LEFT JOIN app_user u ON u.id = t.user_id
           WHERE r.version = $1"#,
        version
    )
    .fetch_optional(db)
    .await
}

/// Newest first, before version `before` if given.
pub async fn list(
    db: impl PgExecutor<'_>,
    before: Option<i32>,
    limit: i64,
) -> Result<Vec<RuleScriptRow>, sqlx::Error> {
    sqlx::query_as!(
        RuleScriptRow,
        r#"SELECT r.version, r.source, r.note, r.token_id, u.name AS "author?", r.created_at
           FROM rule_script r LEFT JOIN api_token t ON t.id = r.token_id
           LEFT JOIN app_user u ON u.id = t.user_id
           WHERE $1::int4 IS NULL OR r.version < $1
           ORDER BY r.version DESC LIMIT $2"#,
        before,
        limit
    )
    .fetch_all(db)
    .await
}

/// Adds a version; the caller holds the lock ([`lock`]) and gives the next
/// number.
pub async fn insert(
    db: impl PgExecutor<'_>,
    version: i32,
    source: &str,
    note: Option<&str>,
    token_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO rule_script (version, source, note, token_id) VALUES ($1, $2, $3, $4)",
        version,
        source,
        note,
        token_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Serializes uploads until the transaction ends.
pub async fn lock(db: impl PgExecutor<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!("LOCK TABLE rule_script IN EXCLUSIVE MODE")
        .execute(db)
        .await?;
    Ok(())
}

/// Every name a script may use.
#[derive(Debug, Clone, Default)]
pub struct NameRows {
    /// (id, name, owner's name)
    pub accounts: Vec<(Uuid, String, String)>,
    pub counterparties: Vec<(Uuid, String)>,
    pub categories: Vec<(Uuid, String)>,
    pub parties: Vec<(Uuid, String)>,
}

pub async fn names(db: &mut sqlx::PgConnection) -> Result<NameRows, sqlx::Error> {
    let accounts = sqlx::query!(
        "SELECT a.id, a.name, p.name AS owner FROM account a JOIN party p ON p.id = a.owner_id"
    )
    .fetch_all(&mut *db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.name, r.owner))
    .collect();
    let counterparties = sqlx::query!("SELECT id, name FROM counterparty")
        .fetch_all(&mut *db)
        .await?
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();
    let categories = sqlx::query!("SELECT id, name FROM category")
        .fetch_all(&mut *db)
        .await?
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();
    let parties = sqlx::query!("SELECT id, name FROM party")
        .fetch_all(&mut *db)
        .await?
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();
    Ok(NameRows {
        accounts,
        counterparties,
        categories,
        parties,
    })
}
