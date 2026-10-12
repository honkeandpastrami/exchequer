//! Import batches (design §6.7): every transaction belongs to the one it
//! entered with, so a whole import can be found, or undone, together.

use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BatchRow {
    pub id: Uuid,
    /// `manual`, `migration` or `file`.
    pub kind: String,
    pub label: Option<String>,
    pub created_at: OffsetDateTime,
    pub version: i64,
    pub transaction_count: i64,
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    kind: &str,
    label: Option<&str>,
    token_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO import_batch (id, kind, label, token_id) VALUES ($1, $2, $3, $4)",
        id,
        kind,
        label,
        token_id,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Newest first. Optional filters: one batch, one kind, and the batches
/// listed after batch `after` (pagination).
pub async fn list(
    db: impl PgExecutor<'_>,
    id: Option<Uuid>,
    kind: Option<&str>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<BatchRow>, sqlx::Error> {
    sqlx::query_as!(
        BatchRow,
        r#"SELECT b.id, b.kind, b.label, b.created_at, b.version,
                  (SELECT count(*) FROM transaction t WHERE t.import_batch_id = b.id)
                      AS "transaction_count!"
           FROM import_batch b
           WHERE ($1::uuid IS NULL OR b.id = $1)
             AND ($2::text IS NULL OR b.kind = $2)
             AND ($3::uuid IS NULL OR (b.created_at, b.id)
                  < (SELECT a.created_at, a.id FROM import_batch a WHERE a.id = $3))
           ORDER BY b.created_at DESC, b.id DESC
           LIMIT $4"#,
        id,
        kind,
        after,
        limit,
    )
    .fetch_all(db)
    .await
}

pub async fn get(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<BatchRow>, sqlx::Error> {
    Ok(list(db, Some(id), None, None, 1).await?.pop())
}

/// The batch's version, locking it until the database transaction ends.
pub async fn lock(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT version FROM import_batch WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(db)
    .await
}

/// Records that a transaction in the batch was added, changed or removed.
pub async fn touch(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE import_batch SET version = version + 1 WHERE id = $1",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// IDs of the batch's transactions.
pub async fn transaction_ids(db: impl PgExecutor<'_>, id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT id FROM transaction WHERE import_batch_id = $1 ORDER BY date, id",
        id
    )
    .fetch_all(db)
    .await
}

/// Deletes the batch itself; its transactions must be gone already.
pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM import_batch WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}
