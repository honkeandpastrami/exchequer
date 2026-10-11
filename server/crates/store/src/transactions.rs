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
    /// First day of the month a settlement payment settles, if set by hand.
    pub settles: Option<Date>,
}

/// A transaction with everything in it.
#[derive(Debug, Clone)]
pub struct Full {
    pub transaction: TransactionRow,
    /// In `position` order.
    pub splits: Vec<SplitRow>,
    /// Tag names, sorted.
    pub tags: Vec<String>,
    /// `(source, ref)` pairs, sorted.
    pub external_refs: Vec<(String, String)>,
}

/// Optional filters for [`list`]; `None` means "any".
#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub from: Option<Date>,
    pub to: Option<Date>,
    pub account: Option<Uuid>,
    pub counterparty: Option<Uuid>,
    pub category: Option<Uuid>,
    /// `(source, ref)`: the transaction with this external reference.
    pub external_ref: Option<(String, String)>,
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
    let (ext_source, ext_ref) = filter.external_ref.clone().unzip();
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
             AND ($8::text IS NULL OR EXISTS (SELECT 1 FROM transaction_external_ref e
                  WHERE e.transaction_id = t.id AND e.source = $8 AND e.ref = $9))
           ORDER BY t.date DESC, t.id DESC
           LIMIT $10"#,
        after_date,
        after_id,
        filter.from,
        filter.to,
        filter.account,
        filter.counterparty,
        filter.category,
        ext_source,
        ext_ref,
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
                purpose_id, category_id, description, settles
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

/// `(transaction id, source, ref)` for several transactions.
pub async fn external_refs_of(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String, String)>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT transaction_id, source, ref FROM transaction_external_ref
         WHERE transaction_id = ANY($1) ORDER BY source, ref",
        ids,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.transaction_id, r.source, r.r#ref))
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
    let refs = external_refs_of(&mut *db, &ids).await?;
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
                external_refs: refs
                    .iter()
                    .filter(|(t, _, _)| *t == id)
                    .map(|(_, s, r)| (s.clone(), r.clone()))
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
                 purpose_id, category_id, description, settles)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
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
            s.settles,
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

    sqlx::query!(
        "DELETE FROM transaction_external_ref WHERE transaction_id = $1",
        t.id
    )
    .execute(&mut *db)
    .await?;
    for (source, r) in &full.external_refs {
        sqlx::query!(
            "INSERT INTO transaction_external_ref (transaction_id, source, ref) VALUES ($1, $2, $3)",
            t.id,
            source,
            r,
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

/// A split as the settlement needs it: with its transaction's date.
#[derive(Debug, Clone)]
pub struct DatedSplit {
    pub date: Date,
    pub split: SplitRow,
}

/// Every split that can matter to the settlement: those with a purpose, and
/// those between two accounts (transfers, liquidations).
pub async fn settlement_splits(db: impl PgExecutor<'_>) -> Result<Vec<DatedSplit>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT t.date, s.transaction_id, s.position,
                s.from_account_id, s.from_counterparty_id, s.from_commodity_id, s.from_quantity,
                s.to_account_id, s.to_counterparty_id, s.to_commodity_id, s.to_quantity,
                s.purpose_id, s.category_id, s.description, s.settles
         FROM split s JOIN transaction t ON t.id = s.transaction_id
         WHERE s.purpose_id IS NOT NULL
            OR (s.from_account_id IS NOT NULL AND s.to_account_id IS NOT NULL)
         ORDER BY t.date, s.transaction_id, s.position"
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| DatedSplit {
            date: r.date,
            split: SplitRow {
                transaction_id: r.transaction_id,
                position: r.position,
                from_account_id: r.from_account_id,
                from_counterparty_id: r.from_counterparty_id,
                from_commodity_id: r.from_commodity_id,
                from_quantity: r.from_quantity,
                to_account_id: r.to_account_id,
                to_counterparty_id: r.to_counterparty_id,
                to_commodity_id: r.to_commodity_id,
                to_quantity: r.to_quantity,
                purpose_id: r.purpose_id,
                category_id: r.category_id,
                description: r.description,
                settles: r.settles,
            },
        })
        .collect())
}

/// A split with what the query engine filters on from its transaction.
#[derive(Debug, Clone)]
pub struct QuerySplit {
    pub date: Date,
    pub description: String,
    pub notes: Option<String>,
    pub split: SplitRow,
}

/// Every split dated `from` to `to` (inclusive; `None` is open-ended),
/// optionally only those touching one of `accounts`.
pub async fn splits_between(
    db: impl PgExecutor<'_>,
    from: Option<Date>,
    to: Option<Date>,
    accounts: Option<&[Uuid]>,
) -> Result<Vec<QuerySplit>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT t.date, t.description AS tx_description, t.notes, s.transaction_id, s.position,
                s.from_account_id, s.from_counterparty_id, s.from_commodity_id, s.from_quantity,
                s.to_account_id, s.to_counterparty_id, s.to_commodity_id, s.to_quantity,
                s.purpose_id, s.category_id, s.description, s.settles
         FROM split s JOIN transaction t ON t.id = s.transaction_id
         WHERE ($1::date IS NULL OR t.date >= $1)
           AND ($2::date IS NULL OR t.date <= $2)
           AND ($3::uuid[] IS NULL OR s.from_account_id = ANY($3) OR s.to_account_id = ANY($3))
         ORDER BY t.date, s.transaction_id, s.position",
        from,
        to,
        accounts,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| QuerySplit {
            date: r.date,
            description: r.tx_description,
            notes: r.notes,
            split: SplitRow {
                transaction_id: r.transaction_id,
                position: r.position,
                from_account_id: r.from_account_id,
                from_counterparty_id: r.from_counterparty_id,
                from_commodity_id: r.from_commodity_id,
                from_quantity: r.from_quantity,
                to_account_id: r.to_account_id,
                to_counterparty_id: r.to_counterparty_id,
                to_commodity_id: r.to_commodity_id,
                to_quantity: r.to_quantity,
                purpose_id: r.purpose_id,
                category_id: r.category_id,
                description: r.description,
                settles: r.settles,
            },
        })
        .collect())
}

/// Each account's balance per commodity from everything dated before `day`.
pub async fn balances_before(
    db: impl PgExecutor<'_>,
    day: Date,
) -> Result<Vec<(Uuid, Uuid, Decimal)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT account_id AS "account_id!", commodity_id AS "commodity_id!",
                  sum(quantity) AS "balance!"
           FROM (SELECT s.from_account_id AS account_id, s.from_commodity_id AS commodity_id,
                        -s.from_quantity AS quantity
                 FROM split s JOIN transaction t ON t.id = s.transaction_id
                 WHERE t.date < $1 AND s.from_account_id IS NOT NULL
                 UNION ALL
                 SELECT s.to_account_id, s.to_commodity_id, s.to_quantity
                 FROM split s JOIN transaction t ON t.id = s.transaction_id
                 WHERE t.date < $1 AND s.to_account_id IS NOT NULL) p
           GROUP BY account_id, commodity_id"#,
        day,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.account_id, r.commodity_id, r.balance))
        .collect())
}
