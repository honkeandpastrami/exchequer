//! `/api/v1/imports` (design §6): upload a bank file, see what each of its
//! rows would become, fix what needs fixing, then commit it. The file itself
//! isn't kept, only its parsed statements and rows, until the import is
//! committed or discarded.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use domain::{AccountNumber, Amount};
use import::fingerprint::fingerprints;
use import::matching::{
    Near, Side, Staged, near_confidence, one_to_one, transfer_confidence, transfer_conflicts,
    transfer_matches,
};
use import::params::{CHANGED, DISAGREES, MATCHER, NEAR_DAYS, SURE};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use store::audit::Action;
use store::imports::{
    CONFIDENT, Fingerprint, ImportRow, ImportedRow, Matchable, NearCandidate, Outcome, RowKey,
    RowRow, StagedSplit, StatementRow, StatusUpdate, SuggesterRow, SuggestionRow,
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
use crate::suggestions;
use crate::transactions::{Leg, MAX_SPLITS, NewSplit, NewTransaction, create_in};

/// Bank files bigger than this are refused.
const MAX_FILE: usize = 10 * 1024 * 1024;
/// Longest bank ID a row may have (the fingerprint column's limit).
const MAX_ID: usize = 200;
/// Rows one review request may tick.
const MAX_REVIEW: usize = 500;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, upload))
        .routes(routes!(get, discard))
        .routes(routes!(rows))
        .routes(routes!(set_statement))
        .routes(routes!(set_row))
        .routes(routes!(review))
        .routes(routes!(commit))
        .routes(routes!(commit_ready))
        .routes(routes!(refresh))
        .layer(DefaultBodyLimit::max(MAX_FILE))
}

// ------------------------------------------------------------- Schemas ----

/// What matching found for a row (design §6.6): its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RowStatus {
    /// Nothing like it in the ledger.
    New,
    /// Already in the ledger: this bank row, unchanged since it was imported,
    /// or the transfer it's part of, recorded the same day.
    Duplicate,
    /// A ledger transaction that may be it: the same bank row changed since
    /// it was imported, a transfer recorded a few days apart, or the same
    /// amount a few days apart.
    PossibleDuplicate,
    /// The other side of a transfer, staged in another of your accounts (in
    /// this import or another).
    TransferPair,
    /// The statement has no account yet.
    NoAccount,
}

impl RowStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Duplicate => "duplicate",
            Self::PossibleDuplicate => "possible_duplicate",
            Self::TransferPair => "transfer_pair",
            Self::NoAccount => "no_account",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        Ok(match s {
            "new" => Self::New,
            "duplicate" => Self::Duplicate,
            "possible_duplicate" => Self::PossibleDuplicate,
            "transfer_pair" => Self::TransferPair,
            "no_account" => Self::NoAccount,
            _ => return Err(Problem::internal()),
        })
    }

    /// Whether a match in the ledger was found.
    const fn matched(self) -> bool {
        matches!(self, Self::Duplicate | Self::PossibleDuplicate)
    }
}

/// How a duplicate or possible duplicate was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    /// The bank's ID for the row (or, without one, its date, description and
    /// amount) was imported before.
    BankId,
    /// A transfer between the account and another of yours already moved
    /// the same amount the same way.
    Transfer,
    /// A transaction in the account has the same amount, a few days apart.
    Amount,
}

impl MatchKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::BankId => "bank_id",
            Self::Transfer => "transfer",
            Self::Amount => "amount",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        Ok(match s {
            "bank_id" => Self::BankId,
            "transfer" => Self::Transfer,
            "amount" => Self::Amount,
            _ => return Err(Problem::internal()),
        })
    }
}

/// What a row, a statement or an import needs (design §6.7), shown as its
/// colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// Red: a value is missing (a label, a statement's account). Commit
    /// refuses.
    NeedsYou,
    /// Yellow: complete, but something shown is a suggestion or a default
    /// short of certain, not ticked as reviewed. Commit goes ahead.
    ToReview,
    /// Green: it commits exactly as shown.
    Ready,
    /// Grey: a row set to don't add, by you or by default. Nothing to
    /// commit, nothing to review.
    NotAdded,
}

/// How many rows are in each state.
#[derive(Debug, Clone, Copy, Default, Serialize, ToSchema)]
pub struct StateCounts {
    needs_you: u32,
    to_review: u32,
    ready: u32,
    not_added: u32,
}

impl StateCounts {
    fn add(&mut self, state: ReviewState) {
        let slot = match state {
            ReviewState::NeedsYou => &mut self.needs_you,
            ReviewState::ToReview => &mut self.to_review,
            ReviewState::Ready => &mut self.ready,
            ReviewState::NotAdded => &mut self.not_added,
        };
        *slot += 1;
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

/// One account's part of the file.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StagedStatement {
    position: i16,
    /// The last four of the account number in the file (CSVs have none).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "6789")]
    number_last4: Option<String>,
    /// The account the rows are worked out against: the one the file's
    /// number belongs to, or the one you picked, else a confident
    /// suggestion (`account_suggested`). `null` until there is one.
    account_id: Option<Uuid>,
    /// Whether `account_id` is the suggestion's, not yet the statement's own.
    account_suggested: bool,
    /// Whether you ticked the suggested account as reviewed, making it the
    /// statement's own.
    reviewed: bool,
    /// Needs you without an account; to review while the account is a
    /// suggestion short of certain; else ready.
    state: ReviewState,
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
    /// Its rows by state.
    rows: StateCounts,
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

impl From<suggest::Label> for Label {
    fn from(l: suggest::Label) -> Self {
        match l {
            suggest::Label::Counterparty => Self::Counterparty,
            suggest::Label::Category => Self::Category,
            suggest::Label::Purpose => Self::Purpose,
        }
    }
}

/// Which level of the suggestion stack made a suggestion (docs/ai.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuggesterKind {
    Rule,
    Local,
    Remote,
    /// Matching: what's already in the ledger or other staged files.
    Match,
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
                "match" => SuggesterKind::Match,
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
    /// The suggested counterparty, category or party; `null` suggests none,
    /// unless `transfer_account_id` is set.
    value: Option<Uuid>,
    /// For the counterparty: one of your accounts, suggested as the other
    /// side (a transfer).
    #[serde(skip_serializing_if = "Option::is_none")]
    transfer_account_id: Option<Uuid>,
    /// Per mille: 900 is 0.9.
    confidence: i16,
    /// Whether it's confident (at least 900): the level below isn't asked.
    confident: bool,
    reason: String,
    suggester: Suggester,
}

/// What matching suggests choosing for a row: the label's default.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DecisionSuggestion {
    decision: Decision,
    /// Per mille: 900 is 0.9.
    confidence: i16,
    /// Whether it's confident (at least 900).
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
    readiness: Readiness,
    #[serde(skip)]
    version: i64,
}

/// How close an import is to committing (design §6.7).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Readiness {
    /// Needs you if a statement or a row does; else to review if one is;
    /// else ready. Never not added.
    state: ReviewState,
    /// Its rows by state; a statement without an account counts its rows
    /// as needing you.
    rows: StateCounts,
    /// Statements with no account.
    no_account: u32,
    /// Statements whose account is a suggestion short of certain, not
    /// ticked as reviewed.
    accounts_to_review: u32,
    /// Transactions a commit would add now: a transfer pair added as one
    /// transfer is one.
    adds: u32,
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
    /// What matching found: the row's label.
    status: RowStatus,
    /// The existing transaction a duplicate or possible duplicate may be.
    #[serde(skip_serializing_if = "Option::is_none")]
    match_transaction_id: Option<Uuid>,
    /// How that match was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    match_kind: Option<MatchKind>,
    /// For a transfer pair: the staged row that is the transfer's other
    /// side.
    #[serde(skip_serializing_if = "Option::is_none")]
    transfer_pair: Option<TransferPair>,
    /// The choices the row has, in the order shown: `add` and `dont_add`,
    /// and for a transfer pair `add` (one transfer), `add_separately` and
    /// `dont_add`. None while the statement has no account.
    options: Vec<Decision>,
    /// What committing does with the row: your choice, else the label's
    /// default. Absent while the statement has no account.
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<Decision>,
    /// Whether `decision` is your choice rather than the default.
    decided: bool,
    /// Whether you ticked the row as reviewed, and it still shows what you
    /// saw then: what it takes counts as yours.
    reviewed: bool,
    /// Its colour: what it needs.
    state: ReviewState,
    /// What a tick would record now.
    #[serde(skip)]
    seen: Seen,
    /// What matching suggests `decision` be (the label's default, and how
    /// sure); absent while the statement has no account.
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_decision: Option<DecisionSuggestion>,
    /// The new transaction's description, if not the bank's.
    #[serde(skip_serializing_if = "Option::is_none")]
    set_description: Option<String>,
    /// The new transaction's tags.
    tags: Vec<String>,
    /// The labels below are what a new transaction gets: your choice, else a
    /// suggestion. For a new transaction: who's on the outside side
    /// (unknown if absent and there's no `transfer_account_id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    counterparty_id: Option<Uuid>,
    /// The other side when it's one of your accounts: the row is a transfer,
    /// whose category and purpose don't apply (unless one account is liquid
    /// and the other isn't: an expense or income). A transfer pair added as
    /// one transfer has its other row's account.
    #[serde(skip_serializing_if = "Option::is_none")]
    transfer_account_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category_id: Option<Uuid>,
    /// Who the money was for; absent, the account's owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    purpose_id: Option<Uuid>,
    /// The labels you set (a `null` one is an explicit none).
    chosen: Vec<Label>,
    /// At most one per label, confident or not.
    suggestions: Vec<LabelSuggestion>,
    /// For a row that will be added: the labels with neither your choice
    /// nor a suggestion. Commit refuses while any row has some.
    missing: Vec<Label>,
    /// Labels that don't apply: a transfer's category and purpose (design
    /// §3.2).
    not_applicable: Vec<Label>,
    /// The row split into parts, each a split of the new transaction; empty
    /// if it isn't split. A split row is added, and its labels are the
    /// parts': the row's own, and its suggestions, don't apply.
    splits: Vec<RowSplit>,
    /// The row's fields as the file had them.
    #[schema(value_type = Object)]
    raw: serde_json::Value,
}

