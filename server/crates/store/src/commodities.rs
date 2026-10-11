//! Commodities: currencies, securities, funds, crypto (design §3.1).

use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CommodityRow {
    pub id: Uuid,
    pub code: String,
    pub kind: String,
    pub precision: i16,
    pub version: i64,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<CommodityRow>, sqlx::Error> {
    sqlx::query_as!(
        CommodityRow,
        "SELECT id, code, kind, precision, version FROM commodity
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
) -> Result<Option<CommodityRow>, sqlx::Error> {
    if lock {
        sqlx::query_as!(
            CommodityRow,
            "SELECT id, code, kind, precision, version FROM commodity WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(db)
        .await
    } else {
        sqlx::query_as!(
            CommodityRow,
            "SELECT id, code, kind, precision, version FROM commodity WHERE id = $1",
            id
        )
        .fetch_optional(db)
        .await
    }
}

/// Inserts a new row, or overwrites every field of an existing one (bumping
/// its version).
pub async fn upsert(
    db: impl PgExecutor<'_>,
    r: &CommodityRow,
) -> Result<CommodityRow, sqlx::Error> {
    sqlx::query_as!(
        CommodityRow,
        "INSERT INTO commodity (id, code, kind, precision) VALUES ($1, $2, $3, $4)
         ON CONFLICT (id) DO UPDATE SET code = $2, kind = $3, precision = $4,
             version = commodity.version + 1
         RETURNING id, code, kind, precision, version",
        r.id,
        r.code,
        r.kind,
        r.precision,
    )
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM commodity WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}
