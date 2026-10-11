//! Parties: the household, people, businesses (design §4.1).

use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PartyRow {
    pub id: Uuid,
    pub name: String,
    /// `household`, `person` or `business`.
    pub kind: String,
    /// The household currency; only on the household.
    pub currency_id: Option<Uuid>,
    pub version: i64,
}

/// Up to `limit` parties with IDs after `after`, in ID order.
pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<PartyRow>, sqlx::Error> {
    sqlx::query_as!(
        PartyRow,
        "SELECT id, name, kind, currency_id, version FROM party
         WHERE $1::uuid IS NULL OR id > $1 ORDER BY id LIMIT $2",
        after,
        limit,
    )
    .fetch_all(db)
    .await
}

/// With `lock`, the row stays locked until the transaction ends.
pub async fn get(
    db: impl PgExecutor<'_>,
    id: Uuid,
    lock: bool,
) -> Result<Option<PartyRow>, sqlx::Error> {
    if lock {
        sqlx::query_as!(
            PartyRow,
            "SELECT id, name, kind, currency_id, version FROM party WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(db)
        .await
    } else {
        sqlx::query_as!(
            PartyRow,
            "SELECT id, name, kind, currency_id, version FROM party WHERE id = $1",
            id
        )
        .fetch_optional(db)
        .await
    }
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    name: &str,
    kind: &str,
) -> Result<PartyRow, sqlx::Error> {
    sqlx::query_as!(
        PartyRow,
        "INSERT INTO party (id, name, kind) VALUES ($1, $2, $3)
         RETURNING id, name, kind, currency_id, version",
        id,
        name,
        kind,
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: Uuid,
    name: &str,
    currency_id: Option<Uuid>,
) -> Result<PartyRow, sqlx::Error> {
    sqlx::query_as!(
        PartyRow,
        "UPDATE party SET name = $2, currency_id = $3, version = version + 1 WHERE id = $1
         RETURNING id, name, kind, currency_id, version",
        id,
        name,
        currency_id,
    )
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM party WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}

/// Every row whose ID is in `ids` (unknown IDs are skipped).
pub async fn get_many(db: impl PgExecutor<'_>, ids: &[Uuid]) -> Result<Vec<PartyRow>, sqlx::Error> {
    sqlx::query_as!(
        PartyRow,
        "SELECT id, name, kind, currency_id, version FROM party WHERE id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await
}

/// The household, if it exists.
pub async fn household(db: impl PgExecutor<'_>) -> Result<Option<PartyRow>, sqlx::Error> {
    sqlx::query_as!(
        PartyRow,
        "SELECT id, name, kind, currency_id, version FROM party WHERE kind = 'household'"
    )
    .fetch_optional(db)
    .await
}

/// Every party, in ID order.
pub async fn all(db: impl PgExecutor<'_>) -> Result<Vec<PartyRow>, sqlx::Error> {
    sqlx::query_as!(
        PartyRow,
        "SELECT id, name, kind, currency_id, version FROM party ORDER BY id"
    )
    .fetch_all(db)
    .await
}