impl StagedRow {
    /// What a tick on the row records, if it may be ticked: not while it
    /// needs you, nor when it's not added.
    fn tick(&self) -> Result<serde_json::Value, Problem> {
        match self.state {
            ReviewState::NeedsYou => Err(Problem::invalid(format!(
                "statement {} row {} needs a value first",
                self.statement, self.position
            ))),
            ReviewState::NotAdded => Err(Problem::invalid(format!(
                "statement {} row {} isn't added: there's nothing to review",
                self.statement, self.position
            ))),
            ReviewState::ToReview | ReviewState::Ready => {
                serde_json::to_value(&self.seen).map_err(|_| Problem::internal())
            }
        }
    }
}

/// One part of a row split before commit: a split of the new transaction
/// between the statement's account and this part's other side.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RowSplit {
    /// Positive; the parts add up to the row's amount, unsigned.
    #[schema(value_type = String, example = "500.00")]
    amount: Amount,
    /// Who's on the other side; with neither this nor
    /// `transfer_account_id`, outside the household, unknown who.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    counterparty_id: Option<Uuid>,
    /// The other side when it's one of your accounts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transfer_account_id: Option<Uuid>,
    /// Absent: none. Not for a transfer (unless between a liquid and a
    /// non-liquid account).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category_id: Option<Uuid>,
    /// Absent: the account's owner (the household, between a liquid and a
    /// non-liquid account). Not for a transfer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    purpose_id: Option<Uuid>,
    /// The split's own note, 1-500 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl From<&StagedSplit> for RowSplit {
    fn from(s: &StagedSplit) -> Self {
        Self {
            amount: Amount::from_decimal(s.amount),
            counterparty_id: s.counterparty_id,
            transfer_account_id: s.transfer_account_id,
            category_id: s.category_id,
            purpose_id: s.purpose_id,
            description: s.description.clone(),
        }
    }
}

/// A transfer pair's other row.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TransferPair {
    import_id: Uuid,
    statement: i16,
    position: i32,
    account_id: Uuid,
    date: Date,
    description: String,
    /// Its file's name, if it was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
}

/// What committing does with a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Add it: a new transaction (with the next free suffix on its bank ID
    /// for a duplicate). For a transfer pair, the two rows are one transfer.
    /// The default for new rows and transfer pairs.
    Add,
    /// For a transfer pair: each row is a transaction of its own. The
    /// default for a transfer pair where your choice of a row's other side
    /// disagrees.
    AddSeparately,
    /// Don't add it. A duplicate or possible duplicate is recorded as the
    /// transaction it matched, so it doesn't come up again; that's its
    /// default. A transfer pair's two rows are both left out.
    DontAdd,
}

impl Decision {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::AddSeparately => "add_separately",
            Self::DontAdd => "dont_add",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        match s {
            "add" => Ok(Self::Add),
            "add_separately" => Ok(Self::AddSeparately),
            "dont_add" => Ok(Self::DontAdd),
            _ => Err(Problem::internal()),
        }
    }

    /// The choices a row with this label has, in the order shown.
    const fn options(status: RowStatus) -> &'static [Self] {
        match status {
            RowStatus::New | RowStatus::Duplicate | RowStatus::PossibleDuplicate => {
                &[Self::Add, Self::DontAdd]
            }
            RowStatus::TransferPair => &[Self::Add, Self::AddSeparately, Self::DontAdd],
            RowStatus::NoAccount => &[],
        }
    }

    /// Whether the decision is one of the label's choices.
    fn fits(self, status: RowStatus) -> bool {
        Self::options(status).contains(&self)
    }

    /// Whether the row becomes part of the ledger as a new transaction.
    const fn adds(self) -> bool {
        matches!(self, Self::Add | Self::AddSeparately)
    }
}

/// Fields to change; omitted ones stay as they are, `null` clears one.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RowUpdate {
    /// One of the row's `options`; `null`: back to the label's default. A
    /// transfer pair's other row takes the same choice.
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Decision>)]
    decision: Option<Option<Decision>>,
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<String>)]
    set_description: Option<Option<String>>,
    /// The new transaction's tags, by name (1-100 characters each; new ones
    /// are created on commit), replacing the row's.
    tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Uuid>)]
    counterparty_id: Option<Option<Uuid>>,
    /// One of your accounts as the other side: the row is a transfer. It
    /// sets the counterparty label (as yours), so not with
    /// `counterparty_id`; `null` is the same as `counterparty_id: null`.
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Uuid>)]
    transfer_account_id: Option<Option<Uuid>>,
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
    /// Split the row into these parts (two or more, adding up to its
    /// amount), each a split of the new transaction; `null` or `[]`: not
    /// split. Only for a row that's added and isn't a transfer pair; a split
    /// row can't change its decision until the parts are cleared.
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Vec<RowSplit>>)]
    splits: Option<Option<Vec<RowSplit>>>,
    /// `true` ticks the row as reviewed, as it is after this change: you
    /// agree with what it shows, so what it takes counts as yours. Not for
    /// a row that needs you or isn't added. `false` unticks it. The tick
    /// goes by itself when the row changes under it: a new suggestion, a
    /// new label, a transfer pair made or undone.
    reviewed: Option<bool>,
}

/// Rows to tick as reviewed, or untick.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Review {
    /// The rows, as you saw them; at most 500.
    rows: Vec<RowRef>,
    /// `true` ticks the rows that can be (not those that need you or
    /// aren't added; they stay as they are); `false` unticks them all.
    reviewed: bool,
}

/// A staged row of the import.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RowRef {
    statement: i16,
    position: i32,
}

/// What a commit did.
#[derive(Debug, Serialize, ToSchema)]
pub struct Committed {
    /// The batch the new transactions are in; absent if there were none.
    #[serde(skip_serializing_if = "Option::is_none")]
    import_batch_id: Option<Uuid>,
    /// New transactions.
    added: u64,
    /// Of them, transfers made of a transfer pair: two rows, one
    /// transaction.
    transfers: u64,
    /// Rows not added: duplicates recorded as what they matched, and rows
    /// left out.
    not_added: u64,
    /// One per statement with a ledger balance: the bank's balance against
    /// the ledger's.
    balance_checks: Vec<crate::balance_checks::BalanceCheck>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CommitReady {
    /// The imports you saw as ready.
    ids: Vec<Uuid>,
}

/// What committing one of the ready imports did.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadyOutcome {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    /// Absent if it wasn't committed.
    #[serde(skip_serializing_if = "Option::is_none")]
    committed: Option<Committed>,
    /// Why it wasn't committed: gone, no longer ready, changed since it was
    /// checked, or refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReadyCommitted {
    /// Oldest import first.
    results: Vec<ReadyOutcome>,
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
    /// Only rows with this label.
    status: Option<RowStatus>,
    /// Items per page (default 100, max 500).
    limit: Option<u32>,
    /// `next_cursor` from the previous page. Opaque.
    cursor: Option<String>,
}

/// One of the two: pick the account, or tick or untick its suggestion.
/// Picking an account, or ticking its suggestion, also adds the file's
/// account number to it, so its next files resolve by themselves.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StatementUpdate {
    /// The account the statement is for; `null` to unset it.
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Uuid>)]
    account_id: Option<Option<Uuid>>,
    /// `true` ticks the suggested account as reviewed: it becomes the
    /// statement's own. `false` unticks it: back to the suggestion.
    reviewed: Option<bool>,
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

