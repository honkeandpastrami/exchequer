//! Counterparties: anyone outside the household (design §3.1).

use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CounterpartyRow {
    pub id: Uuid,
    pub name: String,
    /// `opening_balances` or `reconciliation` for the two built-in ones.
    pub system: Option<String>,
    pub version: i64,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<CounterpartyRow>, sqlx::Error> {
    sqlx::query_as!(
        CounterpartyRow,
        "SELECT id, name, system, version FROM counterparty
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
) -> Result<Option<CounterpartyRow>, sqlx::Error> {
    if lock {
        sqlx::query_as!(
            CounterpartyRow,
            "SELECT id, name, system, version FROM counterparty WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(db)
        .await
    } else {
        sqlx::query_as!(
            CounterpartyRow,
            "SELECT id, name, system, version FROM counterparty WHERE id = $1",
            id
        )
        .fetch_optional(db)
        .await
    }
}

/// Inserts a new counterparty, or renames an existing one (bumping its
/// version). `system` is never set through here.
pub async fn upsert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    name: &str,
) -> Result<CounterpartyRow, sqlx::Error> {
    sqlx::query_as!(
        CounterpartyRow,
        "INSERT INTO counterparty (id, name) VALUES ($1, $2)
         ON CONFLICT (id) DO UPDATE SET name = $2, version = counterparty.version + 1
         RETURNING id, name, system, version",
        id,
        name,
    )
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM counterparty WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}
