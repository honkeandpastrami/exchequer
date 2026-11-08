//! Staged imports (design §6): uploaded bank files waiting for review. A
//! file has statements (one per account), and a statement has rows, each
//! with its label: new, a duplicate of an existing transaction, and so on.

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
    /// Whether `account_id` came from ticking the suggestion as reviewed.
    pub reviewed: bool,
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

/// Confidence, per mille, from which a suggestion counts as made
/// ([`suggest::CONFIDENT`]), as stored.
pub const CONFIDENT: i16 = {
    assert!(suggest::CONFIDENT <= 1000);
    suggest::CONFIDENT as i16
};

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
    /// `new`, `duplicate`, `possible_duplicate`, `transfer_pair` or
    /// `no_account`.
    pub status: String,
    pub match_transaction_id: Option<Uuid>,
    /// How a duplicate's match was found: `bank_id`, `transfer` or `amount`.
    pub match_kind: Option<String>,
    /// `add`, `add_separately`, `dont_add`, or `None` for the label's
    /// default.
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
    /// A transfer pair's other row: a staged row, maybe in another
    /// import.
    pub pair_import_id: Option<Uuid>,
    pub pair_statement: Option<i16>,
    pub pair_position: Option<i32>,
    /// A transfer conflict's other row: a staged row, maybe in another
    /// import.
    pub conflict_import_id: Option<Uuid>,
    pub conflict_statement: Option<i16>,
    pub conflict_position: Option<i32>,
    /// What the row showed when you ticked it as reviewed; `None`: not
    /// ticked.
    pub reviewed: Option<Value>,
    /// What matching found: days between the row and its match, and how
    /// alike their descriptions are (thousandths).
    pub match_days: Option<i16>,
    pub match_similarity: Option<i16>,
    /// Tag names for the new transaction.
    pub tags: Vec<String>,
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
                account_suggester_id, account_confidence, account_reason, reviewed
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
                    reviewed: r.reviewed,
                },
            )
        })
        .collect())
}

/// Sets a statement's own account; `reviewed`: from ticking its suggestion.
pub async fn set_account(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: i16,
    account_id: Option<Uuid>,
    reviewed: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE staged_statement SET account_id = $3, reviewed = $4
         WHERE import_id = $1 AND position = $2",
        import_id,
        statement,
        account_id,
        reviewed
    )
    .execute(db)
    .await?;
    Ok(())
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
                status, match_transaction_id, match_kind, decision, set_description,
                counterparty_id, transfer_account_id, category_id, purpose_id, chosen,
                pair_import_id, pair_statement, pair_position, conflict_import_id,
                conflict_statement, conflict_position, reviewed, match_days,
                match_similarity, tags
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

/// A row's label, as [`set_statuses`] sets it.
#[derive(Debug, Clone)]
pub struct StatusUpdate {
    pub position: i32,
    pub status: String,
    /// The transaction it matches, and how it was found.
    pub transaction: Option<Uuid>,
    pub kind: Option<String>,
    pub days: Option<i16>,
    pub similarity: Option<i16>,
}