/// The imports as the API shows them, with their statements and readiness.
async fn output(db: &mut PgConnection, imports: Vec<ImportRow>) -> Result<Vec<Import>, Problem> {
    let ids: Vec<Uuid> = imports.iter().map(|i| i.id).collect();
    let statements = store::imports::statements(&mut *db, &ids).await?;
    let by = suggesters(
        db,
        statements
            .iter()
            .filter_map(|(_, s)| s.suggestion.as_ref().map(|a| a.suggester_id)),
    )
    .await?;
    let mut out = Vec::with_capacity(imports.len());
    for i in imports {
        let all = store::imports::rows(&mut *db, i.id, None, None, None, i64::MAX).await?;
        let rows = shown(db, i.id, all).await?;
        let statements = statements
            .iter()
            .filter(|(id, _)| *id == i.id)
            .map(|(_, s)| {
                let mut counts = StateCounts::default();
                for r in rows.iter().filter(|r| r.statement == s.position) {
                    counts.add(r.state);
                }
                Ok(StagedStatement {
                    position: s.position,
                    number_last4: s.account_number.as_ref().map(|n| n.last4().to_owned()),
                    account_id: s.effective_account(),
                    account_suggested: s.account_id.is_none() && s.effective_account().is_some(),
                    reviewed: s.reviewed,
                    state: statement_state(s),
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
                    rows: counts,
                })
            })
            .collect::<Result<Vec<_>, Problem>>()?;
        let readiness = readiness(i.id, &statements, &rows);
        out.push(Import {
            id: i.id,
            file_name: i.file_name,
            file_sha256: i.file_sha256,
            format: format_of(&i.format)?,
            created_at: i.created_at,
            statements,
            readiness,
            version: i.version,
        });
    }
    Ok(out)
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

fn near(candidates: Vec<NearCandidate>) -> Vec<Near<i32>> {
    candidates
        .into_iter()
        .map(|c| Near {
            row: c.position,
            transaction: c.transaction_id,
            similarity: c.similarity,
            days_apart: c.days_apart,
        })
        .collect()
}

/// Works out each row's label in one statement, from what the ledger holds
/// now (design §6.3-§6.4): `no_account` without an account; a
/// `duplicate` when its bank ID was imported into the account before and
/// the bank's row is unchanged (or there's nothing to compare it with), a
/// `possible_duplicate` when it changed; then a transfer between the
/// account and another of yours that already moved the same amount the same
/// way within a few days, holding no bank row of this account yet (a
/// `duplicate` the same day); then a transaction in the account with the
/// same commodity and signed amount within [`NEAR_DAYS`] days; else `new`.
/// Each match is one-to-one. A transfer pair stays one until transfer
/// matching says otherwise ([`rematch`]).
async fn restatus(db: &mut PgConnection, import_id: Uuid, statement: i16) -> Result<(), Problem> {
    let (_, s) = store::imports::statements(&mut *db, &[import_id])
        .await?
        .into_iter()
        .find(|(_, s)| s.position == statement)
        .ok_or_else(Problem::internal)?;
    let rows =
        store::imports::rows(&mut *db, import_id, Some(statement), None, None, i64::MAX).await?;
    let mut matched: HashMap<i32, (RowStatus, MatchKind, Uuid)> = HashMap::new();
    // What each candidate match found: days apart and, for the same
    // amount, how alike.
    let mut found: HashMap<(i32, Uuid), (i32, Option<i32>)> = HashMap::new();
    let account = s.effective_account();
    if let Some(account) = account {
        let fps: Vec<String> = rows.iter().map(|r| r.fingerprint.clone()).collect();
        let known = store::imports::known_fingerprints(&mut *db, account, &fps).await?;
        for r in &rows {
            let mine: Vec<_> = known.iter().filter(|k| k.asked == r.fingerprint).collect();
            let same = mine
                .iter()
                .find(|k| k.raws.contains(&r.raw))
                .or_else(|| mine.iter().find(|k| k.raws.is_empty()));
            if let Some(k) = same {
                matched.insert(
                    r.position,
                    (RowStatus::Duplicate, MatchKind::BankId, k.transaction_id),
                );
            } else if let Some(k) = mine.first() {
                matched.insert(
                    r.position,
                    (
                        RowStatus::PossibleDuplicate,
                        MatchKind::BankId,
                        k.transaction_id,
                    ),
                );
            }
        }
        let days = i32::try_from(suggest::TRANSFER_DAYS).unwrap_or(i32::MAX);
        for (kind, candidates) in [
            (
                MatchKind::Transfer,
                store::imports::transfer_duplicates(
                    &mut *db,
                    import_id,
                    statement,
                    account,
                    s.commodity_id,
                    days,
                )
                .await?,
            ),
            (
                MatchKind::Amount,
                store::imports::near_candidates(
                    &mut *db,
                    import_id,
                    statement,
                    account,
                    s.commodity_id,
                    NEAR_DAYS,
                )
                .await?,
            ),
        ] {
            for c in &candidates {
                let sim = (kind == MatchKind::Amount).then_some(c.similarity);
                found.insert((c.position, c.transaction_id), (c.days_apart, sim));
            }
            let done: HashSet<i32> = matched.keys().copied().collect();
            let taken: HashSet<Uuid> = matched.values().map(|m| m.2).collect();
            for (position, t) in one_to_one(near(candidates), &done, &taken) {
                let same_day = found.get(&(position, t)).is_some_and(|f| f.0 == 0);
                let status = if kind == MatchKind::Transfer && same_day {
                    RowStatus::Duplicate
                } else {
                    RowStatus::PossibleDuplicate
                };
                matched.insert(position, (status, kind, t));
            }
        }
    }
    let small = |n: i32| i16::try_from(n).ok();
    let statuses: Vec<StatusUpdate> = rows
        .iter()
        .map(|r| {
            let (status, kind, t) = match (account, matched.get(&r.position)) {
                (None, _) => (RowStatus::NoAccount, None, None),
                (Some(_), Some((status, kind, t))) => (*status, Some(*kind), Some(*t)),
                (Some(_), None) if r.status == RowStatus::TransferPair.as_str() => {
                    (RowStatus::TransferPair, None, None)
                }
                (Some(_), None) => (RowStatus::New, None, None),
            };
            let (days, similarity) = t
                .and_then(|t| found.get(&(r.position, t)))
                .map_or((None, None), |(d, sim)| (small(*d), sim.and_then(small)));
            StatusUpdate {
                position: r.position,
                status: status.as_str().to_owned(),
                transaction: t,
                kind: kind.map(|k| k.as_str().to_owned()),
                days,
                similarity,
            }
        })
        .collect();
    store::imports::set_statuses(&mut *db, import_id, statement, &statuses).await?;
    Ok(())
}

/// A staged row's other side, as transfer matching sees it: your choice
/// only. A suggestion was made without seeing the other side; a pair
/// settles it.
fn side(r: &Matchable) -> Side {
    if !r.chose_side {
        return Side::Open;
    }
    r.transfer_account_id.map_or(Side::Outside, Side::Transfer)
}

fn key(r: &Matchable) -> RowKey {
    (r.import_id, r.statement, r.position)
}

fn pair_of(r: &Matchable) -> Option<RowKey> {
    Some((r.pair_import_id?, r.pair_statement?, r.pair_position?))
}

fn conflict_of(r: &Matchable) -> Option<RowKey> {
    Some((
        r.conflict_import_id?,
        r.conflict_statement?,
        r.conflict_position?,
    ))
}

/// Works out transfer pairs among the rows of every staged import (design
/// §6.4): two new rows in two of your accounts, with opposite amounts a few
/// days apart, unless your choice of either's other side says otherwise
/// ([`import::matching`]). A row you chose as a transfer to one account
/// that fits a row in another is a transfer pair too, a conflicting one:
/// your choice stays its default ([`guess`]). A row's choice doesn't change
/// whether it's paired, so choosing never changes its label. Touches and
/// returns the imports whose rows changed.
async fn rematch(db: &mut PgConnection) -> Result<HashSet<Uuid>, Problem> {
    let rows = store::imports::matchable(&mut *db).await?;
    let open = |r: &Matchable| {
        matches!(
            RowStatus::parse(&r.status),
            Ok(RowStatus::New | RowStatus::TransferPair)
        )
    };
    let staged: Vec<Staged<RowKey>> = rows
        .iter()
        .filter_map(|r| {
            Some(Staged {
                key: key(r),
                account: r.account_id?,
                commodity: r.commodity_id,
                date: r.date,
                amount: r.amount,
                side: side(r),
                // A split row is added as its parts, never one side of a
                // transfer.
                eligible: open(r) && !r.split,
            })
        })
        .collect();
    let pairs = transfer_matches(&staged);
    let conflicts = transfer_conflicts(&staged, &pairs);
    let mut changes = Vec::new();
    let mut changed = HashSet::new();
    for r in rows.iter().filter(|r| open(r)) {
        let pair = pairs.get(&key(r)).copied();
        let conflict = conflicts.get(&key(r)).copied();
        let status = if pair.is_some() || conflict.is_some() {
            RowStatus::TransferPair
        } else {
            RowStatus::New
        };
        if status.as_str() != r.status || pair != pair_of(r) || conflict != conflict_of(r) {
            changes.push((key(r), status.as_str().to_owned(), pair, conflict));
            changed.insert(r.import_id);
        }
    }
    if !changes.is_empty() {
        store::imports::set_pairs(&mut *db, &changes).await?;
    }
    for id in &changed {
        store::imports::touch(&mut *db, *id).await?;
    }
    Ok(changed)
}

/// What matching shows of a row: label, match and transfer pair.
type Matched = HashMap<RowKey, (String, Option<Uuid>, Option<RowKey>, Option<RowKey>)>;

async fn matched(db: &mut PgConnection) -> Result<Matched, Problem> {
    Ok(store::imports::matchable(&mut *db)
        .await?
        .into_iter()
        .map(|r| {
            (
                key(&r),
                (
                    r.status.clone(),
                    r.match_transaction_id,
                    pair_of(&r),
                    conflict_of(&r),
                ),
            )
        })
        .collect())
}

/// Every staged statement but `except`'s, as (import, statement).
async fn staged_statements(
    db: &mut PgConnection,
    except: Option<Uuid>,
) -> Result<Vec<(Uuid, i16)>, Problem> {
    let mut statements: Vec<(Uuid, i16)> = matched(db)
        .await?
        .into_keys()
        .filter(|k| Some(k.0) != except)
        .map(|k| (k.0, k.1))
        .collect();
    statements.sort_unstable();
    statements.dedup();
    Ok(statements)
}

/// Suggests these statements' labels again where they aren't yours or
/// confidently suggested (design §8), and touches the imports whose
/// suggestions changed.
async fn resuggest(
    db: &mut PgConnection,
    levels: &[Arc<dyn suggest::Suggester>],
    statements: &[(Uuid, i16)],
) -> Result<HashSet<Uuid>, Problem> {
    let mut changed = HashSet::new();
    for (import, statement) in statements {
        if suggestions::labels(db, levels, *import, *statement, true).await? {
            changed.insert(*import);
        }
    }
    for id in &changed {
        store::imports::touch(&mut *db, *id).await?;
    }
    Ok(changed)
}

/// After the ledger changed: works out the rows of every other staged
/// import again, suggests their open labels again, and touches those whose
/// rows changed.
async fn refresh_others(
    db: &mut PgConnection,
    levels: &[Arc<dyn suggest::Suggester>],
    except: Uuid,
) -> Result<(), Problem> {
    let before = matched(db).await?;
    let statements = staged_statements(db, Some(except)).await?;
    for (import, statement) in &statements {
        restatus(db, *import, *statement).await?;
    }
    let mut touched = resuggest(db, levels, &statements).await?;
    touched.extend(rematch(db).await?);
    let after = matched(db).await?;
    for (k, v) in &after {
        if before.get(k) != Some(v) && !touched.contains(&k.0) {
            store::imports::touch(&mut *db, k.0).await?;
            touched.insert(k.0);
        }
    }
    Ok(())
}

/// What a new transaction would get for each label: your choice, else a
/// suggestion; a label with neither is missing. The other side is a
/// counterparty or, for a transfer, one of your accounts (a transfer pair
/// added as one transfer has its other row's); a transfer has no category
/// or purpose unless one account is liquid and the other isn't (design
/// §3.2).
#[derive(Debug, PartialEq, Eq)]
struct Labels {
    counterparty: Option<Uuid>,
    transfer: Option<Uuid>,
    category: Option<Uuid>,
    purpose: Option<Uuid>,
    missing: Vec<Label>,
    not_applicable: Vec<Label>,
    /// The suggestions it takes.
    suggested_by: Vec<Took>,
}

/// A suggestion a row's labels take: for which label, what, by whom, and
/// how sure.
#[derive(Debug, PartialEq, Eq)]
struct Took {
    taken: Taken,
    suggester: Uuid,
    confidence: i16,
}

/// A suggested value a row took, as a tick records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Taken {
    label: String,
    value: Option<Uuid>,
    account: Option<Uuid>,
}

