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
    /// The file's bytes' SHA-256, hex.
    pub file_sha256: String,
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
    pub suggestion: Option<AccountSuggestion>,
}

impl StatementRow {
    /// The account the rows are worked out against: the statement's own,
    /// else a confident suggestion.
    pub fn effective_account(&self) -> Option<Uuid> {
        self.account_id.or_else(|| {
            self.suggestion
                .as_ref()
                .filter(|s| s.confidence >= CONFIDENT)
                .map(|s| s.account_id)
        })
    }
}

/// Confidence, per mille, from which a suggestion counts as made (design §8).
pub const CONFIDENT: i16 = 900;

/// A statement's suggested account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSuggestion {
    pub account_id: Uuid,
    pub suggester_id: Uuid,
    /// Per mille.
    pub confidence: i16,
    pub reason: String,
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
    /// `match`, `new`, `skip`, or `None` for the status's default.
    pub decision: Option<String>,
    pub set_description: Option<String>,
    pub counterparty_id: Option<Uuid>,
    /// The other side as one of your accounts (a transfer), when chosen;
    /// never with `counterparty_id`.
    pub transfer_account_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub purpose_id: Option<Uuid>,
    /// The labels set by the client (`counterparty`, `category`,
    /// `purpose`); the others are open to suggestions.
    pub chosen: Vec<String>,
    /// A transfer match's other side: a staged row, maybe in another
    /// import.
    pub pair_import_id: Option<Uuid>,
    pub pair_statement: Option<i16>,
    pub pair_position: Option<i32>,
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
    file_sha256: &str,
    format: &str,
    token_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO staged_import (id, file_name, file_sha256, format, token_id)
         VALUES ($1, $2, $3, $4, $5)",
        id,
        file_name,
        file_sha256,
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
        "SELECT id, file_name, file_sha256, format, created_at, version
         FROM staged_import WHERE id = $1",
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
        "SELECT i.id, i.file_name, i.file_sha256, i.format, i.created_at, i.version
         FROM staged_import i
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
                ledger_amount, ledger_date, skipped, suggested_account_id,
                account_suggester_id, account_confidence, account_reason
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
                    suggestion: match (
                        r.suggested_account_id,
                        r.account_suggester_id,
                        r.account_confidence,
                        r.account_reason,
                    ) {
                        (Some(account_id), Some(suggester_id), Some(confidence), Some(reason)) => {
                            Some(AccountSuggestion {
                                account_id,
                                suggester_id,
                                confidence,
                                reason,
                            })
                        }
                        _ => None,
                    },
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
                status, match_transaction_id, decision, set_description, counterparty_id,
                transfer_account_id, category_id, purpose_id, chosen, pair_import_id,
                pair_statement, pair_position
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
/// A row found in the ledger after all (a duplicate of some kind) starts
/// over at that status's default: a decision made before was about
/// something else. A skip stays.
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
        "UPDATE staged_row r SET status = u.status, match_transaction_id = u.match,
                decision = CASE WHEN r.status <> u.status
                                 AND u.status IN ('duplicate', 'near_duplicate',
                                                  'transfer_duplicate')
                                 AND r.decision IS DISTINCT FROM 'skip'
                                THEN NULL ELSE r.decision END
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