/// Sets each row's label and match, by position, in one statement. A row
/// whose label or match changes goes back to the new label's default, and
/// a row that's no longer new loses its transfer pair.
pub async fn set_statuses(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    statement: i16,
    rows: &[StatusUpdate],
) -> Result<(), sqlx::Error> {
    let positions: Vec<i32> = rows.iter().map(|r| r.position).collect();
    let statuses: Vec<String> = rows.iter().map(|r| r.status.clone()).collect();
    let matches: Vec<Option<Uuid>> = rows.iter().map(|r| r.transaction).collect();
    let kinds: Vec<Option<String>> = rows.iter().map(|r| r.kind.clone()).collect();
    let days: Vec<Option<i16>> = rows.iter().map(|r| r.days).collect();
    let similarity: Vec<Option<i16>> = rows.iter().map(|r| r.similarity).collect();
    sqlx::query!(
        "UPDATE staged_row r SET status = u.status, match_transaction_id = u.match,
                match_kind = u.kind, match_days = u.days, match_similarity = u.similarity,
                decision = CASE WHEN r.status <> u.status
                                  OR r.match_transaction_id IS DISTINCT FROM u.match
                                THEN NULL ELSE r.decision END,
                pair_import_id = CASE WHEN u.status IN ('new', 'transfer_pair')
                                      THEN r.pair_import_id END,
                pair_statement = CASE WHEN u.status IN ('new', 'transfer_pair')
                                      THEN r.pair_statement END,
                pair_position = CASE WHEN u.status IN ('new', 'transfer_pair')
                                     THEN r.pair_position END,
                conflict_import_id = CASE WHEN u.status IN ('new', 'transfer_pair')
                                          THEN r.conflict_import_id END,
                conflict_statement = CASE WHEN u.status IN ('new', 'transfer_pair')
                                          THEN r.conflict_statement END,
                conflict_position = CASE WHEN u.status IN ('new', 'transfer_pair')
                                         THEN r.conflict_position END
         FROM unnest($3::int4[], $4::text[], $5::uuid[], $6::text[], $7::int2[], $8::int2[])
              AS u(position, status, match, kind, days, similarity)
         WHERE r.import_id = $1 AND r.statement = $2 AND r.position = u.position",
        import_id,
        statement,
        &positions,
        &statuses,
        &matches as &[Option<Uuid>],
        &kinds as &[Option<String>],
        &days as &[Option<i16>],
        &similarity as &[Option<i16>],
    )
    .execute(db)
    .await?;
    Ok(())
}

/// A row ID already imported into an account: the transaction it became,
/// and the row's fields as each file had them.
#[derive(Debug, Clone)]
pub struct KnownFingerprint {
    /// The ID asked about.
    pub asked: String,
    /// The ID recorded: the one asked about, or it with a suffix (`~2`) for
    /// a row added again after it changed.
    pub fingerprint: String,
    pub transaction_id: Uuid,
    pub raws: Vec<Value>,
}

/// Of `fingerprints`, those already imported into the account, suffixed
/// ones included.
pub async fn known_fingerprints(
    db: impl PgExecutor<'_>,
    account_id: Uuid,
    fingerprints: &[String],
) -> Result<Vec<KnownFingerprint>, sqlx::Error> {
    sqlx::query_as!(
        KnownFingerprint,
        r#"SELECT u.fp AS "asked!", f.fingerprint, f.transaction_id,
                  coalesce(array_agg(r.raw ORDER BY r.imported_at)
                               FILTER (WHERE r.raw IS NOT NULL), '{}') AS "raws!"
           FROM unnest($2::text[]) AS u(fp)
           JOIN import_fingerprint f
             ON f.account_id = $1
            AND (f.fingerprint = u.fp OR starts_with(f.fingerprint, u.fp || '~'))
           LEFT JOIN imported_row r
             ON r.account_id = f.account_id AND r.fingerprint = f.fingerprint
           GROUP BY u.fp, f.fingerprint, f.transaction_id
           ORDER BY u.fp, f.fingerprint"#,
        account_id,
        fingerprints
    )
    .fetch_all(db)
    .await
}

/// A row ID for a row added again: `fingerprint` if the account has no such
/// ID yet, else it with the first free suffix (`~2`, `~3`…), within the
/// column's 200 characters.
pub async fn free_fingerprint(
    db: impl PgExecutor<'_>,
    account_id: Uuid,
    fingerprint: &str,
) -> Result<String, sqlx::Error> {
    let taken: Vec<String> = sqlx::query_scalar!(
        "SELECT fingerprint FROM import_fingerprint
         WHERE account_id = $1 AND (fingerprint = $2 OR starts_with(fingerprint, $2 || '~'))",
        account_id,
        fingerprint
    )
    .fetch_all(db)
    .await?;
    if !taken.iter().any(|t| t == fingerprint) {
        return Ok(fingerprint.to_owned());
    }
    let mut n = 2_u32;
    loop {
        let suffix = format!("~{n}");
        let room = 200_usize.saturating_sub(suffix.len());
        let base: String = fingerprint.chars().take(room).collect();
        let candidate = format!("{base}{suffix}");
        if !taken.contains(&candidate) {
            return Ok(candidate);
        }
        n += 1;
    }
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
/// (a recorded transfer): a one-split transfer between the account and
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
    pub pair_import_id: Option<Uuid>,
    pub pair_statement: Option<i16>,
    pub pair_position: Option<i32>,
    pub conflict_import_id: Option<Uuid>,
    pub conflict_statement: Option<i16>,
    pub conflict_position: Option<i32>,
    /// Split into parts before commit.
    pub split: bool,
}

