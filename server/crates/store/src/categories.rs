//! Categories: the class of an income or expense (design §3.1).

use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CategoryRow {
    pub id: Uuid,
    pub name: String,
    pub version: i64,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<CategoryRow>, sqlx::Error> {
    sqlx::query_as!(
        CategoryRow,
        "SELECT id, name, version FROM category
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
) -> Result<Option<CategoryRow>, sqlx::Error> {
    if lock {
        sqlx::query_as!(
            CategoryRow,
            "SELECT id, name, version FROM category WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(db)
        .await
    } else {
        sqlx::query_as!(
            CategoryRow,
            "SELECT id, name, version FROM category WHERE id = $1",
            id
        )
        .fetch_optional(db)
        .await
    }
}

/// Inserts a new row, or renames an existing one (bumping its version).
pub async fn upsert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    name: &str,
) -> Result<CategoryRow, sqlx::Error> {
    sqlx::query_as!(
        CategoryRow,
        "INSERT INTO category (id, name) VALUES ($1, $2)
         ON CONFLICT (id) DO UPDATE SET name = $2, version = category.version + 1
         RETURNING id, name, version",
        id,
        name,
    )
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM category WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}
