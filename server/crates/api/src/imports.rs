//! `/api/v1/imports` (design §6): upload a bank file, see what each of its
//! rows would become, fix what needs fixing, then commit it. The file itself
//! isn't kept, only its parsed statements and rows, until the import is
//! committed or discarded.

use std::collections::{HashMap, HashSet};

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use domain::{AccountNumber, Amount};
use import::fingerprint::fingerprints;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use store::audit::Action;
use store::imports::{ImportRow, NearCandidate, RowRow, StatementRow};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::audit;
use crate::conventions::{IfMatch, Page, PageParams, etag};
use crate::problem::{Json, Path, Problem, Query};

/// Bank files bigger than this are refused.
const MAX_FILE: usize = 10 * 1024 * 1024;
/// Longest bank ID a row may have (the fingerprint column's limit).
const MAX_ID: usize = 200;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, upload))
        .routes(routes!(get, discard))
        .routes(routes!(rows))
        .routes(routes!(set_statement))
        .layer(DefaultBodyLimit::max(MAX_FILE))
}

// ------------------------------------------------------------- Schemas ----

/// What a row would become.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RowStatus {
    /// A new transaction.
    New,
    /// Already imported: the same row (by the bank's ID, or its date,
    /// description and amount) is in the account.
    Duplicate,
    /// Probably already in the account under a slightly different form.
    NearDuplicate,
    /// The other side of a transfer already recorded from another account.
    TransferMatch,
    /// The statement has no account yet.
    NeedsAccount,
}