/// Every staged row, in every staged import.
pub async fn matchable(db: impl PgExecutor<'_>) -> Result<Vec<Matchable>, sqlx::Error> {
    sqlx::query_as!(
        Matchable,
        r#"SELECT r.import_id, r.statement, r.position,
                  coalesce(st.account_id,
                           CASE WHEN st.account_confidence >= $1
                                THEN st.suggested_account_id END) AS account_id,
                  st.commodity_id, r.date, r.amount, r.status, r.match_transaction_id,
                  r.decision,
                  'counterparty' = ANY(r.chosen) AS "chose_side!",
                  r.transfer_account_id,
                  r.pair_import_id, r.pair_statement, r.pair_position,
                  r.conflict_import_id, r.conflict_statement, r.conflict_position,
                  EXISTS (SELECT 1 FROM staged_split sp
                          WHERE sp.import_id = r.import_id AND sp.statement = r.statement
                            AND sp.position = r.position) AS "split!"
           FROM staged_row r
           JOIN staged_statement st
             ON st.import_id = r.import_id AND st.position = r.statement
           ORDER BY r.import_id, r.statement, r.position"#,
        CONFIDENT
    )
    .fetch_all(db)
    .await
}

/// A staged row, by its import, statement and position.
pub type RowKey = (Uuid, i16, i32);

/// A staged row as its transfer pair shows it.
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
    pub tags: Vec<String>,
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
                           CASE WHEN st.account_confidence >= $4
                                THEN st.suggested_account_id END) AS account_id,
                  r.date, r.amount, r.description, r.set_description, r.fingerprint, r.raw,
                  r.tags
           FROM unnest($1::uuid[], $2::int2[], $3::int4[]) AS u(i, s, p)
           JOIN staged_row r
             ON r.import_id = u.i AND r.statement = u.s AND r.position = u.p
           JOIN staged_statement st
             ON st.import_id = r.import_id AND st.position = r.statement
           JOIN staged_import i ON i.id = r.import_id"#,
        &imports,
        &statements,
        &positions,
        CONFIDENT,
    )
    .fetch_all(db)
    .await
}

/// What transfer matching sets on a new row: its label, transfer pair and
/// transfer conflict (`None`: none).
pub type PairUpdate = (RowKey, String, Option<RowKey>, Option<RowKey>);