impl Took {
    fn of(s: &SuggestionRow) -> Self {
        Self {
            taken: Taken {
                label: s.label.clone(),
                value: s.value,
                account: s.account_id,
            },
            suggester: s.suggester_id,
            confidence: s.confidence,
        }
    }
}

/// What a row showed when you ticked it as reviewed (design §6.7): its
/// label, its transfer pair (and whether it conflicts with your choice),
/// and the AI suggestions short of certain it took. The tick holds while
/// the label and pair are the same and it takes no AI suggestion it didn't
/// then; one you since made your own choice doesn't undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Seen {
    status: String,
    pair: Option<RowKey>,
    #[serde(default)]
    conflict: bool,
    suggestions: Vec<Taken>,
}

impl Seen {
    fn now(r: &RowRow, l: &Labels, by: &HashMap<Uuid, Suggester>) -> Self {
        Self {
            status: r.status.clone(),
            pair: partner(r),
            conflict: conflict_key(r).is_some(),
            suggestions: l
                .suggested_by
                .iter()
                .filter(|t| unsure(t, by))
                .map(|t| t.taken.clone())
                .collect(),
        }
    }

    /// Whether a tick that saw `self` still holds for a row showing `now`.
    fn holds(&self, now: &Self) -> bool {
        self.status == now.status
            && self.pair == now.pair
            && self.conflict == now.conflict
            && now.suggestions.iter().all(|t| self.suggestions.contains(t))
    }
}

/// An AI suggestion short of certain: worth a look. A certain one, like
/// logic's, counts as yours.
fn unsure(t: &Took, by: &HashMap<Uuid, Suggester>) -> bool {
    !certain(t.confidence)
        && by
            .get(&t.suggester)
            .is_some_and(|s| matches!(s.kind, SuggesterKind::Local | SuggesterKind::Remote))
}

/// Whether the row's tick holds for what it shows now.
fn reviewed(r: &RowRow, now: &Seen) -> bool {
    r.reviewed
        .clone()
        .and_then(|v| serde_json::from_value::<Seen>(v).ok())
        .is_some_and(|seen| seen.holds(now))
}

/// What a row's labels depend on beyond the row.
struct Context<'a> {
    /// Your accounts: money between a liquid and a non-liquid one is an
    /// expense or income, not a transfer.
    accounts: &'a [suggest::Account],
    /// The row's statement's account.
    account: Option<Uuid>,
    /// A transfer pair added as one transfer: its other row's account.
    pair_account: Option<Uuid>,
    /// The row's parts, if it's split: their labels are all yours.
    splits: &'a [StagedSplit],
}

/// `suggestions` may hold other rows' too.
fn labels(r: &RowRow, suggestions: &[SuggestionRow], cx: &Context) -> Labels {
    let mut out = Labels {
        counterparty: r.counterparty_id,
        transfer: r.transfer_account_id,
        category: r.category_id,
        purpose: r.purpose_id,
        missing: Vec::new(),
        not_applicable: Vec::new(),
        suggested_by: Vec::new(),
    };
    // A split row's labels are its parts', all yours.
    if !cx.splits.is_empty() {
        return out;
    }
    let chosen = |label: Label| r.chosen.iter().any(|c| c == label.as_str());
    let suggested = |label: Label| {
        suggestions.iter().find(|s| {
            (s.statement, s.position) == (r.statement, r.position) && s.label == label.as_str()
        })
    };
    if let Some(a) = cx.pair_account {
        (out.counterparty, out.transfer) = (None, Some(a));
    } else if !chosen(Label::Counterparty) {
        match suggested(Label::Counterparty) {
            Some(s) => {
                (out.counterparty, out.transfer) = (s.value, s.account_id);
                out.suggested_by.push(Took::of(s));
            }
            None => {
                (out.counterparty, out.transfer) = (None, None);
                out.missing.push(Label::Counterparty);
            }
        }
    }
    let applies = out.transfer.is_none_or(|a| {
        suggest::labeled(
            cx.accounts,
            cx.account,
            r.amount,
            suggest::Value::Account(a),
        )
    });
    for label in [Label::Category, Label::Purpose] {
        let slot = match label {
            Label::Category => &mut out.category,
            _ => &mut out.purpose,
        };
        if !applies {
            *slot = None;
            out.not_applicable.push(label);
            continue;
        }
        // A purpose names someone and a category something: blank is
        // neither (design §4.1).
        if chosen(label) {
            if slot.is_none() {
                out.missing.push(label);
            }
            continue;
        }
        match suggested(label).filter(|s| s.value.is_some()) {
            Some(s) => {
                *slot = s.value;
                out.suggested_by.push(Took::of(s));
            }
            None => {
                *slot = None;
                out.missing.push(label);
            }
        }
    }
    out
}

/// The decision committing carries out for a row: yours, else the label's
/// default; `None` while its statement has no account.
fn decision_of(r: &RowRow) -> Result<Option<Decision>, Problem> {
    if RowStatus::parse(&r.status)? == RowStatus::NoAccount {
        return Ok(None);
    }
    match r.decision.as_deref() {
        Some(d) => Ok(Some(Decision::parse(d)?)),
        None => Ok(guess(r, None)?.map(|g| g.decision)),
    }
}

// ------------------------------------------------ Matching, suggesting ----
// A row's default is a suggestion (design §6.7, docs/ai.md): matching is
// logic, like the rules, and as sure as what it found.

/// What matching suggests for a row.
#[derive(Debug, Clone)]
struct Guess {
    decision: Decision,
    confidence: i16,
    reason: String,
}

impl Guess {
    const fn confident(&self) -> bool {
        self.confidence >= CONFIDENT
    }
}

fn apart(days: i32) -> String {
    match days {
        0 => "the same day".to_owned(),
        1 => "a day apart".to_owned(),
        d => format!("{d} days apart"),
    }
}

/// What matching suggests committing does with a row (its label's default),
/// and how sure it is; `None` while its statement has no account.
/// `pair_date`: a transfer pair's other row's. A transfer pair that
/// conflicts with your choice of a row's other side keeps your choice: the
/// rows are added separately.
fn guess(r: &RowRow, pair_date: Option<Date>) -> Result<Option<Guess>, Problem> {
    let status = RowStatus::parse(&r.status)?;
    let kind = r.match_kind.as_deref().map(MatchKind::parse).transpose()?;
    // The two sides of a transfer: surer the closer they're dated.
    let transfer = |days: Option<i32>, what: &str| {
        let reason = days.map_or_else(|| what.to_owned(), |d| format!("{what}, {}", apart(d)));
        (transfer_confidence(days), reason)
    };
    let (decision, (confidence, reason)) = match (status, kind) {
        (RowStatus::NoAccount, _) => return Ok(None),
        (RowStatus::New, _) => (
            Decision::Add,
            (SURE, "nothing like it in the ledger".to_owned()),
        ),
        (RowStatus::TransferPair, _) if conflict_key(r).is_some() => (
            Decision::AddSeparately,
            (
                DISAGREES,
                "it fits as the other side, but you chose another account".to_owned(),
            ),
        ),
        (RowStatus::TransferPair, _) => (
            Decision::Add,
            transfer(
                pair_date
                    .map(|p| i32::try_from((r.date - p).whole_days().abs()).unwrap_or(i32::MAX)),
                "the other side of a transfer",
            ),
        ),
        (RowStatus::Duplicate, Some(MatchKind::BankId)) => (
            Decision::DontAdd,
            (SURE, "this bank row was imported before".to_owned()),
        ),
        (_, Some(MatchKind::BankId)) => (
            Decision::DontAdd,
            (
                CHANGED,
                "the same bank ID, changed since it was imported".to_owned(),
            ),
        ),
        (_, Some(MatchKind::Transfer)) => (
            Decision::DontAdd,
            transfer(
                r.match_days.map(i32::from),
                "the transfer is already recorded",
            ),
        ),
        (_, Some(MatchKind::Amount)) => (
            Decision::DontAdd,
            match (r.match_days, r.match_similarity) {
                (Some(d), Some(sim)) => {
                    let (d, sim) = (i32::from(d), i32::from(sim));
                    (
                        near_confidence(d, sim),
                        format!(
                            "the same amount, {}, descriptions {}% alike",
                            apart(d),
                            sim / 10
                        ),
                    )
                }
                _ => (0, "the same amount, a few days apart".to_owned()),
            },
        ),
        (_, None) => return Err(Problem::internal()),
    };
    Ok(Some(Guess {
        decision,
        confidence: i16::try_from(confidence).unwrap_or(0),
        reason,
    }))
}

/// The matcher, as the API shows a suggester.
async fn matcher(db: &mut PgConnection) -> Result<Suggester, Problem> {
    let id = matcher_id(db).await?;
    known(&suggesters(db, [id]).await?, id)
}

async fn matcher_id(db: &mut PgConnection) -> Result<Uuid, Problem> {
    Ok(store::imports::suggester(db, "match", MATCHER.0, MATCHER.1, &json!({})).await?)
}

/// Whether a confidence is as sure as your own choice.
fn certain(confidence: i16) -> bool {
    i32::from(confidence) >= i32::from(suggest::CERTAIN)
}

/// A statement's colour: no account needs you; a suggested account short of
/// certain is to review; else it's ready.
fn statement_state(s: &StatementRow) -> ReviewState {
    match (s.account_id, s.effective_account()) {
        (_, None) => ReviewState::NeedsYou,
        (None, Some(_)) if !s.suggestion.as_ref().is_some_and(|a| certain(a.confidence)) => {
            ReviewState::ToReview
        }
        _ => ReviewState::Ready,
    }
}

