//! Accounts: things of yours that hold value (design §3.1).

use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub owner_id: Uuid,
    pub liquid: bool,
    pub version: i64,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<AccountRow>, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "SELECT id, name, kind, owner_id, liquid, version FROM account
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
) -> Result<Option<AccountRow>, sqlx::Error> {
    if lock {
        sqlx::query_as!(
            AccountRow,
            "SELECT id, name, kind, owner_id, liquid, version FROM account WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(db)
        .await
    } else {
        sqlx::query_as!(
            AccountRow,
            "SELECT id, name, kind, owner_id, liquid, version FROM account WHERE id = $1",
            id
        )
        .fetch_optional(db)
        .await
    }
}

/// Inserts a new row, or overwrites the editable fields of an existing one
/// (bumping its version). The owner never changes after creation.
pub async fn upsert(db: impl PgExecutor<'_>, r: &AccountRow) -> Result<AccountRow, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "INSERT INTO account (id, name, kind, owner_id, liquid) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (id) DO UPDATE SET name = $2, kind = $3, liquid = $5,
             version = account.version + 1
         RETURNING id, name, kind, owner_id, liquid, version",
        r.id,
        r.name,
        r.kind,
        r.owner_id,
        r.liquid,
    )
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM account WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}

/// Every row whose ID is in `ids` (unknown IDs are skipped).
pub async fn get_many(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<AccountRow>, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "SELECT id, name, kind, owner_id, liquid, version FROM account WHERE id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await
}