/// Sets new rows' label, transfer pair and transfer conflict. A row whose
/// label, pair or conflict changes goes back to the new label's default.
pub async fn set_pairs(db: impl PgExecutor<'_>, rows: &[PairUpdate]) -> Result<(), sqlx::Error> {
    let imports: Vec<Uuid> = rows.iter().map(|r| r.0.0).collect();
    let statements: Vec<i16> = rows.iter().map(|r| r.0.1).collect();
    let positions: Vec<i32> = rows.iter().map(|r| r.0.2).collect();
    let statuses: Vec<String> = rows.iter().map(|r| r.1.clone()).collect();
    let pair_imports: Vec<Option<Uuid>> = rows.iter().map(|r| r.2.map(|p| p.0)).collect();
    let pair_statements: Vec<Option<i16>> = rows.iter().map(|r| r.2.map(|p| p.1)).collect();
    let pair_positions: Vec<Option<i32>> = rows.iter().map(|r| r.2.map(|p| p.2)).collect();
    let conflict_imports: Vec<Option<Uuid>> = rows.iter().map(|r| r.3.map(|c| c.0)).collect();
    let conflict_statements: Vec<Option<i16>> = rows.iter().map(|r| r.3.map(|c| c.1)).collect();
    let conflict_positions: Vec<Option<i32>> = rows.iter().map(|r| r.3.map(|c| c.2)).collect();
    sqlx::query!(
        "UPDATE staged_row r SET status = u.status, pair_import_id = u.pi,
                                 pair_statement = u.ps, pair_position = u.pp,
                                 conflict_import_id = u.ci, conflict_statement = u.cs,
                                 conflict_position = u.cp,
                                 decision = CASE WHEN r.status <> u.status
                                     OR (r.pair_import_id, r.pair_statement, r.pair_position)
                                        IS DISTINCT FROM (u.pi, u.ps, u.pp)
                                     OR (r.conflict_import_id, r.conflict_statement,
                                         r.conflict_position)
                                        IS DISTINCT FROM (u.ci, u.cs, u.cp)
                                     THEN NULL ELSE r.decision END
         FROM unnest($1::uuid[], $2::int2[], $3::int4[], $4::text[], $5::uuid[], $6::int2[],
                     $7::int4[], $8::uuid[], $9::int2[], $10::int4[])
              AS u(i, s, p, status, pi, ps, pp, ci, cs, cp)
         WHERE r.import_id = u.i AND r.statement = u.s AND r.position = u.p
           AND r.status IN ('new', 'transfer_pair')",
        &imports,
        &statements,
        &positions,
        &statuses,
        &pair_imports as &[Option<Uuid>],
        &pair_statements as &[Option<i16>],
        &pair_positions as &[Option<i32>],
        &conflict_imports as &[Option<Uuid>],
        &conflict_statements as &[Option<i16>],
        &conflict_positions as &[Option<i32>],
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The other row of a transfer pair takes the same choice: the pair is one
/// transfer, two transactions, or not added.
pub async fn set_pair_decision(
    db: impl PgExecutor<'_>,
    key: RowKey,
    decision: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE staged_row SET decision = $4
         WHERE import_id = $1 AND statement = $2 AND position = $3
           AND status = 'transfer_pair'",
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

/// One row's decision, labels and tags, replacing what it had.
pub async fn set_decision(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    r: &RowRow,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        "UPDATE staged_row SET decision = $4, set_description = $5, counterparty_id = $6,
                               category_id = $7, purpose_id = $8, chosen = $9,
                               transfer_account_id = $10, tags = $11
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
        &r.tags,
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// You chose one of your accounts as a row's other side.
pub async fn choose_transfer(
    db: impl PgExecutor<'_>,
    key: RowKey,
    account_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE staged_row SET transfer_account_id = $4, counterparty_id = NULL,
                               chosen = CASE WHEN 'counterparty' = ANY(chosen) THEN chosen
                                             ELSE chosen || 'counterparty'::text END
         WHERE import_id = $1 AND statement = $2 AND position = $3",
        key.0,
        key.1,
        key.2,
        account_id,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Ticks rows as reviewed, each with what it showed, or unticks them
/// (`None`).
pub async fn set_reviewed(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
    rows: &[((i16, i32), Option<Value>)],
) -> Result<(), sqlx::Error> {
    let statements: Vec<i16> = rows.iter().map(|r| r.0.0).collect();
    let positions: Vec<i32> = rows.iter().map(|r| r.0.1).collect();
    let seen: Vec<Option<Value>> = rows.iter().map(|r| r.1.clone()).collect();
    sqlx::query!(
        "UPDATE staged_row r SET reviewed = u.seen
         FROM unnest($2::int2[], $3::int4[], $4::jsonb[]) AS u(s, p, seen)
         WHERE r.import_id = $1 AND r.statement = u.s AND r.position = u.p",
        import_id,
        &statements,
        &positions,
        &seen as &[Option<Value>],
    )
    .execute(db)
    .await?;
    Ok(())
}

/// One part of a staged row split before commit: a split of the new
/// transaction between the statement's account and `counterparty_id` or
/// `transfer_account_id` (neither: outside, unknown who).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSplit {
    pub statement: i16,
    pub position: i32,
    /// Unsigned: the row's sign says which way the money goes.
    pub amount: Decimal,
    pub counterparty_id: Option<Uuid>,
    pub transfer_account_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub purpose_id: Option<Uuid>,
    pub description: Option<String>,
}

/// Every split row's parts in an import, in order.
pub async fn splits(
    db: impl PgExecutor<'_>,
    import_id: Uuid,
) -> Result<Vec<StagedSplit>, sqlx::Error> {
    sqlx::query_as!(
        StagedSplit,
        "SELECT statement, position, amount, counterparty_id, transfer_account_id,
                category_id, purpose_id, description
         FROM staged_split WHERE import_id = $1
         ORDER BY statement, position, part",
        import_id,
    )
    .fetch_all(db)
    .await
}

/// Replaces a row's parts with `parts` (none: the row isn't split).
pub async fn set_splits(
    db: &mut sqlx::PgConnection,
    import_id: Uuid,
    statement: i16,
    position: i32,
    parts: &[StagedSplit],
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM staged_split WHERE import_id = $1 AND statement = $2 AND position = $3",
        import_id,
        statement,
        position,
    )
    .execute(&mut *db)
    .await?;
    for (part, s) in (0_i16..).zip(parts) {
        sqlx::query!(
            "INSERT INTO staged_split (import_id, statement, position, part, amount,
                                       counterparty_id, transfer_account_id, category_id,
                                       purpose_id, description)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            import_id,
            statement,
            position,
            part,
            s.amount,
            s.counterparty_id,
            s.transfer_account_id,
            s.category_id,
            s.purpose_id,
            s.description,
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(())
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
    /// `rule`, `local`, `remote` or `match`.
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
    /// For a `decision`: what was suggested, and what was committed.
    pub decisions: Option<(String, String)>,
}

pub async fn add_outcomes(db: impl PgExecutor<'_>, items: &[Outcome]) -> Result<(), sqlx::Error> {
    let ids: Vec<Uuid> = items.iter().map(|_| Uuid::now_v7()).collect();
    let suggesters: Vec<Uuid> = items.iter().map(|o| o.suggester_id).collect();
    let labels: Vec<String> = items.iter().map(|o| o.label.clone()).collect();
    let suggested: Vec<Option<Uuid>> = items.iter().map(|o| o.suggested).collect();
    let confidences: Vec<i16> = items.iter().map(|o| o.confidence).collect();
    let committed: Vec<Option<Uuid>> = items.iter().map(|o| o.committed).collect();
    let transactions: Vec<Option<Uuid>> = items.iter().map(|o| o.transaction_id).collect();
    let suggested_decisions: Vec<Option<String>> = items
        .iter()
        .map(|o| o.decisions.as_ref().map(|d| d.0.clone()))
        .collect();
    let committed_decisions: Vec<Option<String>> = items
        .iter()
        .map(|o| o.decisions.as_ref().map(|d| d.1.clone()))
        .collect();
    sqlx::query!(
        "INSERT INTO suggestion_outcome (id, suggester_id, label, suggested, confidence,
                                         committed, transaction_id, suggested_decision,
                                         committed_decision)
         SELECT * FROM unnest($1::uuid[], $2::uuid[], $3::text[], $4::uuid[], $5::int2[],
                              $6::uuid[], $7::uuid[], $8::text[], $9::text[])",
        &ids,
        &suggesters,
        &labels,
        &suggested as &[Option<Uuid>],
        &confidences,
        &committed as &[Option<Uuid>],
        &transactions as &[Option<Uuid>],
        &suggested_decisions as &[Option<String>],
        &committed_decisions as &[Option<String>],
    )
    .execute(db)
    .await?;
    Ok(())
}
