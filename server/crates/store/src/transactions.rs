//! Transactions with their splits and tags (design §3.1). A transaction is
//! always written whole: its splits and tags are replaced together.

use rust_decimal::Decimal;
use sqlx::{PgConnection, PgExecutor};
use time::Date;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TransactionRow {
    pub id: Uuid,
    pub date: Date,
    pub description: String,
    pub notes: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct SplitRow {
    pub transaction_id: Uuid,
    pub position: i16,
    pub from_account_id: Option<Uuid>,
    pub from_counterparty_id: Option<Uuid>,
    pub from_commodity_id: Uuid,
    pub from_quantity: Decimal,
    pub to_account_id: Option<Uuid>,
    pub to_counterparty_id: Option<Uuid>,
    pub to_commodity_id: Uuid,
    pub to_quantity: Decimal,
    pub purpose_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub description: Option<String>,
}

/// A transaction with everything in it.
#[derive(Debug, Clone)]
pub struct Full {
    pub transaction: TransactionRow,
    /// In `position` order.
    pub splits: Vec<SplitRow>,
    /// Tag names, sorted.
    pub tags: Vec<String>,
}

/// Optional filters for [`list`]; `None` means "any".
#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub from: Option<Date>,
    pub to: Option<Date>,
    pub account: Option<Uuid>,
    pub counterparty: Option<Uuid>,
    pub category: Option<Uuid>,
}

/// Newest first: by date, then by ID. `after` is the `(date, id)` of the last
/// row of the previous page.
pub async fn list(
    db: impl PgExecutor<'_>,
    filter: &Filter,
    after: Option<(Date, Uuid)>,
    limit: i64,
) -> Result<Vec<TransactionRow>, sqlx::Error> {
    let (after_date, after_id) = after.unzip();
    sqlx::query_as!(
        TransactionRow,
        r#"SELECT t.id, t.date, t.description, t.notes, t.version
           FROM transaction t
           WHERE ($1::date IS NULL OR (t.date, t.id) < ($1, $2::uuid))
             AND ($3::date IS NULL OR t.date >= $3)
             AND ($4::date IS NULL OR t.date <= $4)
             AND ($5::uuid IS NULL OR EXISTS (SELECT 1 FROM split s WHERE s.transaction_id = t.id
                  AND $5 IN (s.from_account_id, s.to_account_id)))
             AND ($6::uuid IS NULL OR EXISTS (SELECT 1 FROM split s WHERE s.transaction_id = t.id
                  AND $6 IN (s.from_counterparty_id, s.to_counterparty_id)))
             AND ($7::uuid IS NULL OR EXISTS (SELECT 1 FROM split s WHERE s.transaction_id = t.id
                  AND s.category_id = $7))
           ORDER BY t.date DESC, t.id DESC
           LIMIT $8"#,
        after_date,
        after_id,
        filter.from,
        filter.to,
        filter.account,
        filter.counterparty,
        filter.category,
        limit,
    )
    .fetch_all(db)
    .await
}

/// Splits of several transactions, in transaction then position order.
pub async fn splits_of(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<SplitRow>, sqlx::Error> {
    sqlx::query_as!(
        SplitRow,
        "SELECT transaction_id, position,
                from_account_id, from_counterparty_id, from_commodity_id, from_quantity,
                to_account_id, to_counterparty_id, to_commodity_id, to_quantity,
                purpose_id, category_id, description
         FROM split WHERE transaction_id = ANY($1) ORDER BY transaction_id, position",
        ids,
    )
    .fetch_all(db)
    .await
}

/// `(transaction id, tag name)` pairs for several transactions.
pub async fn tags_of(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT tt.transaction_id, t.name FROM transaction_tag tt JOIN tag t ON t.id = tt.tag_id
         WHERE tt.transaction_id = ANY($1) ORDER BY t.name",
        ids,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.transaction_id, r.name))
        .collect())
}

/// Attaches splits and tags to transaction rows, keeping their order.
pub async fn complete(
    db: &mut PgConnection,
    rows: Vec<TransactionRow>,
) -> Result<Vec<Full>, sqlx::Error> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut splits = splits_of(&mut *db, &ids).await?;
    let tags = tags_of(&mut *db, &ids).await?;
    Ok(rows
        .into_iter()
        .map(|transaction| {
            let id = transaction.id;
            let (mine, rest) = splits.drain(..).partition(|s| s.transaction_id == id);
            splits = rest;
            Full {
                transaction,
                splits: mine,
                tags: tags
                    .iter()
                    .filter(|(t, _)| *t == id)
                    .map(|(_, n)| n.clone())
                    .collect(),
            }
        })
        .collect())
}

/// With `lock`, the transaction row stays locked until the database
/// transaction ends.
pub async fn get(db: &mut PgConnection, id: Uuid, lock: bool) -> Result<Option<Full>, sqlx::Error> {
    let row = if lock {
        sqlx::query_as!(
            TransactionRow,
            "SELECT id, date, description, notes, version FROM transaction WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(&mut *db)
        .await?
    } else {
        sqlx::query_as!(
            TransactionRow,
            "SELECT id, date, description, notes, version FROM transaction WHERE id = $1",
            id
        )
        .fetch_optional(&mut *db)
        .await?
    };
    match row {
        Some(r) => Ok(complete(db, vec![r]).await?.pop()),
        None => Ok(None),
    }
}

/// Inserts a new transaction, or replaces every part of an existing one
/// (bumping its version). Returns the stored version.
pub async fn upsert(db: &mut PgConnection, full: &Full) -> Result<i64, sqlx::Error> {
    let t = &full.transaction;
    let version = sqlx::query_scalar!(
        "INSERT INTO transaction (id, date, description, notes) VALUES ($1, $2, $3, $4)
         ON CONFLICT (id) DO UPDATE SET date = $2, description = $3, notes = $4,
             version = transaction.version + 1, updated_at = now()
         RETURNING version",
        t.id,
        t.date,
        t.description,
        t.notes,
    )
    .fetch_one(&mut *db)
    .await?;

    sqlx::query!("DELETE FROM split WHERE transaction_id = $1", t.id)
        .execute(&mut *db)
        .await?;
    for s in &full.splits {
        sqlx::query!(
            "INSERT INTO split (transaction_id, position,
                 from_account_id, from_counterparty_id, from_commodity_id, from_quantity,
                 to_account_id, to_counterparty_id, to_commodity_id, to_quantity,
                 purpose_id, category_id, description)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            t.id,
            s.position,
            s.from_account_id,
            s.from_counterparty_id,
            s.from_commodity_id,
            s.from_quantity,
            s.to_account_id,
            s.to_counterparty_id,
            s.to_commodity_id,
            s.to_quantity,
            s.purpose_id,
            s.category_id,
            s.description,
        )
        .execute(&mut *db)
        .await?;
    }

    sqlx::query!(
        "DELETE FROM transaction_tag WHERE transaction_id = $1",
        t.id
    )
    .execute(&mut *db)
    .await?;
    for name in &full.tags {
        // Creates the tag on first use; returns its ID either way.
        let tag_id = sqlx::query_scalar!(
            "INSERT INTO tag (id, name) VALUES ($1, $2)
             ON CONFLICT (name) DO UPDATE SET name = excluded.name
             RETURNING id",
            Uuid::now_v7(),
            name,
        )
        .fetch_one(&mut *db)
        .await?;
        sqlx::query!(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES ($1, $2)",
            t.id,
            tag_id,
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(version)
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM transaction WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}
