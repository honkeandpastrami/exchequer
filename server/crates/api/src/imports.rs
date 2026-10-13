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
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use store::audit::Action;
use store::imports::{
    CONFIDENT, Fingerprint, ImportRow, ImportedRow, NearCandidate, Outcome, RowRow, StatementRow,
    SuggesterRow, SuggestionRow,
};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::{audit, nullable, write_error};
use crate::conventions::{IfMatch, Page, PageParams, etag};
use crate::problem::{Json, Path, Problem, Query};
use crate::transactions::{
    Leg, NewTransaction, Relabel, create_in, into_transfer, replace_with_row, replaceable,
};

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
        .routes(routes!(set_row))
        .routes(routes!(commit))
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
    /// The account the rows are worked out against: the one the file's
    /// number belongs to, or the client's choice, else a confident suggestion
    /// (`account_suggested`). `null` until there is one.
    account_id: Option<Uuid>,
    /// Whether `account_id` is the suggestion's, not yet the statement's own.
    account_suggested: bool,
    /// A suggested account, confident or not; it stays after you pick one,
    /// to record how good it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_account: Option<AccountSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<StatementKind>,
    commodity_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    ledger: Option<LedgerBalance>,
    skipped: Vec<SkippedLine>,
    counts: RowCounts,
}

/// A label a row's new transaction gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Label {
    Counterparty,
    Category,
    Purpose,
}

impl Label {
    const ALL: [Self; 3] = [Self::Counterparty, Self::Category, Self::Purpose];

    const fn as_str(self) -> &'static str {
        match self {
            Self::Counterparty => "counterparty",
            Self::Category => "category",
            Self::Purpose => "purpose",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        Self::ALL
            .into_iter()
            .find(|l| l.as_str() == s)
            .ok_or_else(Problem::internal)
    }
}

/// Which level of the suggestion stack made a suggestion (docs/ai.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuggesterKind {
    Rule,
    Local,
    Remote,
}

/// The exact system that made a suggestion.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Suggester {
    id: Uuid,
    kind: SuggesterKind,
    /// E.g. the rules, a local method, or a remote deployment and model.
    name: String,
    version: String,
    /// What else identifies it, e.g. the data a local model learned from.
    #[schema(value_type = Object)]
    details: serde_json::Value,
}

impl TryFrom<SuggesterRow> for Suggester {
    type Error = Problem;

    fn try_from(r: SuggesterRow) -> Result<Self, Problem> {
        Ok(Self {
            id: r.id,
            kind: match r.kind.as_str() {
                "rule" => SuggesterKind::Rule,
                "local" => SuggesterKind::Local,
                "remote" => SuggesterKind::Remote,
                _ => return Err(Problem::internal()),
            },
            name: r.name,
            version: r.version,
            details: r.details,
        })
    }
}

/// A suggested value for one of a row's labels.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LabelSuggestion {
    label: Label,
    /// The suggested counterparty, category or party; `null` suggests none.
    value: Option<Uuid>,
    /// Per mille: 900 is 0.9.
    confidence: i16,
    /// Whether it's confident enough (at least 900) to be committed unless
    /// you choose otherwise.
    confident: bool,
    reason: String,
    suggester: Suggester,
}

