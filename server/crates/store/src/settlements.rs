//! Each month's settlement parameters and whether it's closed (design §4.3).
//! Months are stored as their first day.

use rust_decimal::Decimal;
use sqlx::{PgConnection, PgExecutor};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SettlementRow {
    pub month: Date,
    pub id: Uuid,
    pub closed_at: Option<OffsetDateTime>,
    pub version: i64,
}

/// A month's parameters: `(person, stipend)` and `(business, funder)` pairs.
#[derive(Debug, Clone, Default)]
pub struct Params {
    pub stipends: Vec<(Uuid, Decimal)>,
    pub funders: Vec<(Uuid, Uuid)>,
}

/// With `lock`, the row stays locked until the transaction ends.
pub async fn get(
    db: impl PgExecutor<'_>,
    month: Date,
    lock: bool,
) -> Result<Option<SettlementRow>, sqlx::Error> {
    if lock {
        sqlx::query_as!(
            SettlementRow,
            "SELECT month, id, closed_at, version FROM settlement WHERE month = $1 FOR UPDATE",
            month
        )
        .fetch_optional(db)
        .await
    } else {
        sqlx::query_as!(
            SettlementRow,
            "SELECT month, id, closed_at, version FROM settlement WHERE month = $1",
            month
        )
        .fetch_optional(db)
        .await
    }
}

/// Every month with parameters, oldest first.
pub async fn all(db: impl PgExecutor<'_>) -> Result<Vec<SettlementRow>, sqlx::Error> {
    sqlx::query_as!(
        SettlementRow,
        "SELECT month, id, closed_at, version FROM settlement ORDER BY month"
    )
    .fetch_all(db)
    .await
}

/// Every month's parameters, keyed by month.
pub async fn all_params(db: &mut PgConnection) -> Result<Vec<(Date, Params)>, sqlx::Error> {
    let stipends = sqlx::query!("SELECT month, party_id, amount FROM settlement_stipend")
        .fetch_all(&mut *db)
        .await?;
    let funders = sqlx::query!("SELECT month, business_id, funder_id FROM settlement_funder")
        .fetch_all(&mut *db)
        .await?;
    let mut out: std::collections::BTreeMap<Date, Params> = all(&mut *db)
        .await?
        .into_iter()
        .map(|r| (r.month, Params::default()))
        .collect();
    for r in stipends {
        out.entry(r.month)
            .or_default()
            .stipends
            .push((r.party_id, r.amount));
    }
    for r in funders {
        out.entry(r.month)
            .or_default()
            .funders
            .push((r.business_id, r.funder_id));
    }
    Ok(out.into_iter().collect())
}

/// Sets a month's parameters, creating its row if needed. Returns the row.
pub async fn put(
    db: &mut PgConnection,
    month: Date,
    params: &Params,
) -> Result<SettlementRow, sqlx::Error> {
    let row = sqlx::query_as!(
        SettlementRow,
        "INSERT INTO settlement (month, id) VALUES ($1, $2)
         ON CONFLICT (month) DO UPDATE SET version = settlement.version + 1
         RETURNING month, id, closed_at, version",
        month,
        Uuid::now_v7()
    )
    .fetch_one(&mut *db)
    .await?;
    sqlx::query!("DELETE FROM settlement_stipend WHERE month = $1", month)
        .execute(&mut *db)
        .await?;
    sqlx::query!("DELETE FROM settlement_funder WHERE month = $1", month)
        .execute(&mut *db)
        .await?;
    for (party, amount) in &params.stipends {
        sqlx::query!(
            "INSERT INTO settlement_stipend (month, party_id, amount) VALUES ($1, $2, $3)",
            month,
            party,
            amount
        )
        .execute(&mut *db)
        .await?;
    }
    for (business, funder) in &params.funders {
        sqlx::query!(
            "INSERT INTO settlement_funder (month, business_id, funder_id) VALUES ($1, $2, $3)",
            month,
            business,
            funder
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(row)
}

/// Closes (`true`) or reopens a month that has a row.
pub async fn set_closed(
    db: impl PgExecutor<'_>,
    month: Date,
    closed: bool,
) -> Result<SettlementRow, sqlx::Error> {
    sqlx::query_as!(
        SettlementRow,
        "UPDATE settlement
         SET closed_at = CASE WHEN $2 THEN now() END, version = version + 1
         WHERE month = $1
         RETURNING month, id, closed_at, version",
        month,
        closed
    )
    .fetch_one(db)
    .await
}

/// The first of `dates` that falls in a closed month, if any.
pub async fn first_closed(
    db: impl PgExecutor<'_>,
    dates: &[Date],
) -> Result<Option<Date>, sqlx::Error> {
    let row = sqlx::query_scalar!(
        "SELECT month FROM settlement
         WHERE closed_at IS NOT NULL
           AND month = ANY(SELECT date_trunc('month', d)::date FROM unnest($1::date[]) d)
         ORDER BY month LIMIT 1",
        dates
    )
    .fetch_optional(db)
    .await?;
    Ok(row)
}