/// Staged rows whose other side may already be in the ledger as a transfer
/// (a transfer duplicate): a one-split transfer between the account and
/// another of yours, moving the same amount of the commodity the same way,
/// dated within `days`, and not yet holding a bank row of this account.
pub async fn transfer_duplicates(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: i16,
    account_id: Uuid,
    commodity_id: Uuid,
    days: i32,
) -> Result<Vec<NearCandidate>, sqlx::Error> {
    sqlx::query_as!(
        NearCandidate,
        r#"SELECT DISTINCT r.position, t.id AS transaction_id, 0 AS "similarity!",
                  abs(r.date - t.date) AS "days_apart!"
           FROM staged_row r
           JOIN split s ON s.from_commodity_id = $4 AND s.to_commodity_id = $4
                AND s.from_account_id IS NOT NULL AND s.to_account_id IS NOT NULL
                AND ((r.amount < 0 AND s.from_account_id = $3 AND s.to_account_id <> $3
                      AND s.from_quantity = -r.amount)
                  OR (r.amount > 0 AND s.to_account_id = $3 AND s.from_account_id <> $3
                      AND s.to_quantity = r.amount))
           JOIN transaction t ON t.id = s.transaction_id
           WHERE r.import_id = $1 AND r.statement = $2
             AND t.date BETWEEN r.date - $5::int4 AND r.date + $5::int4
             AND (SELECT count(*) FROM split x WHERE x.transaction_id = t.id) = 1
             AND NOT EXISTS (SELECT 1 FROM import_fingerprint f
                             WHERE f.transaction_id = t.id AND f.account_id = $3)"#,
        import_id,
        statement,
        account_id,
        commodity_id,
        days,
    )
    .fetch_all(db)
    .await
}

/// A staged row of any staged import, as transfer matching sees it.
#[derive(Debug, Clone)]
pub struct Matchable {
    pub import_id: Uuid,
    pub statement: i16,
    pub position: i32,
    /// The statement's account: its own, else a confident suggestion.
    pub account_id: Option<Uuid>,
    pub commodity_id: Uuid,
    pub date: Date,
    pub amount: Decimal,
    pub status: String,
    pub match_transaction_id: Option<Uuid>,
    pub decision: Option<String>,
    /// Whether you chose the other side, and what: an account, a
    /// counterparty, or none.
    pub chose_side: bool,
    pub transfer_account_id: Option<Uuid>,
    /// The suggested other side, if it's an account, and how sure.
    pub suggested_account_id: Option<Uuid>,
    pub suggested_confidence: Option<i16>,
    pub pair_import_id: Option<Uuid>,
    pub pair_statement: Option<i16>,
    pub pair_position: Option<i32>,
}

/// Every staged row, in every staged import.
pub async fn matchable(db: impl PgExecutor<'_>) -> Result<Vec<Matchable>, sqlx::Error> {
    sqlx::query_as!(
        Matchable,
        r#"SELECT r.import_id, r.statement, r.position,
                  coalesce(st.account_id,
                           CASE WHEN st.account_confidence >= 900
                                THEN st.suggested_account_id END) AS account_id,
                  st.commodity_id, r.date, r.amount, r.status, r.match_transaction_id,
                  r.decision,
                  'counterparty' = ANY(r.chosen) AS "chose_side!",
                  r.transfer_account_id,
                  sg.account_id AS "suggested_account_id?",
                  sg.confidence AS "suggested_confidence?",
                  r.pair_import_id, r.pair_statement, r.pair_position
           FROM staged_row r
           JOIN staged_statement st
             ON st.import_id = r.import_id AND st.position = r.statement
           LEFT JOIN staged_suggestion sg
             ON sg.import_id = r.import_id AND sg.statement = r.statement
            AND sg.position = r.position AND sg.label = 'counterparty'
           ORDER BY r.import_id, r.statement, r.position"#
    )
    .fetch_all(db)
    .await
}

/// A staged row, by its import, statement and position.
pub type RowKey = (Uuid, i16, i32);

/// A staged row as its transfer match shows it.
#[derive(Debug, Clone)]
pub struct PairRow {
    pub import_id: Uuid,
    pub statement: i16,
    pub position: i32,
    pub file_name: Option<String>,
    pub file_sha256: String,
    /// The statement's account: its own, else a confident suggestion.
    pub account_id: Option<Uuid>,
    pub date: Date,
    pub amount: Decimal,
    pub description: String,
    pub set_description: Option<String>,
    pub fingerprint: String,
    pub raw: Value,
}