/// A statement's suggested account.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AccountSuggestion {
    account_id: Uuid,
    /// Per mille: 900 is 0.9.
    confidence: i16,
    confident: bool,
    reason: String,
    suggester: Suggester,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Import {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    /// The file's identity: its bytes' SHA-256 (hex), whatever it's called.
    file_sha256: String,
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
    /// What committing does with the row: the client's choice, else the
    /// default for its status. Absent while the statement has no account.
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<Decision>,
    /// Whether `decision` is the client's choice rather than the default.
    decided: bool,
    /// The new transaction's description, if not the bank's.
    #[serde(skip_serializing_if = "Option::is_none")]
    set_description: Option<String>,
    /// The labels below are what a new transaction gets: your choice, else a
    /// confident suggestion. For a new transaction: who's on the outside
    /// side (unknown if absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    counterparty_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category_id: Option<Uuid>,
    /// Who the money was for; absent, the account's owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    purpose_id: Option<Uuid>,
    /// The labels you set (a `null` one is an explicit none).
    chosen: Vec<Label>,
    /// At most one per label, confident or not.
    suggestions: Vec<LabelSuggestion>,
    /// For a row that will be a new transaction: the labels that are
    /// neither yours nor confidently suggested. Commit refuses while any row
    /// has some.
    missing: Vec<Label>,
    /// The row's fields as the file had them.
    #[schema(value_type = Object)]
    raw: serde_json::Value,
}

/// What committing does with a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// The row is its match: record that it was imported (a duplicate or
    /// near-duplicate), or make the matched transaction a transfer to this
    /// account (a transfer match). The default for those statuses.
    Match,
    /// A new transaction. The default for `new` rows; not for duplicates.
    New,
    /// Leave the row out.
    Skip,
    /// For a duplicate or near-duplicate: the matched transaction takes the
    /// row's date, amount and description, and the labels set here (yours,
    /// or confident suggestions); its other labels, notes, tags and batch
    /// stay. It must be one split between this account and the outside.
    Replace,
}

impl Decision {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::New => "new",
            Self::Skip => "skip",
            Self::Replace => "replace",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        match s {
            "match" => Ok(Self::Match),
            "new" => Ok(Self::New),
            "skip" => Ok(Self::Skip),
            "replace" => Ok(Self::Replace),
            _ => Err(Problem::internal()),
        }
    }

    const fn default_for(status: RowStatus) -> Option<Self> {
        match status {
            RowStatus::New => Some(Self::New),
            RowStatus::Duplicate | RowStatus::NearDuplicate | RowStatus::TransferMatch => {
                Some(Self::Match)
            }
            RowStatus::NeedsAccount => None,
        }
    }

    /// Whether the decision makes sense for a row with this status.
    const fn fits(self, status: RowStatus) -> bool {
        match self {
            Self::Match => matches!(
                status,
                RowStatus::Duplicate | RowStatus::NearDuplicate | RowStatus::TransferMatch
            ),
            Self::New => !matches!(status, RowStatus::Duplicate),
            Self::Skip => true,
            Self::Replace => matches!(status, RowStatus::Duplicate | RowStatus::NearDuplicate),
        }
    }
}

/// Fields to change; omitted ones stay as they are, `null` clears one.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RowUpdate {
    /// `null`: back to the default for the row's status.
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Decision>)]
    decision: Option<Option<Decision>>,
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<String>)]
    set_description: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Uuid>)]
    counterparty_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Uuid>)]
    category_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Uuid>)]
    purpose_id: Option<Option<Uuid>>,
    /// Labels to stop choosing, so their suggestions apply again. Setting a
    /// label (to a value or `null`) makes it your choice.
    #[serde(default)]
    revert: Vec<Label>,
}

