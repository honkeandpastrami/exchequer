//! Staged imports (design §6): uploaded bank files waiting for review. A
//! file has statements (one per account), and a statement has rows, each
//! with its status: new, a duplicate of an existing transaction, and so on.

use domain::AccountNumber;
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ImportRow {
    pub id: Uuid,
    pub file_name: Option<String>,
    /// `ofx` or `csv`.
    pub format: String,
    pub created_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct StatementRow {
    pub position: i16,
    pub account_number: Option<AccountNumber>,
    pub account_id: Option<Uuid>,
    pub kind: Option<String>,
    pub commodity_id: Uuid,
    pub ledger_amount: Option<Decimal>,
    pub ledger_date: Option<Date>,
    pub skipped: Value,
}

#[derive(Debug, Clone)]
pub struct RowRow {
    pub statement: i16,
    pub position: i32,
    pub date: Date,
    pub amount: Decimal,
    pub description: String,
    pub native_id: Option<String>,
    pub fingerprint: String,
    pub raw: Value,
    pub status: String,
    pub match_transaction_id: Option<Uuid>,
}

/// How many of a statement's rows have each status.
#[derive(Debug, Clone)]
pub struct StatusCount {
    pub statement: i16,
    pub status: String,
    pub count: i64,
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    file_name: Option<&str>,
    format: &str,
    token_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO staged_import (id, file_name, format, token_id) VALUES ($1, $2, $3, $4)",
        id,
        file_name,
        format,
        token_id,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn insert_statement(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    s: &StatementRow,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO staged_statement (import_id, position, account_number, account_id, kind,
                                       commodity_id, ledger_amount, ledger_date, skipped)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        import_id,
        s.position,
        s.account_number.as_ref().map(AccountNumber::reveal),
        s.account_id,
        s.kind,
        s.commodity_id,
        s.ledger_amount,
        s.ledger_date,
        s.skipped,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Inserts rows in one statement.
pub async fn insert_rows(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    rows: &[RowRow],
) -> Result<(), sqlx::Error> {
    let col = |f: fn(&RowRow) -> String| rows.iter().map(f).collect::<Vec<_>>();
    let statements: Vec<i16> = rows.iter().map(|r| r.statement).collect();
    let positions: Vec<i32> = rows.iter().map(|r| r.position).collect();
    let dates: Vec<Date> = rows.iter().map(|r| r.date).collect();
    let amounts: Vec<Decimal> = rows.iter().map(|r| r.amount).collect();
    let native: Vec<Option<String>> = rows.iter().map(|r| r.native_id.clone()).collect();
    let raw: Vec<Value> = rows.iter().map(|r| r.raw.clone()).collect();
    let matches: Vec<Option<Uuid>> = rows.iter().map(|r| r.match_transaction_id).collect();
    sqlx::query!(
        "INSERT INTO staged_row (import_id, statement, position, date, amount, description,
                                 native_id, fingerprint, raw, status, match_transaction_id)
         SELECT $1, * FROM unnest($2::int2[], $3::int4[], $4::date[], $5::numeric[], $6::text[],
                                  $7::text[], $8::text[], $9::jsonb[], $10::text[], $11::uuid[])",
        import_id,
        &statements,
        &positions,
        &dates,
        &amounts,
        &col(|r| r.description.clone()),
        &native as &[Option<String>],
        &col(|r| r.fingerprint.clone()),
        &raw,
        &col(|r| r.status.clone()),
        &matches as &[Option<Uuid>],
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<ImportRow>, sqlx::Error> {
    sqlx::query_as!(
        ImportRow,
        "SELECT id, file_name, format, created_at, version FROM staged_import WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

/// Newest first; `after` is the last import of the previous page.
pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<ImportRow>, sqlx::Error> {
    sqlx::query_as!(
        ImportRow,
        "SELECT i.id, i.file_name, i.format, i.created_at, i.version FROM staged_import i
         WHERE $1::uuid IS NULL OR (i.created_at, i.id)
               < (SELECT a.created_at, a.id FROM staged_import a WHERE a.id = $1)
         ORDER BY i.created_at DESC, i.id DESC LIMIT $2",
        after,
        limit
    )
    .fetch_all(db)
    .await
}

/// The import's version, locked until the transaction ends.
pub async fn lock(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT version FROM staged_import WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn touch(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE staged_import SET version = version + 1 WHERE id = $1",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM staged_import WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(())
}

/// The imports' statements, in order.
pub async fn statements(
    db: impl PgExecutor<'_>,
    imports: &[Uuid],
) -> Result<Vec<(Uuid, StatementRow)>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT import_id, position, account_number, account_id, kind, commodity_id,
                ledger_amount, ledger_date, skipped
         FROM staged_statement WHERE import_id = ANY($1) ORDER BY import_id, position",
        imports
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.import_id,
                StatementRow {
                    position: r.position,
                    account_number: r.account_number.as_deref().and_then(AccountNumber::new),
                    account_id: r.account_id,
                    kind: r.kind,
                    commodity_id: r.commodity_id,
                    ledger_amount: r.ledger_amount,
                    ledger_date: r.ledger_date,
                    skipped: r.skipped,
                },
            )
        })
        .collect())
}

pub async fn set_account(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: i16,
    account_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE staged_statement SET account_id = $3 WHERE import_id = $1 AND position = $2",
        import_id,
        statement,
        account_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Row counts by statement and status.
pub async fn counts(
    db: impl PgExecutor<'_>,
    imports: &[Uuid],
) -> Result<Vec<(Uuid, StatusCount)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT import_id, statement, status, count(*) AS "count!" FROM staged_row
           WHERE import_id = ANY($1) GROUP BY import_id, statement, status
           ORDER BY import_id, statement, status"#,
        imports
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.import_id,
                StatusCount {
                    statement: r.statement,
                    status: r.status,
                    count: r.count,
                },
            )
        })
        .collect())
}