/// These rows, in any staged imports.
pub async fn pair_rows(
    db: impl PgExecutor<'_>,
    keys: &[RowKey],
) -> Result<Vec<PairRow>, sqlx::Error> {
    let imports: Vec<Uuid> = keys.iter().map(|k| k.0).collect();
    let statements: Vec<i16> = keys.iter().map(|k| k.1).collect();
    let positions: Vec<i32> = keys.iter().map(|k| k.2).collect();
    sqlx::query_as!(
        PairRow,
        r#"SELECT r.import_id, r.statement, r.position, i.file_name, i.file_sha256,
                  coalesce(st.account_id,
                           CASE WHEN st.account_confidence >= 900
                                THEN st.suggested_account_id END) AS account_id,
                  r.date, r.amount, r.description, r.set_description, r.fingerprint, r.raw
           FROM unnest($1::uuid[], $2::int2[], $3::int4[]) AS u(i, s, p)
           JOIN staged_row r
             ON r.import_id = u.i AND r.statement = u.s AND r.position = u.p
           JOIN staged_statement st
             ON st.import_id = r.import_id AND st.position = r.statement
           JOIN staged_import i ON i.id = r.import_id"#,
        &imports,
        &statements,
        &positions,
    )
    .fetch_all(db)
    .await
}