/// How close an import is to committing ([`Readiness`]), from its
/// statements and all its rows.
fn readiness(import_id: Uuid, statements: &[StagedStatement], rows: &[StagedRow]) -> Readiness {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let mut out = Readiness {
        state: ReviewState::Ready,
        rows: StateCounts::default(),
        no_account: count(
            statements
                .iter()
                .filter(|s| s.state == ReviewState::NeedsYou)
                .count(),
        ),
        accounts_to_review: count(
            statements
                .iter()
                .filter(|s| s.state == ReviewState::ToReview)
                .count(),
        ),
        adds: 0,
    };
    for r in rows {
        out.rows.add(r.state);
        let adds = match (r.decision, &r.transfer_pair) {
            // One transfer for two rows: counted once in an import holding
            // both.
            (Some(Decision::Add), Some(p)) if r.status == RowStatus::TransferPair => {
                p.import_id != import_id || (r.statement, r.position) < (p.statement, p.position)
            }
            (Some(d), _) => d.adds(),
            (None, _) => false,
        };
        if adds {
            out.adds += 1;
        }
    }
    out.state = if out.no_account + out.rows.needs_you > 0 {
        ReviewState::NeedsYou
    } else if out.accounts_to_review + out.rows.to_review > 0 {
        ReviewState::ToReview
    } else {
        ReviewState::Ready
    };
    out
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
    let statements: HashMap<i16, Option<Uuid>> = store::imports::statements(&mut *db, &[import_id])
        .await?
        .into_iter()
        .map(|(_, s)| (s.position, s.effective_account()))
        .collect();
    let accounts = suggestions::accounts(db).await?;
    let pairs = pairs(db, &rows).await?;
    let matcher = matcher(db).await?;
    let splits = split_rows(db, import_id).await?;
    rows.into_iter()
        .map(|r| {
            let mine: Vec<SuggestionRow> = all
                .iter()
                .filter(|s| (s.statement, s.position) == (r.statement, r.position))
                .cloned()
                .collect();
            let pair = partner(&r).and_then(|k| pairs.get(&k)).cloned();
            let account = statements.get(&r.statement).copied().flatten();
            let parts = parts(&splits, &r);
            staged_row(r, &mine, &by, &matcher, &accounts, account, pair, parts)
        })
        .collect()
}

/// An import's split rows' parts, by row.
async fn split_rows(
    db: &mut PgConnection,
    import_id: Uuid,
) -> Result<HashMap<(i16, i32), Vec<StagedSplit>>, Problem> {
    let mut out: HashMap<(i16, i32), Vec<StagedSplit>> = HashMap::new();
    for s in store::imports::splits(&mut *db, import_id).await? {
        out.entry((s.statement, s.position)).or_default().push(s);
    }
    Ok(out)
}

/// A row's parts; none if it isn't split.
fn parts<'a>(splits: &'a HashMap<(i16, i32), Vec<StagedSplit>>, r: &RowRow) -> &'a [StagedSplit] {
    splits
        .get(&(r.statement, r.position))
        .map_or(&[], Vec::as_slice)
}

fn pair_key(r: &RowRow) -> Option<RowKey> {
    Some((r.pair_import_id?, r.pair_statement?, r.pair_position?))
}

fn conflict_key(r: &RowRow) -> Option<RowKey> {
    Some((
        r.conflict_import_id?,
        r.conflict_statement?,
        r.conflict_position?,
    ))
}

/// A transfer pair's other row: paired, or conflicting with your choice.
fn partner(r: &RowRow) -> Option<RowKey> {
    pair_key(r).or_else(|| conflict_key(r))
}

/// The other rows of these rows' transfer pairs.
async fn pairs(
    db: &mut PgConnection,
    rows: &[RowRow],
) -> Result<HashMap<RowKey, TransferPair>, Problem> {
    let keys: Vec<RowKey> = rows.iter().filter_map(partner).collect();
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(store::imports::pair_rows(&mut *db, &keys)
        .await?
        .into_iter()
        .filter_map(|p| {
            Some((
                (p.import_id, p.statement, p.position),
                TransferPair {
                    import_id: p.import_id,
                    statement: p.statement,
                    position: p.position,
                    account_id: p.account_id?,
                    date: p.date,
                    description: p.description,
                    file_name: p.file_name,
                },
            ))
        })
        .collect())
}