/// Rows in order, optionally of one statement and one status, after
/// `(statement, position)` (pagination).
pub async fn rows(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: Option<i16>,
    status: Option<&str>,
    after: Option<(i16, i32)>,
    limit: i64,
) -> Result<Vec<RowRow>, sqlx::Error> {
    sqlx::query_as!(
        RowRow,
        "SELECT statement, position, date, amount, description, native_id, fingerprint, raw,
                status, match_transaction_id
         FROM staged_row
         WHERE import_id = $1
           AND ($2::int2 IS NULL OR statement = $2)
           AND ($3::text IS NULL OR status = $3)
           AND ($4::int2 IS NULL OR (statement, position) > ($4, $5::int4))
         ORDER BY statement, position LIMIT $6",
        import_id,
        statement,
        status,
        after.map(|a| a.0),
        after.map(|a| a.1),
        limit
    )
    .fetch_all(db)
    .await
}

/// Sets each row's status and match, by position, in one statement.
pub async fn set_statuses(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: i16,
    rows: &[(i32, String, Option<Uuid>)],
) -> Result<(), sqlx::Error> {
    let positions: Vec<i32> = rows.iter().map(|r| r.0).collect();
    let statuses: Vec<String> = rows.iter().map(|r| r.1.clone()).collect();
    let matches: Vec<Option<Uuid>> = rows.iter().map(|r| r.2).collect();
    sqlx::query!(
        "UPDATE staged_row r SET status = u.status, match_transaction_id = u.match
         FROM unnest($3::int4[], $4::text[], $5::uuid[]) AS u(position, status, match)
         WHERE r.import_id = $1 AND r.statement = $2 AND r.position = u.position",
        import_id,
        statement,
        &positions,
        &statuses,
        &matches as &[Option<Uuid>],
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Of `fingerprints`, those already imported into the account, with the
/// transaction each became.
pub async fn known_fingerprints(
    db: impl PgExecutor<'_>,
    account_id: Uuid,
    fingerprints: &[String],
) -> Result<Vec<(String, Uuid)>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT fingerprint, transaction_id FROM import_fingerprint
         WHERE account_id = $1 AND fingerprint = ANY($2)",
        account_id,
        fingerprints
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.fingerprint, r.transaction_id))
        .collect())
}

/// A staged row and an existing transaction that may be the same: same
/// account, commodity and signed amount, dated within `days` of each other.
#[derive(Debug, Clone)]
pub struct NearCandidate {
    pub position: i32,
    pub transaction_id: Uuid,
    /// Trigram similarity of the descriptions, in thousandths (0-1000).
    pub similarity: i32,
    pub days_apart: i32,
}

pub async fn near_candidates(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: i16,
    account_id: Uuid,
    commodity_id: Uuid,
    days: i32,
) -> Result<Vec<NearCandidate>, sqlx::Error> {
    sqlx::query_as!(
        NearCandidate,
        r#"SELECT DISTINCT r.position, t.id AS transaction_id,
                  round(similarity(lower(r.description), lower(t.description)) * 1000)::int4
                      AS "similarity!",
                  abs(r.date - t.date) AS "days_apart!"
           FROM staged_row r
           JOIN split s ON (r.amount < 0 AND s.from_account_id = $3
                            AND s.from_commodity_id = $4 AND s.from_quantity = -r.amount)
                        OR (r.amount > 0 AND s.to_account_id = $3
                            AND s.to_commodity_id = $4 AND s.to_quantity = r.amount)
           JOIN transaction t ON t.id = s.transaction_id
           WHERE r.import_id = $1 AND r.statement = $2
             AND t.date BETWEEN r.date - $5::int4 AND r.date + $5::int4"#,
        import_id,
        statement,
        account_id,
        commodity_id,
        days,
    )
    .fetch_all(db)
    .await
}