/// Sets rows' status and transfer match (`None`: none).
pub async fn set_pairs(
    db: impl PgExecutor<'_>,
    rows: &[(RowKey, String, Option<RowKey>)],
) -> Result<(), sqlx::Error> {
    let imports: Vec<Uuid> = rows.iter().map(|r| r.0.0).collect();
    let statements: Vec<i16> = rows.iter().map(|r| r.0.1).collect();
    let positions: Vec<i32> = rows.iter().map(|r| r.0.2).collect();
    let statuses: Vec<String> = rows.iter().map(|r| r.1.clone()).collect();
    let pair_imports: Vec<Option<Uuid>> = rows.iter().map(|r| r.2.map(|p| p.0)).collect();
    let pair_statements: Vec<Option<i16>> = rows.iter().map(|r| r.2.map(|p| p.1)).collect();
    let pair_positions: Vec<Option<i32>> = rows.iter().map(|r| r.2.map(|p| p.2)).collect();
    sqlx::query!(
        "UPDATE staged_row r SET status = u.status, match_transaction_id = NULL,
                                 pair_import_id = u.pi, pair_statement = u.ps,
                                 pair_position = u.pp
         FROM unnest($1::uuid[], $2::int2[], $3::int4[], $4::text[], $5::uuid[], $6::int2[],
                     $7::int4[]) AS u(i, s, p, status, pi, ps, pp)
         WHERE r.import_id = u.i AND r.statement = u.s AND r.position = u.p",
        &imports,
        &statements,
        &positions,
        &statuses,
        &pair_imports as &[Option<Uuid>],
        &pair_statements as &[Option<i16>],
        &pair_positions as &[Option<i32>],
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The other side of a transfer match takes the same decision: both rows
/// are one transfer, or neither is. A skipped row keeps its skip.
pub async fn set_pair_decision(
    db: impl PgExecutor<'_>,
    key: RowKey,
    decision: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE staged_row SET decision = $4
         WHERE import_id = $1 AND statement = $2 AND position = $3
           AND status = 'transfer_match' AND decision IS DISTINCT FROM 'skip'",
        key.0,
        key.1,
        key.2,
        decision,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// One imported row's ID in an account, and the transaction it became.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    pub account_id: Uuid,
    pub fingerprint: String,
    pub transaction_id: Uuid,
}

/// Of `items`, those whose transaction has no split on the account.
pub async fn fingerprints_off_account(
    db: impl PgExecutor<'_>,
    items: &[Fingerprint],
) -> Result<Vec<usize>, sqlx::Error> {
    let accounts: Vec<Uuid> = items.iter().map(|f| f.account_id).collect();
    let transactions: Vec<Uuid> = items.iter().map(|f| f.transaction_id).collect();
    let rows = sqlx::query_scalar!(
        r#"SELECT (u.n - 1)::int8 AS "n!" FROM unnest($1::uuid[], $2::uuid[])
               WITH ORDINALITY AS u(account_id, transaction_id, n)
           WHERE NOT EXISTS (SELECT 1 FROM split s WHERE s.transaction_id = u.transaction_id
                             AND u.account_id IN (s.from_account_id, s.to_account_id))
           ORDER BY u.n"#,
        &accounts,
        &transactions,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|n| usize::try_from(n).ok())
        .collect())
}

/// Records fingerprints, leaving any already recorded alone; returns how
/// many were new.
pub async fn add_fingerprints(
    db: impl PgExecutor<'_>,
    items: &[Fingerprint],
) -> Result<u64, sqlx::Error> {
    let accounts: Vec<Uuid> = items.iter().map(|f| f.account_id).collect();
    let fps: Vec<String> = items.iter().map(|f| f.fingerprint.clone()).collect();
    let transactions: Vec<Uuid> = items.iter().map(|f| f.transaction_id).collect();
    let r = sqlx::query!(
        "INSERT INTO import_fingerprint (account_id, fingerprint, transaction_id)
         SELECT * FROM unnest($1::uuid[], $2::text[], $3::uuid[])
         ON CONFLICT (account_id, fingerprint) DO NOTHING",
        &accounts,
        &fps,
        &transactions,
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

/// What's recorded for these (account, fingerprint) pairs.
pub async fn recorded_fingerprints(
    db: impl PgExecutor<'_>,
    keys: &[(Uuid, String)],
) -> Result<Vec<Fingerprint>, sqlx::Error> {
    let accounts: Vec<Uuid> = keys.iter().map(|k| k.0).collect();
    let fps: Vec<String> = keys.iter().map(|k| k.1.clone()).collect();
    sqlx::query_as!(
        Fingerprint,
        "SELECT f.account_id, f.fingerprint, f.transaction_id FROM import_fingerprint f
         JOIN unnest($1::uuid[], $2::text[]) AS u(account_id, fingerprint)
           ON u.account_id = f.account_id AND u.fingerprint = f.fingerprint",
        &accounts,
        &fps,
    )
    .fetch_all(db)
    .await
}

/// One row's decision and labels, replacing what it had.
pub async fn set_decision(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    r: &RowRow,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        "UPDATE staged_row SET decision = $4, set_description = $5, counterparty_id = $6,
                               category_id = $7, purpose_id = $8, chosen = $9,
                               transfer_account_id = $10
         WHERE import_id = $1 AND statement = $2 AND position = $3",
        import_id,
        r.statement,
        r.position,
        r.decision,
        r.set_description,
        r.counterparty_id,
        r.category_id,
        r.purpose_id,
        &r.chosen,
        r.transfer_account_id,
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// One bank row as one file had it.
#[derive(Debug, Clone)]
pub struct ImportedRow {
    pub account_id: Uuid,
    pub fingerprint: String,
    pub file_sha256: String,
    pub raw: Value,
}

/// Records rows as files had them; a file already recorded for a row adds
/// nothing. Each row's fingerprint must be recorded first.
pub async fn add_imported_rows(
    db: impl PgExecutor<'_>,
    rows: &[ImportedRow],
) -> Result<(), sqlx::Error> {
    let accounts: Vec<Uuid> = rows.iter().map(|r| r.account_id).collect();
    let fps: Vec<String> = rows.iter().map(|r| r.fingerprint.clone()).collect();
    let files: Vec<String> = rows.iter().map(|r| r.file_sha256.clone()).collect();
    let raw: Vec<Value> = rows.iter().map(|r| r.raw.clone()).collect();
    sqlx::query!(
        "INSERT INTO imported_row (account_id, fingerprint, file_sha256, raw)
         SELECT * FROM unnest($1::uuid[], $2::text[], $3::text[], $4::jsonb[])
         ON CONFLICT DO NOTHING",
        &accounts,
        &fps,
        &files,
        &raw,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The bank rows recorded for a transaction, each with the files it came
/// in (none for rows recorded another way, e.g. by a migration).
#[derive(Debug, Clone)]
pub struct RowOfTransaction {
    pub account_id: Uuid,
    pub fingerprint: String,
    pub file_sha256: Option<String>,
    pub raw: Option<Value>,
    pub imported_at: Option<OffsetDateTime>,
}

pub async fn rows_of_transaction(
    db: impl PgExecutor<'_>,
    transaction_id: Uuid,
) -> Result<Vec<RowOfTransaction>, sqlx::Error> {
    sqlx::query_as!(
        RowOfTransaction,
        r#"SELECT f.account_id, f.fingerprint, r.file_sha256 AS "file_sha256?",
                  r.raw AS "raw?", r.imported_at AS "imported_at?"
           FROM import_fingerprint f
           LEFT JOIN imported_row r USING (account_id, fingerprint)
           WHERE f.transaction_id = $1
           ORDER BY f.account_id, f.fingerprint, r.imported_at"#,
        transaction_id
    )
    .fetch_all(db)
    .await
}

// --------------------------------------------------------- Suggestions ----

/// The exact system that made suggestions (design §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggesterRow {
    pub id: Uuid,
    /// `rule`, `local` or `remote`.
    pub kind: String,
    pub name: String,
    pub version: String,
    pub details: Value,
}

/// The suggester with this identity, recorded the first time it's seen.
pub async fn suggester(
    db: &mut sqlx::PgConnection,
    kind: &str,
    name: &str,
    version: &str,
    details: &Value,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query!(
        "INSERT INTO suggester (id, kind, name, version, details) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (kind, name, version, details) DO NOTHING",
        Uuid::now_v7(),
        kind,
        name,
        version,
        details,
    )
    .execute(&mut *db)
    .await?;
    sqlx::query_scalar!(
        "SELECT id FROM suggester WHERE kind = $1 AND name = $2 AND version = $3 AND details = $4",
        kind,
        name,
        version,
        details,
    )
    .fetch_one(&mut *db)
    .await
}

pub async fn suggesters(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<SuggesterRow>, sqlx::Error> {
    sqlx::query_as!(
        SuggesterRow,
        "SELECT id, kind, name, version, details FROM suggester WHERE id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await
}

/// A suggested label for a staged row; `value` `None` suggests "none",
/// unless `account_id` suggests one of your accounts as the other side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionRow {
    pub statement: i16,
    pub position: i32,
    /// `counterparty`, `category` or `purpose`.
    pub label: String,
    pub value: Option<Uuid>,
    /// For the counterparty: one of your accounts (a transfer).
    pub account_id: Option<Uuid>,
    pub suggester_id: Uuid,
    /// Per mille.
    pub confidence: i16,
    pub reason: String,
}

/// The suggestions for these rows of an import.
pub async fn suggestions(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    rows: &[(i16, i32)],
) -> Result<Vec<SuggestionRow>, sqlx::Error> {
    let statements: Vec<i16> = rows.iter().map(|r| r.0).collect();
    let positions: Vec<i32> = rows.iter().map(|r| r.1).collect();
    sqlx::query_as!(
        SuggestionRow,
        r#"SELECT s.statement, s.position, s.label,
                  coalesce(s.counterparty_id, s.category_id, s.purpose_id) AS value,
                  s.account_id, s.suggester_id, s.confidence, s.reason
           FROM staged_suggestion s
           JOIN unnest($2::int2[], $3::int4[]) AS u(statement, position)
             ON u.statement = s.statement AND u.position = s.position
           WHERE s.import_id = $1
           ORDER BY s.statement, s.position, s.label"#,
        import_id,
        &statements,
        &positions,
    )
    .fetch_all(db)
    .await
}

/// Replaces the suggestions for the rows of one statement with these.
pub async fn set_suggestions(
    db: &mut sqlx::PgConnection,
    import_id: Uuid,
    statement: i16,
    items: &[SuggestionRow],
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM staged_suggestion WHERE import_id = $1 AND statement = $2",
        import_id,
        statement
    )
    .execute(&mut *db)
    .await?;
    let positions: Vec<i32> = items.iter().map(|s| s.position).collect();
    let labels: Vec<String> = items.iter().map(|s| s.label.clone()).collect();
    let values: Vec<Option<Uuid>> = items.iter().map(|s| s.value).collect();
    let accounts: Vec<Option<Uuid>> = items.iter().map(|s| s.account_id).collect();
    let suggesters: Vec<Uuid> = items.iter().map(|s| s.suggester_id).collect();
    let confidences: Vec<i16> = items.iter().map(|s| s.confidence).collect();
    let reasons: Vec<String> = items.iter().map(|s| s.reason.clone()).collect();
    sqlx::query!(
        "INSERT INTO staged_suggestion (import_id, statement, position, label, counterparty_id,
                                        category_id, purpose_id, account_id, suggester_id,
                                        confidence, reason)
         SELECT $1, $2, u.position, u.label,
                CASE WHEN u.label = 'counterparty' THEN u.value END,
                CASE WHEN u.label = 'category' THEN u.value END,
                CASE WHEN u.label = 'purpose' THEN u.value END,
                u.account, u.suggester, u.confidence, u.reason
         FROM unnest($3::int4[], $4::text[], $5::uuid[], $6::uuid[], $7::uuid[], $8::int2[],
                     $9::text[])
              AS u(position, label, value, account, suggester, confidence, reason)",
        import_id,
        statement,
        &positions,
        &labels,
        &values as &[Option<Uuid>],
        &accounts as &[Option<Uuid>],
        &suggesters,
        &confidences,
        &reasons,
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

/// Sets or clears a statement's suggested account.
pub async fn set_account_suggestion(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: i16,
    s: Option<&AccountSuggestion>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE staged_statement SET suggested_account_id = $3, account_suggester_id = $4,
                                     account_confidence = $5, account_reason = $6
         WHERE import_id = $1 AND position = $2",
        import_id,
        statement,
        s.map(|s| s.account_id),
        s.map(|s| s.suggester_id),
        s.map(|s| s.confidence),
        s.map(|s| s.reason.clone()),
    )
    .execute(db)
    .await?;
    Ok(())
}

/// What a suggestion became on commit.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub suggester_id: Uuid,
    /// `account`, `counterparty`, `category` or `purpose`.
    pub label: String,
    pub suggested: Option<Uuid>,
    pub confidence: i16,
    pub committed: Option<Uuid>,
    pub transaction_id: Option<Uuid>,
}

pub async fn add_outcomes(db: impl PgExecutor<'_>, items: &[Outcome]) -> Result<(), sqlx::Error> {
    let ids: Vec<Uuid> = items.iter().map(|_| Uuid::now_v7()).collect();
    let suggesters: Vec<Uuid> = items.iter().map(|o| o.suggester_id).collect();
    let labels: Vec<String> = items.iter().map(|o| o.label.clone()).collect();
    let suggested: Vec<Option<Uuid>> = items.iter().map(|o| o.suggested).collect();
    let confidences: Vec<i16> = items.iter().map(|o| o.confidence).collect();
    let committed: Vec<Option<Uuid>> = items.iter().map(|o| o.committed).collect();
    let transactions: Vec<Option<Uuid>> = items.iter().map(|o| o.transaction_id).collect();
    sqlx::query!(
        "INSERT INTO suggestion_outcome (id, suggester_id, label, suggested, confidence,
                                         committed, transaction_id)
         SELECT * FROM unnest($1::uuid[], $2::uuid[], $3::text[], $4::uuid[], $5::int2[],
                              $6::uuid[], $7::uuid[])",
        &ids,
        &suggesters,
        &labels,
        &suggested as &[Option<Uuid>],
        &confidences,
        &committed as &[Option<Uuid>],
        &transactions as &[Option<Uuid>],
    )
    .execute(db)
    .await?;
    Ok(())
}