#[expect(clippy::too_many_arguments, reason = "a row and what it depends on")]
fn staged_row(
    r: RowRow,
    suggestions: &[SuggestionRow],
    by: &HashMap<Uuid, Suggester>,
    matcher: &Suggester,
    accounts: &[suggest::Account],
    account: Option<Uuid>,
    pair: Option<TransferPair>,
    splits: &[StagedSplit],
) -> Result<StagedRow, Problem> {
    let status = RowStatus::parse(&r.status)?;
    let g = guess(&r, pair.as_ref().map(|m| m.date))?;
    let chosen = r.decision.as_deref().map(Decision::parse).transpose()?;
    let decision = decision_of(&r)?;
    let pair = pair.filter(|_| status == RowStatus::TransferPair);
    let cx = Context {
        accounts,
        account,
        // Added as one transfer, the other side is the other row's account.
        pair_account: pair
            .as_ref()
            .filter(|_| decision == Some(Decision::Add))
            .map(|p| p.account_id),
        splits,
    };
    let l = labels(&r, suggestions, &cx);
    let seen = Seen::now(&r, &l, by);
    let reviewed = reviewed(&r, &seen);
    let missing = if decision.is_some_and(Decision::adds) {
        l.missing
    } else {
        Vec::new()
    };
    // Something shown short of certain: an AI suggestion it takes, or a
    // default matching isn't sure of.
    let unsure = (decision.is_some_and(Decision::adds) && !seen.suggestions.is_empty())
        || (chosen.is_none() && g.as_ref().is_some_and(|g| !certain(g.confidence)));
    let state = match decision {
        None => ReviewState::NeedsYou,
        Some(Decision::DontAdd) => ReviewState::NotAdded,
        Some(_) if !missing.is_empty() => ReviewState::NeedsYou,
        Some(_) if unsure && !reviewed => ReviewState::ToReview,
        Some(_) => ReviewState::Ready,
    };
    let suggestions = suggestions
        .iter()
        .map(|s| {
            Ok(LabelSuggestion {
                label: Label::parse(&s.label)?,
                value: s.value,
                transfer_account_id: s.account_id,
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
        match_kind: r.match_kind.as_deref().map(MatchKind::parse).transpose()?,
        transfer_pair: pair,
        options: Decision::options(status).to_vec(),
        decision,
        decided: chosen.is_some(),
        reviewed,
        state,
        seen,
        suggested_decision: g.map(|g| DecisionSuggestion {
            decision: g.decision,
            confidence: g.confidence,
            confident: g.confident(),
            reason: g.reason,
            suggester: matcher.clone(),
        }),
        set_description: r.set_description,
        tags: r.tags,
        counterparty_id: l.counterparty,
        transfer_account_id: l.transfer,
        category_id: l.category,
        purpose_id: l.purpose,
        chosen: Label::ALL
            .into_iter()
            .filter(|l| r.chosen.iter().any(|c| c == l.as_str()))
            .collect(),
        suggestions,
        missing,
        not_applicable: l.not_applicable,
        splits: splits.iter().map(RowSplit::from).collect(),
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

/// Adds a file's account number to an account.
async fn remember(
    db: &mut PgConnection,
    account: Uuid,
    number: &AccountNumber,
) -> Result<(), Problem> {
    store::account_numbers::insert(&mut *db, Uuid::now_v7(), account, number).await?;
    store::accounts::touch(&mut *db, account).await?;
    Ok(())
}

/// The account a statement is for: the one its number belongs to, else the
/// one the client gave, which then remembers the number.
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
            if let Some(n) = number {
                remember(db, g, n).await?;
            }
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

/// Get one staged import, with its statements and its rows by state.
#[utoipa::path(get, path = "/imports/{id}", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Import, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let mut db = state.pool.acquire().await?;
    Ok(respond(StatusCode::OK, &one(&mut db, id).await?))
}

/// Upload a bank file (OFX, QFX, QBO or CSV; the format is read from the
/// content). Its rows are staged, each with what it would become, and
/// suggestions are made: the account of a statement that has none, and every
/// row's labels (docs/ai.md). Nothing enters the ledger until the import is
/// committed.
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
    let levels = state.suggestions.levels(&mut tx).await?;
    let mut positions = Vec::new();
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
                match_kind: None,
                decision: None,
                set_description: None,
                counterparty_id: None,
                transfer_account_id: None,
                category_id: None,
                purpose_id: None,
                chosen: Vec::new(),
                pair_import_id: None,
                pair_statement: None,
                pair_position: None,
                conflict_import_id: None,
                conflict_statement: None,
                conflict_position: None,
                reviewed: None,
                match_days: None,
                match_similarity: None,
                tags: Vec::new(),
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
            reviewed: false,
        };
        store::imports::insert_statement(&mut *tx, id, &statement).await?;
        if !rows.is_empty() {
            store::imports::insert_rows(&mut *tx, id, &rows).await?;
        }
        suggestions::account(&mut tx, &levels, id, file_name, &statement).await?;
        restatus(&mut tx, id, position).await?;
        positions.push(position);
    }
    // Labels once every statement is in: each sees the others' rows as
    // potential transfer matches.
    for position in positions {
        suggestions::labels(&mut tx, &levels, id, position, false).await?;
    }
    // The other staged imports' rows now see this one's as potential
    // transfer pairs.
    let others = staged_statements(&mut tx, Some(id)).await?;
    resuggest(&mut tx, &levels, &others).await?;
    rematch(&mut tx).await?;
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

/// Set the account a statement is for, or tick its suggested account as
/// reviewed; either remembers the file's account number for the account.
/// Its rows' labels are worked out again, and their suggestions made again
/// (yours stay).
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
    let (account, reviewed) = match (req.account_id, req.reviewed) {
        (Some(_), Some(_)) => {
            return Err(Problem::invalid(
                "account_id and reviewed: one or the other",
            ));
        }
        (Some(a), None) => (a, false),
        (None, Some(true)) => {
            let Some(a) = statement.suggestion.as_ref().map(|a| a.account_id) else {
                return Err(Problem::invalid(
                    "reviewed: the statement has no suggested account",
                ));
            };
            if statement.account_id.is_some_and(|own| own != a) {
                return Err(Problem::invalid(
                    "reviewed: the statement's account isn't the suggested one",
                ));
            }
            (Some(a), true)
        }
        (None, Some(false)) if statement.reviewed => (None, false),
        (None, Some(false)) => (statement.account_id, false),
        (None, None) => return Err(Problem::invalid("account_id or reviewed: one of them")),
    };
    if let Some(a) = account {
        store::accounts::get(&mut *tx, a, true)
            .await?
            .ok_or_else(|| Problem::invalid("account_id: no such account"))?;
    }
    // Picked or confirmed: the account's next files resolve by themselves.
    if let (Some(number), Some(a)) = (&statement.account_number, account) {
        match store::account_numbers::find(&mut *tx, number).await? {
            Some(n) if n.account_id == a => {}
            Some(_) => {
                return Err(Problem::conflict(format!(
                    "{number} belongs to another account: change it on the account first"
                )));
            }
            None => remember(&mut tx, a, number).await?,
        }
    }
    store::imports::set_account(&mut *tx, id, position, account, reviewed).await?;
    restatus(&mut tx, id, position).await?;
    // The account is context for the rows' labels.
    let levels = state.suggestions.levels(&mut tx).await?;
    suggestions::labels(&mut tx, &levels, id, position, false).await?;
    rematch(&mut tx).await?;
    store::imports::touch(&mut *tx, id).await?;
    let after = one(&mut tx, id).await?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "staged_import",
        id,
        None,
        Some(&json!({ "statement": position, "account_id": account, "reviewed": reviewed })),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Work an import out again without uploading it again: its rows' labels
/// against the ledger, the suggestions that aren't confident (the
/// statement's account, then each row's labels; yours and confident ones
/// stay) and transfer pairs across every staged import.
#[utoipa::path(post, path = "/imports/{id}/refresh", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = OK, body = Import, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json")))]
async fn refresh(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let current = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current != version {
        return Err(Problem::stale());
    }
    let import = store::imports::get(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    let levels = state.suggestions.levels(&mut tx).await?;
    let statements = store::imports::statements(&mut *tx, &[id]).await?;
    for (_, s) in &statements {
        if s.suggestion
            .as_ref()
            .is_none_or(|a| a.confidence < store::imports::CONFIDENT)
        {
            suggestions::account(&mut tx, &levels, id, import.file_name.as_deref(), s).await?;
        }
        restatus(&mut tx, id, s.position).await?;
    }
    let positions: Vec<(Uuid, i16)> = statements.iter().map(|(_, s)| (id, s.position)).collect();
    resuggest(&mut tx, &levels, &positions).await?;
    rematch(&mut tx).await?;
    store::imports::touch(&mut *tx, id).await?;
    let after = one(&mut tx, id).await?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "staged_import",
        id,
        None,
        Some(&json!({ "refresh": true })),
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
    // Its rows' transfer pairs in other imports are gone.
    rematch(&mut tx).await?;
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

/// Choose what committing does with a row, and label the transaction it
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
    let status = RowStatus::parse(&row.status)?;
    if let Some(d) = req.decision {
        if let Some(d) = d
            && !d.fits(status)
        {
            return Err(Problem::invalid(format!(
                "a {} row can't be {}",
                status.as_str(),
                d.as_str()
            )));
        }
        row.decision = d.map(|d| d.as_str().to_owned());
    }
    if let Some(tags) = req.tags {
        let mut tags: Vec<String> = tags.iter().map(|t| t.trim().to_owned()).collect();
        if tags.iter().any(|t| t.is_empty() || t.chars().count() > 100) {
            return Err(Problem::invalid("tags must be 1-100 characters"));
        }
        tags.sort();
        tags.dedup();
        row.tags = tags;
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
    // The other side: a counterparty, or one of your accounts.
    let counterparty = match (req.counterparty_id, req.transfer_account_id) {
        (Some(_), Some(Some(_))) => {
            return Err(Problem::invalid(
                "counterparty_id and transfer_account_id: one or the other",
            ));
        }
        (Some(c), _) => Some((c, None)),
        (None, Some(Some(a))) => {
            store::accounts::get(&mut *tx, a, false)
                .await?
                .ok_or_else(|| Problem::invalid("transfer_account_id: no such account"))?;
            let (_, s) = store::imports::statements(&mut *tx, &[id])
                .await?
                .into_iter()
                .find(|(_, s)| s.position == statement)
                .ok_or_else(Problem::internal)?;
            if s.effective_account() == Some(a) {
                return Err(Problem::invalid(
                    "transfer_account_id: the statement's own account",
                ));
            }
            Some((None, Some(a)))
        }
        (None, Some(None)) => Some((None, None)),
        (None, None) => None,
    };
    if let Some((c, a)) = counterparty {
        row.counterparty_id = c;
        row.transfer_account_id = a;
    }
    for (label, set, values) in [
        (
            Label::Counterparty,
            counterparty.is_some(),
            vec![&mut row.counterparty_id, &mut row.transfer_account_id],
        ),
        (
            Label::Category,
            req.category_id.is_some(),
            vec![&mut row.category_id],
        ),
        (
            Label::Purpose,
            req.purpose_id.is_some(),
            vec![&mut row.purpose_id],
        ),
    ] {
        let name = label.as_str().to_owned();
        if req.revert.contains(&label) {
            if set {
                return Err(Problem::invalid(format!(
                    "{name}: set and reverted at once"
                )));
            }
            for v in values {
                *v = None;
            }
            row.chosen.retain(|c| *c != name);
        } else if set && !row.chosen.contains(&name) {
            row.chosen.push(name);
        }
    }
    if let Some(v) = req.category_id {
        if v.is_none() {
            return Err(Problem::invalid(
                "category_id: what it was; to drop your choice, revert it",
            ));
        }
        row.category_id = v;
    }
    if let Some(v) = req.purpose_id {
        if v.is_none() {
            return Err(Problem::invalid(
                "purpose_id: who it was for; to drop your choice, revert it",
            ));
        }
        row.purpose_id = v;
    }
    // Split or not, checked against the row as it is now.
    let was_split = !parts(&split_rows(&mut tx, id).await?, &row).is_empty();
    let parts = match req.splits {
        Some(Some(v)) if !v.is_empty() => Some(split_parts(&mut tx, id, &row, v).await?),
        Some(_) => Some(Vec::new()),
        None => None,
    };
    if parts.as_ref().map_or(was_split, |p| !p.is_empty()) {
        if status == RowStatus::TransferPair {
            return Err(Problem::invalid("a transfer pair can't be split"));
        }
        if decision_of(&row)? != Some(Decision::Add) {
            return Err(Problem::invalid(if parts.is_some() {
                "only a row that's added can be split"
            } else {
                "a split row is added: clear its splits first"
            }));
        }
    }
    // A conflicting pair added as one transfer: the other side you chose
    // gives way to the pair, on either row.
    let merge = status == RowStatus::TransferPair
        && req.decision == Some(Some(Decision::Add))
        && pair_key(&row).is_none();
    let other = partner(&row);
    if merge && let Some(k) = other {
        let mine = store::imports::statements(&mut *tx, &[id])
            .await?
            .into_iter()
            .find(|(_, s)| s.position == statement)
            .and_then(|(_, s)| s.effective_account())
            .ok_or_else(Problem::internal)?;
        let theirs = store::imports::matchable(&mut *tx)
            .await?
            .into_iter()
            .find(|r| key(r) == k)
            .ok_or_else(Problem::internal)?;
        let their_account = theirs.account_id.ok_or_else(Problem::internal)?;
        if row.transfer_account_id.is_some_and(|a| a != their_account) {
            row.transfer_account_id = Some(their_account);
            row.counterparty_id = None;
        }
        if theirs.chose_side && theirs.transfer_account_id.is_some_and(|a| a != mine) {
            store::imports::choose_transfer(&mut *tx, k, mine).await?;
        }
    }
    store::imports::set_decision(&mut *tx, id, &row)
        .await
        .map_err(|e| write_error(e, String::new))?;
    if let Some(p) = &parts {
        store::imports::set_splits(&mut tx, id, statement, position, p)
            .await
            .map_err(|e| write_error(e, String::new))?;
    }
    // A transfer pair is one choice: the other row takes it too.
    if req.decision.is_some()
        && status == RowStatus::TransferPair
        && let Some(k) = other
    {
        store::imports::set_pair_decision(&mut *tx, k, row.decision.as_deref()).await?;
        if k.0 != id {
            store::imports::touch(&mut *tx, k.0).await?;
        }
    }
    // A transfer chosen may make or undo a transfer pair.
    rematch(&mut tx).await?;
    if merge && let Some(k) = other {
        // Paired now, which starts the pair over at its default: added as
        // one transfer, as you chose.
        for k in [(id, statement, position), k] {
            store::imports::set_pair_decision(&mut *tx, k, Some(Decision::Add.as_str())).await?;
        }
    }
    store::imports::touch(&mut *tx, id).await?;
    let row = store::imports::rows(
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
    .ok_or_else(Problem::internal)?;
    let version = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::internal)?;
    let mut row = shown(&mut tx, id, vec![row])
        .await?
        .pop()
        .ok_or_else(Problem::internal)?;
    if let Some(tick) = req.reviewed {
        let seen = if tick { Some(row.tick()?) } else { None };
        store::imports::set_reviewed(&mut *tx, id, &[((statement, position), seen)]).await?;
        // Its colour follows the tick.
        let ticked = store::imports::rows(
            &mut *tx,
            id,
            Some(statement),
            None,
            Some((statement, after)),
            1,
        )
        .await?;
        row = shown(&mut tx, id, ticked)
            .await?
            .pop()
            .ok_or_else(Problem::internal)?;
    }
    tx.commit().await?;
    Ok((StatusCode::OK, etag(version), axum::Json(row)).into_response())
}

/// Tick rows as reviewed, or untick them (design §6.7): what a ticked row
/// takes counts as yours, so an import whose rows you ticked is ready to
/// commit. A row that needs you, or isn't added, isn't ticked. Each tick
/// goes by itself when its row changes under it.
#[utoipa::path(post, path = "/imports/{id}/review", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path),
           ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = Review,
    responses((status = OK, body = Import, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn review(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<Review>,
) -> Result<Response, Problem> {
    if req.rows.len() > MAX_REVIEW {
        return Err(Problem::invalid(format!("rows: at most {MAX_REVIEW}")));
    }
    let mut tx = state.pool.begin().await?;
    let current = store::imports::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current != version {
        return Err(Problem::stale());
    }
    let wanted: HashSet<(i16, i32)> = req.rows.iter().map(|r| (r.statement, r.position)).collect();
    let rows: Vec<RowRow> = store::imports::rows(&mut *tx, id, None, None, None, i64::MAX)
        .await?
        .into_iter()
        .filter(|r| wanted.contains(&(r.statement, r.position)))
        .collect();
    if rows.len() != wanted.len() {
        return Err(Problem::invalid("rows: not all of them are this import's"));
    }
    let mut ticks = Vec::new();
    for r in shown(&mut tx, id, rows).await? {
        let seen = match (req.reviewed, r.state) {
            (false, _) => None,
            (true, ReviewState::NeedsYou | ReviewState::NotAdded) => continue,
            (true, ReviewState::ToReview | ReviewState::Ready) => Some(r.tick()?),
        };
        ticks.push(((r.statement, r.position), seen));
    }
    store::imports::set_reviewed(&mut *tx, id, &ticks).await?;
    store::imports::touch(&mut *tx, id).await?;
    let after = one(&mut tx, id).await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// A row's parts as a request gives them, checked: two or more, each more
/// than zero, adding up to the row's amount, with one other side each (not
/// the statement's own account), and no category or purpose on a transfer
/// between two liquid or two non-liquid accounts.
async fn split_parts(
    tx: &mut PgConnection,
    id: Uuid,
    row: &RowRow,
    splits: Vec<RowSplit>,
) -> Result<Vec<StagedSplit>, Problem> {
    if splits.len() < 2 {
        return Err(Problem::invalid(
            "splits: two or more (unsplit, the row is one), or none",
        ));
    }
    if splits.len() > MAX_SPLITS {
        return Err(Problem::invalid(format!("splits: at most {MAX_SPLITS}")));
    }
    let account = store::imports::statements(&mut *tx, &[id])
        .await?
        .into_iter()
        .find(|(_, s)| s.position == row.statement)
        .and_then(|(_, s)| s.effective_account())
        .ok_or_else(|| Problem::invalid("splits: pick the statement's account first"))?;
    let accounts = suggestions::accounts(tx).await?;
    let mut out = Vec::new();
    for (i, p) in (1..).zip(splits) {
        let amount = p.amount.as_decimal();
        if amount <= Decimal::ZERO {
            return Err(Problem::invalid(format!(
                "split {i}: the amount must be more than zero"
            )));
        }
        if let Some(a) = p.transfer_account_id {
            if p.counterparty_id.is_some() {
                return Err(Problem::invalid(format!(
                    "split {i}: counterparty_id and transfer_account_id: one or the other"
                )));
            }
            if a == account {
                return Err(Problem::invalid(format!(
                    "split {i}: transfer_account_id is the statement's own account"
                )));
            }
            store::accounts::get(&mut *tx, a, false)
                .await?
                .ok_or_else(|| {
                    Problem::invalid(format!("split {i}: transfer_account_id: no such account"))
                })?;
            if (p.category_id.is_some() || p.purpose_id.is_some())
                && !suggest::labeled(
                    &accounts,
                    Some(account),
                    row.amount,
                    suggest::Value::Account(a),
                )
            {
                return Err(Problem::invalid(format!(
                    "split {i}: a transfer has no category or purpose"
                )));
            }
        }
        let description = p.description.map(|d| d.trim().to_owned());
        if description
            .as_ref()
            .is_some_and(|d| d.chars().count() > 500)
        {
            return Err(Problem::invalid(format!(
                "split {i}: description is at most 500 characters"
            )));
        }
        out.push(StagedSplit {
            statement: row.statement,
            position: row.position,
            amount,
            counterparty_id: p.counterparty_id,
            transfer_account_id: p.transfer_account_id,
            category_id: p.category_id,
            purpose_id: p.purpose_id,
            description: description.filter(|d| !d.is_empty()),
        });
    }
    let total: Decimal = out.iter().map(|p| p.amount).sum();
    if total != row.amount.abs() {
        return Err(Problem::invalid(format!(
            "splits add up to {total}, the row is {}",
            row.amount.abs()
        )));
    }
    // Income or an expense with a liquid account says who it was for and
    // what it was.
    let liquid = |id: Uuid| {
        accounts
            .iter()
            .find(|a| a.id == id)
            .is_none_or(|a| a.liquid)
    };
    for (i, p) in (1..).zip(&out) {
        let takes_purpose = match p.transfer_account_id {
            Some(a) => liquid(a) != liquid(account),
            None => liquid(account),
        };
        if takes_purpose && p.purpose_id.is_none() {
            return Err(Problem::invalid(format!(
                "split {i}: purpose_id: who it was for"
            )));
        }
        if takes_purpose && p.category_id.is_none() {
            return Err(Problem::invalid(format!(
                "split {i}: category_id: what it was"
            )));
        }
    }
    Ok(out)
}

/// Commit the import: all or nothing. Every row's label is worked out again
/// first; if any changed (the ledger moved on since), nothing is committed
/// and the import comes back with a 409 to review again. Then each row's
/// choice is carried out: new transactions go into one `file` import
/// batch, a transfer pair added as one becomes one transfer, and every row
/// that's now in the ledger is recorded as imported, so the same row is a
/// duplicate next time, and what this file says about it is kept with its
/// transaction (once per file, whatever its name). Each statement's ledger
/// balance becomes a balance check. The staged import is then gone. Rows to
/// review don't stop it; rows that need you do.
#[utoipa::path(post, path = "/imports/{id}/commit", tag = "imports", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = OK, body = Committed),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "A statement has no account, a row needs a label, a row's label changed, or a month is closed", body = Problem, content_type = "application/problem+json"),
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
    let committed = carry_out(&mut tx, &state, principal, id).await?;
    tx.commit().await?;
    committed.map(axum::Json).ok_or_else(|| {
        Problem::conflict("the ledger changed since the rows were checked: review them again")
    })
}

/// Commits a locked import ([`commit`]); `None`, with nothing committed but
/// its rows worked out again, if any of them changed.
async fn carry_out(
    tx: &mut PgConnection,
    state: &AppState,
    principal: Principal,
    id: Uuid,
) -> Result<Option<Committed>, Problem> {
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

    // The ledger, or other staged imports, may have moved on since the rows
    // were checked.
    let seen = store::imports::rows(&mut *tx, id, None, None, None, i64::MAX).await?;
    for s in &statements {
        restatus(tx, id, s.position).await?;
    }
    rematch(tx).await?;
    let rows = store::imports::rows(&mut *tx, id, None, None, None, i64::MAX).await?;
    let changed = rows.iter().zip(&seen).any(|(now, before)| {
        now.status != before.status
            || now.match_transaction_id != before.match_transaction_id
            || pair_key(now) != pair_key(before)
            || conflict_key(now) != conflict_key(before)
    });
    if changed {
        store::imports::touch(&mut *tx, id).await?;
        return Ok(None);
    }

    // Every row that's added needs each label from you or a suggestion
    // (design §8): exactly the rows that need you.
    let shown = shown(tx, id, rows.clone()).await?;
    let lacking: Vec<String> = shown
        .iter()
        .filter(|r| r.state == ReviewState::NeedsYou)
        .map(|r| {
            let names: Vec<&str> = r.missing.iter().map(|l| l.as_str()).collect();
            format!(
                "statement {} row {} ({})",
                r.statement,
                r.position,
                names.join(", ")
            )
        })
        .collect();
    if !lacking.is_empty() {
        let n = lacking.len();
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "labels-missing",
            "Rows need labels",
        )
        .detail(format!(
            "{n} rows that are added have labels with neither your choice nor a suggestion: {}{}",
            lacking[..n.min(10)].join("; "),
            if n > 10 { "; …" } else { "" }
        )));
    }
    let keys: Vec<(i16, i32)> = rows.iter().map(|r| (r.statement, r.position)).collect();
    let suggestions = store::imports::suggestions(&mut *tx, id, &keys).await?;
    let accounts = suggestions::accounts(tx).await?;
    let pairs: HashMap<RowKey, store::imports::PairRow> = {
        let keys: Vec<RowKey> = rows.iter().filter_map(partner).collect();
        store::imports::pair_rows(&mut *tx, &keys)
            .await?
            .into_iter()
            .map(|p| ((p.import_id, p.statement, p.position), p))
            .collect()
    };
    let splits = split_rows(tx, id).await?;
    let context = |r: &RowRow, decision: Decision| -> Result<Context, Problem> {
        let s = statements
            .iter()
            .find(|s| s.position == r.statement)
            .ok_or_else(Problem::internal)?;
        Ok(Context {
            accounts: &accounts,
            account: s.effective_account(),
            pair_account: pair_key(r)
                .filter(|_| decision == Decision::Add)
                .and_then(|k| pairs.get(&k))
                .and_then(|p| p.account_id),
            splits: parts(&splits, r),
        })
    };
    let matcher = matcher_id(tx).await?;
    let mut outcomes = Vec::new();
    for (r, s) in rows.iter().zip(&shown) {
        let (Some(g), Some(committed)) = (&s.suggested_decision, s.decision) else {
            continue;
        };
        outcomes.push(Outcome {
            suggester_id: matcher,
            label: "decision".to_owned(),
            suggested: None,
            confidence: g.confidence,
            committed: None,
            transaction_id: None,
            decisions: Some((
                g.decision.as_str().to_owned(),
                committed.as_str().to_owned(),
            )),
        });
        debug_assert_eq!((r.statement, r.position), (s.statement, s.position));
    }

    let mut out = Committed {
        import_batch_id: None,
        added: 0,
        transfers: 0,
        not_added: 0,
        balance_checks: Vec::new(),
    };
    let mut imported = Vec::new();
    let mut seen_in_file = Vec::new();
    // Transfers made of a transfer pair with a row of this import, by that
    // row: it's recorded already.
    let mut paired: HashMap<RowKey, Uuid> = HashMap::new();
    let mut batch_id = None;
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
        let mine = parts(&splits, r);
        if !mine.is_empty() && decision != Decision::Add {
            return Err(at_row(r)(Problem::conflict(format!(
                "a split row is added, not {}: clear its splits first",
                decision.as_str()
            ))));
        }
        let mut batch = async |tx: &mut PgConnection| -> Result<Uuid, Problem> {
            if let Some(b) = batch_id {
                return Ok(b);
            }
            let b = Uuid::now_v7();
            let token = principal.actor().map(|t| *t.as_uuid());
            let label = import.file_name.as_deref().unwrap_or("Imported file");
            store::import_batches::insert(&mut *tx, b, "file", Some(label), token).await?;
            batch_id = Some(b);
            Ok(b)
        };
        let description = r
            .set_description
            .clone()
            .unwrap_or_else(|| r.description.chars().take(500).collect());
        // A row added again after it changed (or added anyway) needs an ID
        // of its own.
        let fingerprint = if decision.adds() && status.matched() {
            store::imports::free_fingerprint(&mut *tx, account, &r.fingerprint).await?
        } else {
            r.fingerprint.clone()
        };
        let became = match (decision, status) {
            (Decision::DontAdd, s) if s.matched() => {
                out.not_added += 1;
                r.match_transaction_id.ok_or_else(Problem::internal)?
            }
            (Decision::DontAdd, _) => {
                out.not_added += 1;
                continue;
            }
            (Decision::Add, RowStatus::TransferPair) => {
                if let Some(t) = paired.get(&(id, r.statement, r.position)) {
                    *t
                } else {
                    let p = pair_key(r)
                        .and_then(|k| pairs.get(&k))
                        .ok_or_else(|| {
                            Problem::conflict("the transfer pair changed: review it again")
                        })
                        .map_err(at_row(r))?;
                    let other = p.account_id.ok_or_else(Problem::internal)?;
                    // One transfer, dated and described as the side the
                    // money left.
                    let (from, to, date, description) = if r.amount.is_sign_negative() {
                        (account, other, r.date, description)
                    } else {
                        let theirs = p
                            .set_description
                            .clone()
                            .unwrap_or_else(|| p.description.chars().take(500).collect());
                        (other, account, p.date, theirs)
                    };
                    let l = labels(r, &suggestions, &context(r, decision)?);
                    let quantity = Amount::from_decimal(r.amount.abs());
                    let mut tags = r.tags.clone();
                    tags.extend(p.tags.iter().cloned());
                    let req = NewTransaction::single(
                        date,
                        description,
                        Leg::new(Some(from), None, s.commodity_id, quantity),
                        Leg::new(Some(to), None, s.commodity_id, quantity),
                        l.purpose,
                        l.category,
                    )
                    .with_tags(tags);
                    let b = batch(tx).await?;
                    let t = create_in(tx, principal, req, b)
                        .await
                        .map_err(at_row(r))?
                        .id();
                    out.added += 1;
                    out.transfers += 1;
                    // The other side's bank row is recorded with it, so it's a
                    // duplicate of this transfer from now on.
                    imported.push(Fingerprint {
                        account_id: other,
                        fingerprint: p.fingerprint.clone(),
                        transaction_id: t,
                    });
                    seen_in_file.push(ImportedRow {
                        account_id: other,
                        fingerprint: p.fingerprint.clone(),
                        file_sha256: p.file_sha256.clone(),
                        raw: p.raw.clone(),
                    });
                    if p.import_id == id {
                        paired.insert((p.import_id, p.statement, p.position), t);
                    }
                    t
                }
            }
            _ if !mine.is_empty() => {
                let parts = mine
                    .iter()
                    .map(|p| {
                        let quantity = Amount::from_decimal(p.amount);
                        let mine = Leg::new(Some(account), None, s.commodity_id, quantity);
                        let outside = Leg::new(
                            p.transfer_account_id,
                            p.counterparty_id,
                            s.commodity_id,
                            quantity,
                        );
                        let (from, to) = if r.amount.is_sign_negative() {
                            (mine, outside)
                        } else {
                            (outside, mine)
                        };
                        NewSplit::new(from, to, p.purpose_id, p.category_id, p.description.clone())
                    })
                    .collect();
                let req = NewTransaction::of(r.date, description, parts).with_tags(r.tags.clone());
                let b = batch(tx).await?;
                let t = create_in(tx, principal, req, b).await.map_err(at_row(r))?;
                out.added += 1;
                // The row's suggestions didn't apply: no outcomes for them.
                t.id()
            }
            _ => {
                let l = labels(r, &suggestions, &context(r, decision)?);
                let quantity = Amount::from_decimal(r.amount.abs());
                let mine = Leg::new(Some(account), None, s.commodity_id, quantity);
                let outside = Leg::new(l.transfer, l.counterparty, s.commodity_id, quantity);
                let (from, to) = if r.amount.is_sign_negative() {
                    (mine, outside)
                } else {
                    (outside, mine)
                };
                let req =
                    NewTransaction::single(r.date, description, from, to, l.purpose, l.category)
                        .with_tags(r.tags.clone());
                let b = batch(tx).await?;
                let t = create_in(tx, principal, req, b).await.map_err(at_row(r))?;
                out.added += 1;
                for sg in suggestions
                    .iter()
                    .filter(|sg| (sg.statement, sg.position) == (r.statement, r.position))
                {
                    let committed = match Label::parse(&sg.label)? {
                        Label::Counterparty => l.transfer.or(l.counterparty),
                        Label::Category => l.category,
                        Label::Purpose => l.purpose,
                    };
                    outcomes.push(Outcome {
                        suggester_id: sg.suggester_id,
                        label: sg.label.clone(),
                        suggested: sg.account_id.or(sg.value),
                        confidence: sg.confidence,
                        committed,
                        transaction_id: Some(t.id()),
                        decisions: None,
                    });
                }
                t.id()
            }
        };
        imported.push(Fingerprint {
            account_id: account,
            fingerprint: fingerprint.clone(),
            transaction_id: became,
        });
        seen_in_file.push(ImportedRow {
            account_id: account,
            fingerprint,
            file_sha256: import.file_sha256.clone(),
            raw: r.raw.clone(),
        });
    }
    out.import_batch_id = batch_id;
    if !imported.is_empty() {
        // A row already recorded (a duplicate) keeps its transaction; what
        // this file says about it is added either way.
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
                decisions: None,
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
                    tx,
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
    // The ledger changed: other staged imports' rows may now be duplicates,
    // and their transfer pairs with this one's rows are gone.
    let levels = state.suggestions.levels(tx).await?;
    refresh_others(tx, &levels, id).await?;
    audit(
        tx,
        principal,
        Action::Delete,
        "staged_import",
        id,
        Some(&json!({ "file_name": import.file_name, "committed": true })),
        Some(
            &json!({ "import_batch_id": out.import_batch_id, "added": out.added,
                      "transfers": out.transfers, "not_added": out.not_added }),
        ),
    )
    .await?;
    Ok(Some(out))
}

/// Commit the imports you saw as ready, oldest first, each on its own, as
/// [`commit`] does. Each is checked again just before: one that's gone, no
/// longer ready (committing another may have changed it), or changed since
/// is skipped, as is one commit refuses; the others go ahead.
#[utoipa::path(post, path = "/imports/commit-ready", tag = "imports", security(("bearer" = [])),
    request_body = CommitReady,
    responses((status = OK, body = ReadyCommitted)))]
async fn commit_ready(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<CommitReady>,
) -> Result<axum::Json<ReadyCommitted>, Problem> {
    let mut imports = Vec::new();
    {
        let mut db = state.pool.acquire().await?;
        for id in &body.ids {
            if let Some(i) = store::imports::get(&mut *db, *id).await? {
                imports.push(i);
            }
        }
    }
    imports.sort_by_key(|i| (i.created_at, i.id));
    imports.dedup_by_key(|i| i.id);
    let mut results: Vec<ReadyOutcome> = body
        .ids
        .iter()
        .filter(|id| !imports.iter().any(|i| i.id == **id))
        .map(|id| ReadyOutcome {
            id: *id,
            file_name: None,
            committed: None,
            skipped: Some("it's gone".to_owned()),
        })
        .collect();
    for i in imports {
        let outcome = |committed: Option<Committed>, skipped: Option<String>| ReadyOutcome {
            id: i.id,
            file_name: i.file_name.clone(),
            committed,
            skipped,
        };
        let mut tx = state.pool.begin().await?;
        if store::imports::lock(&mut *tx, i.id).await?.is_none() {
            results.push(outcome(None, Some("it's gone".to_owned())));
            continue;
        }
        if one(&mut tx, i.id).await?.readiness.state != ReviewState::Ready {
            results.push(outcome(None, Some("it isn't ready".to_owned())));
            continue;
        }
        match carry_out(&mut tx, &state, principal, i.id).await {
            Ok(Some(c)) => {
                tx.commit().await?;
                results.push(outcome(Some(c), None));
            }
            Ok(None) => {
                tx.commit().await?;
                results.push(outcome(
                    None,
                    Some("the ledger changed since its rows were checked".to_owned()),
                ));
            }
            Err(p) => {
                drop(tx);
                results.push(outcome(None, Some(p.detail.unwrap_or(p.title))));
            }
        }
    }
    Ok(axum::Json(ReadyCommitted { results }))
}