/// What a commit did.
#[derive(Debug, Serialize, ToSchema)]
pub struct Committed {
    /// The batch the new transactions are in; absent if there were none.
    #[serde(skip_serializing_if = "Option::is_none")]
    import_batch_id: Option<Uuid>,
    /// New transactions.
    created: u64,
    /// Existing transactions made transfers.
    transfers: u64,
    /// Rows recorded as already in the ledger (duplicates, near-duplicates).
    matched: u64,
    /// Existing transactions replaced by their rows.
    replaced: u64,
    skipped: u64,
    /// One per statement with a ledger balance: the bank's balance against
    /// the ledger's.
    balance_checks: Vec<crate::balance_checks::BalanceCheck>,
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

/// The suggesters with these IDs, as the API shows them.
async fn suggesters(
    db: &mut PgConnection,
    ids: impl IntoIterator<Item = Uuid>,
) -> Result<HashMap<Uuid, Suggester>, Problem> {
    let mut ids: Vec<Uuid> = ids.into_iter().collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    store::imports::suggesters(&mut *db, &ids)
        .await?
        .into_iter()
        .map(|r| Ok((r.id, Suggester::try_from(r)?)))
        .collect()
}

fn known(suggesters: &HashMap<Uuid, Suggester>, id: Uuid) -> Result<Suggester, Problem> {
    suggesters.get(&id).cloned().ok_or_else(Problem::internal)
}

/// The imports as the API shows them, with their statements and counts.
async fn output(db: &mut PgConnection, imports: Vec<ImportRow>) -> Result<Vec<Import>, Problem> {
    let ids: Vec<Uuid> = imports.iter().map(|i| i.id).collect();
    let statements = store::imports::statements(&mut *db, &ids).await?;
    let counts = store::imports::counts(&mut *db, &ids).await?;
    let by = suggesters(
        db,
        statements
            .iter()
            .filter_map(|(_, s)| s.suggestion.as_ref().map(|a| a.suggester_id)),
    )
    .await?;
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
                        account_id: s.effective_account(),
                        account_suggested: s.account_id.is_none()
                            && s.effective_account().is_some(),
                        suggested_account: s
                            .suggestion
                            .as_ref()
                            .map(|a| {
                                Ok::<_, Problem>(AccountSuggestion {
                                    account_id: a.account_id,
                                    confidence: a.confidence,
                                    confident: a.confidence >= CONFIDENT,
                                    reason: a.reason.clone(),
                                    suggester: known(&by, a.suggester_id)?,
                                })
                            })
                            .transpose()?,
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
                file_sha256: i.file_sha256,
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
    let account = s.effective_account();
    if let Some(account) = account {
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
            let (status, t) = match (account, matched.get(&r.position)) {
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

/// What a new transaction would get for each label: your choice, else a
/// confident suggestion; a label with neither is missing.
#[derive(Debug, PartialEq, Eq)]
struct Labels {
    counterparty: Option<Uuid>,
    category: Option<Uuid>,
    purpose: Option<Uuid>,
    missing: Vec<Label>,
}

/// `suggestions` may hold other rows' too.
fn labels(r: &RowRow, suggestions: &[SuggestionRow]) -> Labels {
    let mut out = Labels {
        counterparty: r.counterparty_id,
        category: r.category_id,
        purpose: r.purpose_id,
        missing: Vec::new(),
    };
    for label in Label::ALL {
        if r.chosen.iter().any(|c| c == label.as_str()) {
            continue;
        }
        let slot = match label {
            Label::Counterparty => &mut out.counterparty,
            Label::Category => &mut out.category,
            Label::Purpose => &mut out.purpose,
        };
        match suggestions.iter().find(|s| {
            (s.statement, s.position) == (r.statement, r.position)
                && s.label == label.as_str()
                && s.confidence >= CONFIDENT
        }) {
            Some(s) => *slot = s.value,
            None => {
                *slot = None;
                out.missing.push(label);
            }
        }
    }
    out
}

/// The decision committing carries out for a row; `None` while its
/// statement has no account.
fn decision_of(r: &RowRow) -> Result<Option<Decision>, Problem> {
    let status = RowStatus::parse(&r.status)?;
    if status == RowStatus::NeedsAccount {
        return Ok(None);
    }
    Ok(match r.decision.as_deref() {
        Some(d) => Some(Decision::parse(d)?),
        None => Decision::default_for(status),
    })
}

/// Rows as the API shows them, with their suggestions.
async fn shown(
    db: &mut PgConnection,
    import_id: Uuid,
    rows: Vec<RowRow>,
) -> Result<Vec<StagedRow>, Problem> {
    let keys: Vec<(i16, i32)> = rows.iter().map(|r| (r.statement, r.position)).collect();
    let all = store::imports::suggestions(&mut *db, import_id, &keys).await?;
    let by = suggesters(db, all.iter().map(|s| s.suggester_id)).await?;
    rows.into_iter()
        .map(|r| {
            let mine: Vec<SuggestionRow> = all
                .iter()
                .filter(|s| (s.statement, s.position) == (r.statement, r.position))
                .cloned()
                .collect();
            staged_row(r, &mine, &by)
        })
        .collect()
}

fn staged_row(
    r: RowRow,
    suggestions: &[SuggestionRow],
    by: &HashMap<Uuid, Suggester>,
) -> Result<StagedRow, Problem> {
    let status = RowStatus::parse(&r.status)?;
    let chosen = r.decision.as_deref().map(Decision::parse).transpose()?;
    let decision = decision_of(&r)?;
    let l = labels(&r, suggestions);
    let suggestions = suggestions
        .iter()
        .map(|s| {
            Ok(LabelSuggestion {
                label: Label::parse(&s.label)?,
                value: s.value,
                confidence: s.confidence,
                confident: s.confidence >= CONFIDENT,
                reason: s.reason.clone(),
                suggester: known(by, s.suggester_id)?,
            })
        })
        .collect::<Result<_, Problem>>()?;
    Ok(StagedRow {
        statement: r.statement,
        position: r.position,
        date: r.date,
        amount: Amount::from_decimal(r.amount),
        description: r.description,
        native_id: r.native_id,
        status,
        match_transaction_id: r.match_transaction_id,
        decision,
        decided: chosen.is_some(),
        set_description: r.set_description,
        counterparty_id: l.counterparty,
        category_id: l.category,
        purpose_id: l.purpose,
        chosen: Label::ALL
            .into_iter()
            .filter(|l| r.chosen.iter().any(|c| c == l.as_str()))
            .collect(),
        suggestions,
        missing: if decision == Some(Decision::New) {
            l.missing
        } else {
            Vec::new()
        },
        raw: r.raw,
    })
}

/// Puts which row an error is about in front of its detail.
fn at_row(r: &RowRow) -> impl Fn(Problem) -> Problem + '_ {
    move |mut p| {
        p.detail = Some(format!(
            "statement {} row {}: {}",
            r.statement,
            r.position,
            p.detail.as_deref().unwrap_or(&p.title)
        ));
        p
    }
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
    let sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
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
    store::imports::insert(&mut *tx, id, file_name, &sha256, format, token).await?;
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
                decision: None,
                set_description: None,
                counterparty_id: None,
                category_id: None,
                purpose_id: None,
                chosen: Vec::new(),
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
            suggestion: None,
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
    let items = shown(&mut db, id, rows).await?;
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

/// Decide what committing does with a row, and label the transaction it
/// will become.
#[utoipa::path(patch, path = "/imports/{id}/rows/{statement}/{position}", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("statement" = i16, Path), ("position" = i32, Path),
           ("If-Match" = String, Header, description = "The import's ETag from your last read")),
    request_body = RowUpdate,
    responses((status = OK, body = StagedRow, headers(("ETag" = String, description = "The import's"))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn set_row(
    State(state): State<AppState>,
    Path((id, statement, position)): Path<(Uuid, i16, i32)>,
    IfMatch(version): IfMatch,
    Json(req): Json<RowUpdate>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let current = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current != version {
        return Err(Problem::stale());
    }
    let after = position.checked_sub(1).ok_or_else(Problem::not_found)?;
    let mut row = store::imports::rows(
        &mut *tx,
        id,
        Some(statement),
        None,
        Some((statement, after)),
        1,
    )
    .await?
    .into_iter()
    .find(|r| r.position == position)
    .ok_or_else(Problem::not_found)?;
    if let Some(d) = req.decision {
        let status = RowStatus::parse(&row.status)?;
        if let Some(d) = d
            && status != RowStatus::NeedsAccount
            && !d.fits(status)
        {
            return Err(Problem::invalid(format!(
                "a {} row can't be {}",
                status.as_str(),
                d.as_str()
            )));
        }
        if d == Some(Decision::Replace) {
            let (_, s) = store::imports::statements(&mut *tx, &[id])
                .await?
                .into_iter()
                .find(|(_, s)| s.position == statement)
                .ok_or_else(Problem::internal)?;
            let (Some(t), Some(account)) = (row.match_transaction_id, s.effective_account()) else {
                return Err(Problem::internal());
            };
            replaceable(&mut tx, t, account, s.commodity_id)
                .await
                .map_err(|p| Problem::invalid(p.detail.unwrap_or(p.title)))?;
        }
        row.decision = d.map(|d| d.as_str().to_owned());
    }
    if let Some(d) = req.set_description {
        let d = d.map(|d| d.trim().to_owned());
        if d.as_ref()
            .is_some_and(|d| d.is_empty() || d.chars().count() > 500)
        {
            return Err(Problem::invalid("set_description must be 1-500 characters"));
        }
        row.set_description = d;
    }
    for (label, value, slot) in [
        (
            Label::Counterparty,
            req.counterparty_id,
            &mut row.counterparty_id,
        ),
        (Label::Category, req.category_id, &mut row.category_id),
        (Label::Purpose, req.purpose_id, &mut row.purpose_id),
    ] {
        let name = label.as_str().to_owned();
        if req.revert.contains(&label) {
            if value.is_some() {
                return Err(Problem::invalid(format!(
                    "{name}: set and reverted at once"
                )));
            }
            *slot = None;
            row.chosen.retain(|c| *c != name);
        } else if let Some(v) = value {
            *slot = v;
            if !row.chosen.contains(&name) {
                row.chosen.push(name);
            }
        }
    }
    store::imports::set_decision(&mut *tx, id, &row)
        .await
        .map_err(|e| write_error(e, String::new))?;
    store::imports::touch(&mut *tx, id).await?;
    let version = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::internal)?;
    let row = shown(&mut tx, id, vec![row])
        .await?
        .pop()
        .ok_or_else(Problem::internal)?;
    tx.commit().await?;
    Ok((StatusCode::OK, etag(version), axum::Json(row)).into_response())
}

/// Commit the import: all or nothing. Every row's status is worked out
/// again first; if any changed (the ledger moved on since), nothing is
/// committed and the import comes back with a 409 to review again. Then each
/// row's decision is carried out: new transactions go into one `file`
/// import batch; transfer matches turn the matched transaction into a
/// transfer; every row that's now in the ledger is recorded as imported, so
/// the same row is a duplicate next time, and what this file says about it is
/// kept with its transaction (once per file, whatever its name). Each
/// statement's ledger balance becomes a balance check. The staged import is
/// then gone.
#[utoipa::path(post, path = "/imports/{id}/commit", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = OK, body = Committed),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "A statement has no account, a row's status changed, or a month is closed", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, description = "A row breaks a ledger rule", body = Problem, content_type = "application/problem+json")))]
async fn commit(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
) -> Result<axum::Json<Committed>, Problem> {
    let mut tx = state.pool.begin().await?;
    let current = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current != version {
        return Err(Problem::stale());
    }
    let import = store::imports::get(&mut *tx, id)
        .await?
        .ok_or_else(Problem::internal)?;
    let statements: Vec<StatementRow> = store::imports::statements(&mut *tx, &[id])
        .await?
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    if let Some(s) = statements.iter().find(|s| s.effective_account().is_none()) {
        return Err(Problem::conflict(format!(
            "statement {} has no account yet",
            s.position
        )));
    }

    // The ledger may have moved on since the rows were checked.
    let seen = store::imports::rows(&mut *tx, id, None, None, None, i64::MAX).await?;
    for s in &statements {
        restatus(&mut tx, id, s.position).await?;
    }
    let rows = store::imports::rows(&mut *tx, id, None, None, None, i64::MAX).await?;
    let changed = rows.iter().zip(&seen).any(|(now, before)| {
        now.status != before.status || now.match_transaction_id != before.match_transaction_id
    });
    if changed {
        store::imports::touch(&mut *tx, id).await?;
        tx.commit().await?;
        return Err(Problem::conflict(
            "the ledger changed since the rows were checked: review them again",
        ));
    }

    // Every new transaction needs each label from you or a confident
    // suggestion (design §8).
    let keys: Vec<(i16, i32)> = rows.iter().map(|r| (r.statement, r.position)).collect();
    let suggestions = store::imports::suggestions(&mut *tx, id, &keys).await?;
    let mut lacking = Vec::new();
    for r in &rows {
        let l = labels(r, &suggestions);
        if decision_of(r)? == Some(Decision::New) && !l.missing.is_empty() {
            let names: Vec<&str> = l.missing.iter().map(|l| l.as_str()).collect();
            lacking.push(format!(
                "statement {} row {} ({})",
                r.statement,
                r.position,
                names.join(", ")
            ));
        }
    }
    if !lacking.is_empty() {
        let n = lacking.len();
        lacking.truncate(10);
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "labels-missing",
            "Rows need labels",
        )
        .detail(format!(
            "{n} new rows have labels that are neither chosen nor confidently suggested: {}{}",
            lacking.join("; "),
            if n > 10 { "; …" } else { "" }
        )));
    }
    let mut outcomes = Vec::new();

    let mut out = Committed {
        import_batch_id: None,
        created: 0,
        transfers: 0,
        matched: 0,
        replaced: 0,
        skipped: 0,
        balance_checks: Vec::new(),
    };
    let mut imported = Vec::new();
    let mut seen_in_file = Vec::new();
    for r in &rows {
        let s = statements
            .iter()
            .find(|s| s.position == r.statement)
            .ok_or_else(Problem::internal)?;
        let account = s.effective_account().ok_or_else(Problem::internal)?;
        let status = RowStatus::parse(&r.status)?;
        let decision = decision_of(r)?.ok_or_else(Problem::internal)?;
        if !decision.fits(status) {
            return Err(at_row(r)(Problem::conflict(format!(
                "a {} row can't be {}",
                status.as_str(),
                decision.as_str()
            ))));
        }
        let became = match decision {
            Decision::Skip => {
                out.skipped += 1;
                continue;
            }
            Decision::New => {
                let batch = match out.import_batch_id {
                    Some(b) => b,
                    None => {
                        let b = Uuid::now_v7();
                        let token = principal.actor().map(|t| *t.as_uuid());
                        let label = import.file_name.as_deref().unwrap_or("Imported file");
                        store::import_batches::insert(&mut *tx, b, "file", Some(label), token)
                            .await?;
                        out.import_batch_id = Some(b);
                        b
                    }
                };
                let l = labels(r, &suggestions);
                let quantity = Amount::from_decimal(r.amount.abs());
                let mine = Leg::new(Some(account), None, s.commodity_id, quantity);
                let outside = Leg::new(None, l.counterparty, s.commodity_id, quantity);
                let (from, to) = if r.amount.is_sign_negative() {
                    (mine, outside)
                } else {
                    (outside, mine)
                };
                let description = r
                    .set_description
                    .clone()
                    .unwrap_or_else(|| r.description.chars().take(500).collect());
                let req =
                    NewTransaction::single(r.date, description, from, to, l.purpose, l.category);
                let t = create_in(&mut tx, principal, req, batch)
                    .await
                    .map_err(at_row(r))?;
                out.created += 1;
                for sg in suggestions
                    .iter()
                    .filter(|sg| (sg.statement, sg.position) == (r.statement, r.position))
                {
                    let committed = match Label::parse(&sg.label)? {
                        Label::Counterparty => l.counterparty,
                        Label::Category => l.category,
                        Label::Purpose => l.purpose,
                    };
                    outcomes.push(Outcome {
                        suggester_id: sg.suggester_id,
                        label: sg.label.clone(),
                        suggested: sg.value,
                        confidence: sg.confidence,
                        committed,
                        transaction_id: Some(t.id()),
                    });
                }
                t.id()
            }
            Decision::Replace => {
                let t = r.match_transaction_id.ok_or_else(Problem::internal)?;
                // Labels from review replace the transaction's; with neither
                // yours nor a confident suggestion, its own stay.
                let l = labels(r, &suggestions);
                let set =
                    |label: Label, v: Option<Uuid>| (!l.missing.contains(&label)).then_some(v);
                let description = r
                    .set_description
                    .clone()
                    .unwrap_or_else(|| r.description.chars().take(500).collect());
                replace_with_row(
                    &mut tx,
                    principal,
                    t,
                    account,
                    s.commodity_id,
                    r.date,
                    description,
                    Amount::from_decimal(r.amount),
                    Relabel {
                        counterparty: set(Label::Counterparty, l.counterparty),
                        category: set(Label::Category, l.category),
                        purpose: set(Label::Purpose, l.purpose),
                    },
                )
                .await
                .map_err(at_row(r))?;
                out.replaced += 1;
                t
            }
            Decision::Match => {
                let t = r.match_transaction_id.ok_or_else(Problem::internal)?;
                if status == RowStatus::TransferMatch {
                    into_transfer(&mut tx, principal, t, account)
                        .await
                        .map_err(at_row(r))?;
                    out.transfers += 1;
                } else {
                    out.matched += 1;
                }
                t
            }
        };
        imported.push(Fingerprint {
            account_id: account,
            fingerprint: r.fingerprint.clone(),
            transaction_id: became,
        });
        seen_in_file.push(ImportedRow {
            account_id: account,
            fingerprint: r.fingerprint.clone(),
            file_sha256: import.file_sha256.clone(),
            raw: r.raw.clone(),
        });
    }
    if !imported.is_empty() {
        // A row already recorded (an exact duplicate) keeps its transaction;
        // what this file says about it is added either way.
        store::imports::add_fingerprints(&mut *tx, &imported).await?;
        store::imports::add_imported_rows(&mut *tx, &seen_in_file).await?;
    }
    if let Some(b) = out.import_batch_id {
        store::import_batches::touch(&mut *tx, b).await?;
    }
    for s in &statements {
        if let Some(a) = &s.suggestion {
            outcomes.push(Outcome {
                suggester_id: a.suggester_id,
                label: "account".to_owned(),
                suggested: Some(a.account_id),
                confidence: a.confidence,
                committed: s.effective_account(),
                transaction_id: None,
            });
        }
    }
    if !outcomes.is_empty() {
        store::imports::add_outcomes(&mut *tx, &outcomes).await?;
    }
    for s in &statements {
        if let (Some(amount), Some(date), Some(account)) =
            (s.ledger_amount, s.ledger_date, s.effective_account())
        {
            out.balance_checks.push(
                crate::balance_checks::record(
                    &mut tx,
                    principal,
                    account,
                    s.commodity_id,
                    date,
                    Amount::from_decimal(amount),
                    "ofx",
                )
                .await?,
            );
        }
    }
    store::imports::delete(&mut *tx, id).await?;
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "staged_import",
        id,
        Some(&json!({ "file_name": import.file_name, "committed": true })),
        Some(
            &json!({ "import_batch_id": out.import_batch_id, "created": out.created,
                      "transfers": out.transfers, "matched": out.matched,
                      "replaced": out.replaced,
                      "skipped": out.skipped }),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(axum::Json(out))
}
