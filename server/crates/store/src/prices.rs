//! Prices (design §3.4). Rates are read and written as decimal strings, so
//! no precision is lost on the way.

use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PriceRow {
    pub id: Uuid,
    pub base_id: Uuid,
    pub quote_id: Uuid,
    pub date: Date,
    pub rate: String,
    /// `manual` or `provider:<name>`.
    pub source: String,
    pub provisional: bool,
    pub fetched_at: OffsetDateTime,
    pub version: i64,
}

/// Newest first (by date, then ID). Optional filters: a commodity on either
/// side, one source, dates before `(date, id)` for pagination.
pub async fn list(
    db: impl PgExecutor<'_>,
    commodity: Option<Uuid>,
    source: Option<&str>,
    after: Option<(Date, Uuid)>,
    limit: i64,
) -> Result<Vec<PriceRow>, sqlx::Error> {
    let (after_date, after_id) = after.unzip();
    sqlx::query_as!(
        PriceRow,
        r#"SELECT id, base_id, quote_id, date, rate::text AS "rate!", source, provisional,
                  fetched_at, version
           FROM price
           WHERE ($1::uuid IS NULL OR base_id = $1 OR quote_id = $1)
             AND ($2::text IS NULL OR source = $2)
             AND ($3::date IS NULL OR (date, id) < ($3, $4))
           ORDER BY date DESC, id DESC LIMIT $5"#,
        commodity,
        source,
        after_date,
        after_id,
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
) -> Result<Option<PriceRow>, sqlx::Error> {
    if lock {
        sqlx::query_as!(
            PriceRow,
            r#"SELECT id, base_id, quote_id, date, rate::text AS "rate!", source, provisional,
                      fetched_at, version
               FROM price WHERE id = $1 FOR UPDATE"#,
            id
        )
        .fetch_optional(db)
        .await
    } else {
        sqlx::query_as!(
            PriceRow,
            r#"SELECT id, base_id, quote_id, date, rate::text AS "rate!", source, provisional,
                      fetched_at, version
               FROM price WHERE id = $1"#,
            id
        )
        .fetch_optional(db)
        .await
    }
}

/// Inserts a new price; one already stored for the pair, source and day is
/// a unique violation.
pub async fn insert(db: impl PgExecutor<'_>, r: &PriceRow) -> Result<PriceRow, sqlx::Error> {
    sqlx::query_as!(
        PriceRow,
        r#"INSERT INTO price (id, base_id, quote_id, date, rate, source, provisional)
           VALUES ($1, $2, $3, $4, $5::text::numeric, $6, $7)
           RETURNING id, base_id, quote_id, date, rate::text AS "rate!", source, provisional,
                     fetched_at, version"#,
        r.id,
        r.base_id,
        r.quote_id,
        r.date,
        r.rate,
        r.source,
        r.provisional,
    )
    .fetch_one(db)
    .await
}

/// Stores a fetched price, replacing the source's earlier figure for that
/// day (a provisional one, or the same close fetched again).
pub async fn store(db: impl PgExecutor<'_>, r: &PriceRow) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO price (id, base_id, quote_id, date, rate, source, provisional)
         VALUES ($1, $2, $3, $4, $5::text::numeric, $6, $7)
         ON CONFLICT (base_id, quote_id, source, date) DO UPDATE
         SET rate = EXCLUDED.rate, provisional = EXCLUDED.provisional, fetched_at = now(),
             version = price.version + 1",
        r.id,
        r.base_id,
        r.quote_id,
        r.date,
        r.rate,
        r.source,
        r.provisional,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Changes a price's rate, bumping its version.
pub async fn set_rate(
    db: impl PgExecutor<'_>,
    id: Uuid,
    rate: &str,
) -> Result<PriceRow, sqlx::Error> {
    sqlx::query_as!(
        PriceRow,
        r#"UPDATE price SET rate = $2::text::numeric, fetched_at = now(), version = version + 1
           WHERE id = $1
           RETURNING id, base_id, quote_id, date, rate::text AS "rate!", source, provisional,
                     fetched_at, version"#,
        id,
        rate,
    )
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM price WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}

/// What valuing `commodities` between `from` and `to` may need: every price
/// touching one of them dated in `[from - window, to]`, plus each pair and
/// source's latest one before that (a stale figure beats none).
pub async fn for_valuation(
    db: impl PgExecutor<'_>,
    commodities: &[Uuid],
    from: Date,
    to: Date,
    window_days: i64,
) -> Result<Vec<PriceRow>, sqlx::Error> {
    let start = from - time::Duration::days(window_days);
    sqlx::query_as!(
        PriceRow,
        r#"SELECT id AS "id!", base_id AS "base_id!", quote_id AS "quote_id!", date AS "date!",
                  rate AS "rate!", source AS "source!", provisional AS "provisional!",
                  fetched_at AS "fetched_at!", version AS "version!"
           FROM (
               SELECT id, base_id, quote_id, date, rate::text AS rate, source, provisional,
                      fetched_at, version
               FROM price
               WHERE (base_id = ANY($1) OR quote_id = ANY($1)) AND date BETWEEN $2 AND $3
               UNION ALL
               SELECT * FROM (
                   SELECT DISTINCT ON (base_id, quote_id, source)
                          id, base_id, quote_id, date, rate::text AS rate, source, provisional,
                          fetched_at, version
                   FROM price
                   WHERE (base_id = ANY($1) OR quote_id = ANY($1)) AND date < $2
                   ORDER BY base_id, quote_id, source, date DESC
               ) older
           ) p"#,
        commodities,
        start,
        to,
    )
    .fetch_all(db)
    .await
}