impl RowStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Duplicate => "duplicate",
            Self::NearDuplicate => "near_duplicate",
            Self::TransferMatch => "transfer_match",
            Self::NeedsAccount => "needs_account",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        Ok(match s {
            "new" => Self::New,
            "duplicate" => Self::Duplicate,
            "near_duplicate" => Self::NearDuplicate,
            "transfer_match" => Self::TransferMatch,
            "needs_account" => Self::NeedsAccount,
            _ => return Err(Problem::internal()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    Ofx,
    Csv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatementKind {
    Bank,
    CreditCard,
}

/// The balance the bank reported at the end of the statement.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LedgerBalance {
    #[schema(value_type = String, example = "1234.56")]
    amount: Amount,
    date: Date,
}

/// A line of the file that wasn't read.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SkippedLine {
    /// 1-based line (CSV) or transaction (OFX) number.
    line: u64,
    reason: String,
}

/// How many rows have each status.
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct RowCounts {
    new: i64,
    duplicate: i64,
    near_duplicate: i64,
    transfer_match: i64,
    needs_account: i64,
}

impl RowCounts {
    fn add(&mut self, status: RowStatus, n: i64) {
        let slot = match status {
            RowStatus::New => &mut self.new,
            RowStatus::Duplicate => &mut self.duplicate,
            RowStatus::NearDuplicate => &mut self.near_duplicate,
            RowStatus::TransferMatch => &mut self.transfer_match,
            RowStatus::NeedsAccount => &mut self.needs_account,
        };
        *slot += n;
    }
}

/// One account's part of the file.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StagedStatement {
    position: i16,
    /// The last four of the account number in the file (CSVs have none).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "6789")]
    number_last4: Option<String>,
    /// `null` until the account is known: from the number in the file, or
    /// set by the client.
    account_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<StatementKind>,
    commodity_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    ledger: Option<LedgerBalance>,
    skipped: Vec<SkippedLine>,
    counts: RowCounts,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Import {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    format: FileFormat,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    statements: Vec<StagedStatement>,
    #[serde(skip)]
    version: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StagedRow {
    statement: i16,
    position: i32,
    date: Date,
    /// Signed from the account's side: negative is money leaving it.
    #[schema(value_type = String, example = "-80.00")]
    amount: Amount,
    description: String,
    /// The bank's own ID for the row, if it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    native_id: Option<String>,
    status: RowStatus,
    /// The existing transaction a duplicate or match is of.
    #[serde(skip_serializing_if = "Option::is_none")]
    match_transaction_id: Option<Uuid>,
    /// The row's fields as the file had them.
    #[schema(value_type = Object)]
    raw: serde_json::Value,
}

/// A bank file's bytes, as is.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct BankFile(#[expect(dead_code, reason = "only describes the body")] Vec<u8>);

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UploadParams {
    /// The account the file is for, if the client knows: used when the file
    /// names none (a CSV) or a number no account has. Without it, such a
    /// statement waits for its account in review. Refused for a file with
    /// several statements.
    account_id: Option<Uuid>,
    /// The file's name, to recognize the import by.
    file_name: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RowParams {
    /// Only this statement's rows.
    statement: Option<i16>,
    /// Only rows with this status.
    status: Option<RowStatus>,
    /// Items per page (default 100, max 500).
    limit: Option<u32>,
    /// `next_cursor` from the previous page. Opaque.
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StatementUpdate {
    /// The account the statement is for; `null` to unset it.
    account_id: Option<Uuid>,
    /// Also add the file's account number to the account, so its next
    /// files resolve by themselves.
    #[serde(default)]
    remember_number: bool,
}

// ------------------------------------------------------------- Helpers ----

fn format_of(s: &str) -> Result<FileFormat, Problem> {
    match s {
        "ofx" => Ok(FileFormat::Ofx),
        "csv" => Ok(FileFormat::Csv),
        _ => Err(Problem::internal()),
    }
}

fn kind_of(s: &str) -> Result<StatementKind, Problem> {
    match s {
        "bank" => Ok(StatementKind::Bank),
        "credit_card" => Ok(StatementKind::CreditCard),
        _ => Err(Problem::internal()),
    }
}

/// The imports as the API shows them, with their statements and counts.
async fn output(db: &mut PgConnection, imports: Vec<ImportRow>) -> Result<Vec<Import>, Problem> {
    let ids: Vec<Uuid> = imports.iter().map(|i| i.id).collect();
    let statements = store::imports::statements(&mut *db, &ids).await?;
    let counts = store::imports::counts(&mut *db, &ids).await?;
    imports
        .into_iter()
        .map(|i| {
            let statements = statements
                .iter()
                .filter(|(id, _)| *id == i.id)
                .map(|(_, s)| {
                    let mut c = RowCounts::default();
                    for (_, n) in counts
                        .iter()
                        .filter(|(id, n)| *id == i.id && n.statement == s.position)
                    {
                        c.add(RowStatus::parse(&n.status)?, n.count);
                    }
                    Ok(StagedStatement {
                        position: s.position,
                        number_last4: s.account_number.as_ref().map(|n| n.last4().to_owned()),
                        account_id: s.account_id,
                        kind: s.kind.as_deref().map(kind_of).transpose()?,
                        commodity_id: s.commodity_id,
                        ledger: s
                            .ledger_amount
                            .zip(s.ledger_date)
                            .map(|(a, d)| LedgerBalance {
                                amount: Amount::from_decimal(a),
                                date: d,
                            }),
                        skipped: serde_json::from_value(s.skipped.clone())
                            .map_err(|_| Problem::internal())?,
                        counts: c,
                    })
                })
                .collect::<Result<_, Problem>>()?;
            Ok(Import {
                id: i.id,
                file_name: i.file_name,
                format: format_of(&i.format)?,
                created_at: i.created_at,
                statements,
                version: i.version,
            })
        })
        .collect()
}

async fn one(db: &mut PgConnection, id: Uuid) -> Result<Import, Problem> {
    let row = store::imports::get(&mut *db, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    output(db, vec![row])
        .await?
        .pop()
        .ok_or_else(Problem::internal)
}

fn respond(status: StatusCode, i: &Import) -> Response {
    (status, etag(i.version), axum::Json(i)).into_response()
}

/// How far apart, in days, a near-duplicate's date may be.
const NEAR_DAYS: i32 = 3;
/// How far apart the two sides of a transfer may be dated (money can take
/// a few business days to arrive).
const TRANSFER_DAYS: i32 = 5;

/// One-to-one near-duplicate matching: each row and each transaction is used
/// at most once, the most similar descriptions first, then the closest
/// dates. Rows in `done` and transactions in `taken` are already matched.
fn near_matches(
    mut candidates: Vec<NearCandidate>,
    done: &HashSet<i32>,
    taken: &HashSet<Uuid>,
) -> HashMap<i32, Uuid> {
    candidates.sort_by(|a, b| {
        b.similarity
            .cmp(&a.similarity)
            .then(a.days_apart.cmp(&b.days_apart))
            .then(a.position.cmp(&b.position))
            .then(a.transaction_id.cmp(&b.transaction_id))
    });
    let mut taken = taken.clone();
    let mut out = HashMap::new();
    for c in candidates {
        if done.contains(&c.position) || out.contains_key(&c.position) {
            continue;
        }
        if taken.insert(c.transaction_id) {
            out.insert(c.position, c.transaction_id);
        }
    }
    out
}

/// Works out each row's status in one statement, from what the ledger holds
/// now (design §6.3-§6.4): `needs_account` without an account; `duplicate`
/// when its fingerprint was imported into the account before;
/// `near_duplicate` when a transaction in the account has the same commodity
/// and signed amount within [`NEAR_DAYS`] days; `transfer_match` when a
/// transaction moved the same amount the other way between another of your
/// accounts and the outside within [`TRANSFER_DAYS`] days, so the two are one
/// transfer; else `new`. Each match is one-to-one.
async fn restatus(db: &mut PgConnection, import_id: Uuid, statement: i16) -> Result<(), Problem> {
    let (_, s) = store::imports::statements(&mut *db, &[import_id])
        .await?
        .into_iter()
        .find(|(_, s)| s.position == statement)
        .ok_or_else(Problem::internal)?;
    let rows =
        store::imports::rows(&mut *db, import_id, Some(statement), None, None, i64::MAX).await?;
    let mut matched: HashMap<i32, (RowStatus, Uuid)> = HashMap::new();
    if let Some(account) = s.account_id {
        let fps: Vec<String> = rows.iter().map(|r| r.fingerprint.clone()).collect();
        let known: HashMap<String, Uuid> =
            store::imports::known_fingerprints(&mut *db, account, &fps)
                .await?
                .into_iter()
                .collect();
        for r in &rows {
            if let Some(t) = known.get(&r.fingerprint) {
                matched.insert(r.position, (RowStatus::Duplicate, *t));
            }
        }
        let candidates = store::imports::near_candidates(
            &mut *db,
            import_id,
            statement,
            account,
            s.commodity_id,
            NEAR_DAYS,
        )
        .await?;
        let done: HashSet<i32> = matched.keys().copied().collect();
        let taken: HashSet<Uuid> = matched.values().map(|m| m.1).collect();
        for (position, t) in near_matches(candidates, &done, &taken) {
            matched.insert(position, (RowStatus::NearDuplicate, t));
        }
        let candidates = store::imports::transfer_candidates(
            &mut *db,
            import_id,
            statement,
            account,
            s.commodity_id,
            TRANSFER_DAYS,
        )
        .await?;
        let done: HashSet<i32> = matched.keys().copied().collect();
        let taken: HashSet<Uuid> = matched.values().map(|m| m.1).collect();
        for (position, t) in near_matches(candidates, &done, &taken) {
            matched.insert(position, (RowStatus::TransferMatch, t));
        }
    }
    let statuses: Vec<(i32, String, Option<Uuid>)> = rows
        .iter()
        .map(|r| {
            let (status, t) = match (s.account_id, matched.get(&r.position)) {
                (None, _) => (RowStatus::NeedsAccount, None),
                (Some(_), Some((status, t))) => (*status, Some(*t)),
                (Some(_), None) => (RowStatus::New, None),
            };
            (r.position, status.as_str().to_owned(), t)
        })
        .collect();
    store::imports::set_statuses(&mut *db, import_id, statement, &statuses).await?;
    Ok(())
}

/// The account a statement is for: the one its number belongs to, else the
/// one the client gave.
async fn resolve(
    db: &mut PgConnection,
    number: Option<&AccountNumber>,
    given: Option<Uuid>,
) -> Result<Option<Uuid>, Problem> {
    let found = match number {
        Some(n) => store::account_numbers::find(&mut *db, n)
            .await?
            .map(|r| r.account_id),
        None => None,
    };
    match (found, given) {
        (Some(f), Some(g)) if f != g => Err(Problem::invalid(format!(
            "the file's account number {} belongs to another account",
            number.map(ToString::to_string).unwrap_or_default()
        ))),
        (Some(f), _) => Ok(Some(f)),
        (None, Some(g)) => {
            store::accounts::get(&mut *db, g, false)
                .await?
                .ok_or_else(|| Problem::invalid("account_id: no such account"))?;
            Ok(Some(g))
        }
        (None, None) => Ok(None),
    }
}

// ------------------------------------------------------------ Handlers ----

/// List staged imports, newest first.
#[utoipa::path(get, path = "/imports", tag = "imports", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<Import>)))]
async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<Import>>, Problem> {
    let after = crate::common::id_cursor(page.cursor.as_deref())?;
    let mut db = state.pool.acquire().await?;
    let rows = store::imports::list(&mut *db, after, page.limit() + 1).await?;
    let items = output(&mut db, rows).await?;
    Ok(axum::Json(Page::from_rows(items, page.limit(), |i| {
        i.id.to_string()
    })))
}

/// Get one staged import, with its statements and how many rows have each
/// status.
#[utoipa::path(get, path = "/imports/{id}", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Import, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let mut db = state.pool.acquire().await?;
    Ok(respond(StatusCode::OK, &one(&mut db, id).await?))
}

/// Upload a bank file (OFX, QFX, QBO or CSV; the format is read from the
/// content). Its rows are staged, each with what it would become; nothing
/// enters the ledger until the import is committed.
#[utoipa::path(post, path = "/imports", tag = "imports", security(("bearer" = [])),
    params(UploadParams),
    request_body(content = BankFile, content_type = "application/octet-stream",
                 description = "The file, as is (at most 10 MiB)"),
    responses((status = CREATED, body = Import, headers(("ETag" = String), ("Location" = String))),
              (status = PAYLOAD_TOO_LARGE, description = "Bigger than 10 MiB"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn upload(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Query(p): Query<UploadParams>,
    body: Bytes,
) -> Result<Response, Problem> {
    let file_name = p.file_name.as_deref().map(str::trim);
    if file_name.is_some_and(|n| n.is_empty() || n.chars().count() > 200) {
        return Err(Problem::invalid("file_name must be 1-200 characters"));
    }
    let zone = state.zone.clone();
    let bytes = body.to_vec();
    let statements = tokio::task::spawn_blocking(move || import::parse(bytes, &zone))
        .await
        .map_err(|_| Problem::internal())?
        .map_err(|e| Problem::invalid(e.to_string()))?;
    let format = match statements[0].format {
        import::Format::Ofx => "ofx",
        import::Format::Csv => "csv",
    };
    if statements.len() > 1 && p.account_id.is_some() {
        return Err(Problem::invalid(
            "the file has several statements: set each one's account after uploading",
        ));
    }

    let id = Uuid::now_v7();
    let mut tx = state.pool.begin().await?;
    let token = principal.actor().map(|t| *t.as_uuid());
    store::imports::insert(&mut *tx, id, file_name, format, token).await?;
    for (i, s) in statements.into_iter().enumerate() {
        let position = i16::try_from(i).map_err(|_| Problem::invalid("too many statements"))?;
        let code = s.currency.as_deref().unwrap_or("USD");
        let commodity = store::commodities::by_code(&mut *tx, code)
            .await?
            .ok_or_else(|| {
                Problem::invalid(format!("the file's currency {code} isn't a commodity"))
            })?;
        let mut skipped: Vec<SkippedLine> = s
            .skipped
            .iter()
            .map(|k| SkippedLine {
                line: k.line,
                reason: k.reason.clone(),
            })
            .collect();
        let fps = fingerprints(&s.rows);
        let mut rows = Vec::with_capacity(s.rows.len());
        for (n, (r, fp)) in s.rows.into_iter().zip(fps).enumerate() {
            let line = u64::try_from(n + 1).unwrap_or(u64::MAX);
            if fp.len() > MAX_ID {
                skipped.push(SkippedLine {
                    line,
                    reason: format!("the bank's ID is longer than {MAX_ID} characters"),
                });
                continue;
            }
            if r.amount.as_decimal().is_zero() {
                skipped.push(SkippedLine {
                    line,
                    reason: "the amount is zero".to_owned(),
                });
                continue;
            }
            rows.push(RowRow {
                statement: position,
                position: i32::try_from(n).map_err(|_| Problem::invalid("too many rows"))?,
                date: r.date,
                amount: r.amount.as_decimal(),
                description: r.description,
                native_id: r.native_id,
                fingerprint: fp,
                raw: serde_json::Value::Object(r.raw),
                status: RowStatus::New.as_str().to_owned(),
                match_transaction_id: None,
            });
        }
        let account_id = resolve(&mut tx, s.account.as_ref(), p.account_id).await?;
        let statement = StatementRow {
            position,
            account_number: s.account,
            account_id,
            kind: s.kind.map(|k| match k {
                import::StatementKind::Bank => "bank".to_owned(),
                import::StatementKind::CreditCard => "credit_card".to_owned(),
            }),
            commodity_id: commodity.id,
            ledger_amount: s.ledger.as_ref().map(|l| l.amount.as_decimal()),
            ledger_date: s.ledger.as_ref().map(|l| l.date),
            skipped: serde_json::to_value(&skipped).map_err(|_| Problem::internal())?,
        };
        store::imports::insert_statement(&mut *tx, id, &statement).await?;
        if !rows.is_empty() {
            store::imports::insert_rows(&mut *tx, id, &rows).await?;
        }
        restatus(&mut tx, id, position).await?;
    }
    let created = one(&mut tx, id).await?;
    audit(
        &mut tx,
        principal,
        Action::Create,
        "staged_import",
        id,
        None,
        Some(&json!({ "file_name": created.file_name, "format": format })),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &created);
    if let Ok(loc) = format!("/api/v1/imports/{id}").parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// The import's rows, in file order.
#[utoipa::path(get, path = "/imports/{id}/rows", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path), RowParams),
    responses((status = OK, body = Page<StagedRow>),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn rows(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(p): Query<RowParams>,
) -> Result<axum::Json<Page<StagedRow>>, Problem> {
    let after = p
        .cursor
        .as_deref()
        .map(|c| {
            c.split_once(':')
                .and_then(|(s, r)| Some((s.parse().ok()?, r.parse().ok()?)))
                .ok_or_else(|| Problem::invalid("invalid cursor"))
        })
        .transpose()?;
    let limit = i64::from(p.limit.unwrap_or(100).clamp(1, 500));
    let mut db = state.pool.acquire().await?;
    store::imports::get(&mut *db, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    let rows = store::imports::rows(
        &mut *db,
        id,
        p.statement,
        p.status.map(RowStatus::as_str),
        after,
        limit + 1,
    )
    .await?;
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(StagedRow {
                statement: r.statement,
                position: r.position,
                date: r.date,
                amount: Amount::from_decimal(r.amount),
                description: r.description,
                native_id: r.native_id,
                status: RowStatus::parse(&r.status)?,
                match_transaction_id: r.match_transaction_id,
                raw: r.raw,
            })
        })
        .collect::<Result<Vec<_>, Problem>>()?;
    Ok(axum::Json(Page::from_rows(items, limit, |r| {
        format!("{}:{}", r.statement, r.position)
    })))
}

/// Set the account a statement is for; its rows' statuses are worked out
/// again.
#[utoipa::path(patch, path = "/imports/{id}/statements/{position}", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("position" = i16, Path),
           ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = StatementUpdate,
    responses((status = OK, body = Import, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "The number belongs to another account", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn set_statement(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((id, position)): Path<(Uuid, i16)>,
    IfMatch(version): IfMatch,
    Json(req): Json<StatementUpdate>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let current = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current != version {
        return Err(Problem::stale());
    }
    let (_, statement) = store::imports::statements(&mut *tx, &[id])
        .await?
        .into_iter()
        .find(|(_, s)| s.position == position)
        .ok_or_else(Problem::not_found)?;
    if let Some(a) = req.account_id {
        store::accounts::get(&mut *tx, a, true)
            .await?
            .ok_or_else(|| Problem::invalid("account_id: no such account"))?;
    }
    if req.remember_number {
        let (Some(number), Some(account)) = (&statement.account_number, req.account_id) else {
            return Err(Problem::invalid(
                "remember_number needs an account_id and a statement with an account number",
            ));
        };
        match store::account_numbers::find(&mut *tx, number).await? {
            Some(n) if n.account_id == account => {}
            Some(_) => {
                return Err(Problem::conflict(format!(
                    "{number} already belongs to another account"
                )));
            }
            None => {
                store::account_numbers::insert(&mut *tx, Uuid::now_v7(), account, number).await?;
                store::accounts::touch(&mut *tx, account).await?;
            }
        }
    }
    store::imports::set_account(&mut *tx, id, position, req.account_id).await?;
    restatus(&mut tx, id, position).await?;
    store::imports::touch(&mut *tx, id).await?;
    let after = one(&mut tx, id).await?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "staged_import",
        id,
        None,
        Some(
            &json!({ "statement": position, "account_id": req.account_id,
                      "remember_number": req.remember_number }),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Discard a staged import: nothing of it enters the ledger.
#[utoipa::path(delete, path = "/imports/{id}", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = NO_CONTENT),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json")))]
async fn discard(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
) -> Result<StatusCode, Problem> {
    let mut tx = state.pool.begin().await?;
    let current = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current != version {
        return Err(Problem::stale());
    }
    store::imports::delete(&mut *tx, id).await?;
    audit::<serde_json::Value>(
        &mut tx,
        principal,
        Action::Delete,
        "staged_import",
        id,
        None,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(position: i32, t: u128, similarity: i32, days_apart: i32) -> NearCandidate {
        NearCandidate {
            position,
            transaction_id: Uuid::from_u128(t),
            similarity,
            days_apart,
        }
    }

    #[test]
    fn near_matches_are_one_to_one_best_first() {
        // Two identical coffees and one recorded coffee: one match. The
        // better description wins a transaction; closer dates break ties.
        let got = near_matches(
            vec![
                c(0, 1, 200, 0),
                c(1, 1, 900, 2),
                c(2, 2, 500, 3),
                c(2, 3, 500, 1),
                c(3, 4, 1000, 0),
            ],
            &HashSet::from([3]),
            &HashSet::from([Uuid::from_u128(2)]),
        );
        assert_eq!(
            got,
            HashMap::from([(1, Uuid::from_u128(1)), (2, Uuid::from_u128(3))])
        );
    }
}
