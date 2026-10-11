//! Balance checks (design §7.1), with the ledger balance on their date.

use rust_decimal::Decimal;
use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CheckRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub commodity_id: Uuid,
    pub date: Date,
    /// What the bank showed, in the ledger's sign.
    pub balance: Decimal,
    pub source: String,
    pub created_at: OffsetDateTime,
    /// The ledger's balance for the account and commodity at the end of `date`.
    pub ledger: Decimal,
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    account_id: Uuid,
    commodity_id: Uuid,
    date: Date,
    balance: Decimal,
    source: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO balance_check (id, account_id, commodity_id, date, balance, source)
         VALUES ($1, $2, $3, $4, $5, $6)",
        id,
        account_id,
        commodity_id,
        date,
        balance,
        source,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Checks, newest first (by date, then ID), each with its ledger balance.
/// Optional filters: one check, one account (and commodity), dates before
/// `(date, id)` for pagination.
pub async fn list(
    db: impl PgExecutor<'_>,
    id: Option<Uuid>,
    account: Option<Uuid>,
    commodity: Option<Uuid>,
    after: Option<(Date, Uuid)>,
    limit: i64,
) -> Result<Vec<CheckRow>, sqlx::Error> {
    let (after_date, after_id) = after.unzip();
    sqlx::query_as!(
        CheckRow,
        r#"SELECT c.id, c.account_id, c.commodity_id, c.date, c.balance, c.source, c.created_at,
                  COALESCE((
                      SELECT sum(CASE WHEN s.to_account_id = c.account_id
                                       AND s.to_commodity_id = c.commodity_id
                                      THEN s.to_quantity ELSE 0 END)
                           - sum(CASE WHEN s.from_account_id = c.account_id
                                       AND s.from_commodity_id = c.commodity_id
                                      THEN s.from_quantity ELSE 0 END)
                      FROM split s JOIN transaction t ON t.id = s.transaction_id
                      WHERE t.date <= c.date
                        AND c.account_id IN (s.from_account_id, s.to_account_id)
                  ), 0) AS "ledger!"
           FROM balance_check c
           WHERE ($1::uuid IS NULL OR c.id = $1)
             AND ($2::uuid IS NULL OR c.account_id = $2)
             AND ($3::uuid IS NULL OR c.commodity_id = $3)
             AND ($4::date IS NULL OR (c.date, c.id) < ($4, $5::uuid))
           ORDER BY c.date DESC, c.id DESC
           LIMIT $6"#,
        id,
        account,
        commodity,
        after_date,
        after_id,
        limit,
    )
    .fetch_all(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM balance_check WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}

/// `(transaction, position, date, signed quantity)` of every posting on the
/// account in the commodity dated after `after` (if given) through `until`.
pub async fn postings(
    db: impl PgExecutor<'_>,
    account: Uuid,
    commodity: Uuid,
    after: Option<Date>,
    until: Date,
) -> Result<Vec<(Uuid, i16, Date, Decimal)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT s.transaction_id, s.position, t.date,
                  (CASE WHEN s.to_account_id = $1 AND s.to_commodity_id = $2
                        THEN s.to_quantity ELSE 0 END
                 - CASE WHEN s.from_account_id = $1 AND s.from_commodity_id = $2
                        THEN s.from_quantity ELSE 0 END) AS "quantity!"
           FROM split s JOIN transaction t ON t.id = s.transaction_id
           WHERE $1 IN (s.from_account_id, s.to_account_id)
             AND $2 IN (s.from_commodity_id, s.to_commodity_id)
             AND ($3::date IS NULL OR t.date > $3)
             AND t.date <= $4
           ORDER BY t.date, s.transaction_id, s.position"#,
        account,
        commodity,
        after,
        until,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter(|r| !r.quantity.is_zero())
        .map(|r| (r.transaction_id, r.position, r.date, r.quantity))
        .collect())
}
