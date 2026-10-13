//! `/api/v1/rules` (design §8, docs/ai.md): the household's rules script.
//! Every upload is a new version, validated first; the newest is active.
//! Versions are never edited or deleted, so they are their own history (who
//! uploaded each, when, and why).

use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::rules::RuleScriptRow;
use suggest::{Facts, Label, Row};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::conventions::{IfMatch, Page, PageParams, etag};
use crate::imports::Label as LabelName;
use crate::problem::{Json, Path, Problem, ProblemItem, Query};
use crate::suggestions::{history, names};

/// Rows of a dry run listed at most.
const MAX_ITEMS: usize = 500;
/// Longest dry run, in days.
const MAX_DAYS: i64 = 3 * 366;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get, put))
        .routes(routes!(versions))
        .routes(routes!(version))
        .routes(routes!(dry_run))
}

// ------------------------------------------------------------- Schemas ----

/// A version of the rules script.
#[derive(Debug, Serialize, ToSchema)]
pub struct RuleScript {
    /// 0 when there's none yet: no rules.
    version: i32,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    /// The name of the token that uploaded it.
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<OffsetDateTime>,
}

impl From<RuleScriptRow> for RuleScript {
    fn from(r: RuleScriptRow) -> Self {
        Self {
            version: r.version,
            source: r.source,
            note: r.note,
            author: r.author,
            created_at: Some(r.created_at),
        }
    }
}

/// A version, without its source.
#[derive(Debug, Serialize, ToSchema)]
pub struct RuleVersion {
    version: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    created_at: OffsetDateTime,
    lines: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleUpload {
    /// The whole script; empty for no rules.
    source: String,
    /// Why, like a commit message (at most 500 characters).
    note: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DryRunParams {
    /// First day.
    from: Date,
    /// Last day (at most about three years after `from`).
    to: Date,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DryRunRequest {
    /// A candidate script; the active one if omitted (`{}`).
    source: Option<String>,
}

/// A label the script would set differently from the ledger.
#[derive(Debug, Serialize, ToSchema)]
pub struct DryRunChange {
    label: LabelName,
    /// What the ledger has; `null` for none (or a transfer).
    current: Option<Uuid>,
    /// For the counterparty: the ledger has a transfer with this account.
    #[serde(skip_serializing_if = "Option::is_none")]
    current_transfer_account_id: Option<Uuid>,
    /// What the script sets; `null` for none (or a transfer).
    proposed: Option<Uuid>,
    /// For the counterparty: the script sets a transfer with this account.
    #[serde(skip_serializing_if = "Option::is_none")]
    proposed_transfer_account_id: Option<Uuid>,
    /// The line that sets it.
    line: u32,
}

/// A transaction the script would label differently, or fails on.
#[derive(Debug, Serialize, ToSchema)]
pub struct DryRunRow {
    transaction_id: Uuid,
    date: Date,
    description: String,
    account_id: Uuid,
    changes: Vec<DryRunChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DryRun {
    /// The script's version; absent for a candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<i32>,
    /// Transactions it ran on: those between one of your accounts and the
    /// outside, and transfers between two of them (once from each side),
    /// dated in the range.
    rows: u32,
    /// Of them, those it set any label for.
    labeled: u32,
    /// Those it would label differently from the ledger.
    changed: u32,
    /// Those it failed on.
    failed: u32,
    /// Changed and failed rows, by date (at most 500).
    items: Vec<DryRunRow>,
    truncated: bool,
}

// ------------------------------------------------------------- Helpers ----

fn invalid(errors: Vec<rules::ScriptError>) -> Problem {
    let n = errors.len();
    Problem::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "rules-invalid",
        "The script is invalid",
    )
    .detail(format!(
        "{n} error{}: {}",
        if n == 1 { "" } else { "s" },
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    ))
    .errors(
        errors
            .into_iter()
            .map(|e| ProblemItem {
                line: e.line.and_then(|l| u32::try_from(l).ok()),
                column: e.column.and_then(|c| u32::try_from(c).ok()),
                message: e.message,
            })
            .collect(),
    )
}

fn none() -> RuleScript {
    RuleScript {
        version: 0,
        source: String::new(),
        note: None,
        author: None,
        created_at: None,
    }
}

fn respond(status: StatusCode, r: &RuleScript) -> Response {
    (status, etag(i64::from(r.version)), axum::Json(r)).into_response()
}

// ------------------------------------------------------------ Handlers ----

/// The active rules script (the newest version); version 0, empty, when
/// there's none.
#[utoipa::path(get, path = "/rules", tag = "rules", security(("bearer" = [])),
    responses((status = OK, body = RuleScript, headers(("ETag" = String)))))]
async fn get(State(state): State<AppState>) -> Result<Response, Problem> {
    let mut db = state.pool.acquire().await?;
    let r = store::rules::active(&mut *db)
        .await?
        .map_or_else(none, RuleScript::from);
    Ok(respond(StatusCode::OK, &r))
}

/// Upload a new version of the rules script. It's compiled and every name
/// it uses is checked against the ledger; an invalid script is refused with
/// its errors, by line (`rules-invalid`), and never becomes active. A valid
/// one is active at once, for the next file uploaded.
#[utoipa::path(put, path = "/rules", tag = "rules", security(("bearer" = [])),
    params(("If-Match" = String, Header, description = "The active version's ETag")),
    request_body = RuleUpload,
    responses((status = CREATED, body = RuleScript, headers(("ETag" = String))),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, description = "Invalid, with `errors` by line", body = Problem, content_type = "application/problem+json")))]
async fn put(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    IfMatch(expected): IfMatch,
    Json(req): Json<RuleUpload>,
) -> Result<Response, Problem> {
    let note = req.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if note.is_some_and(|n| n.chars().count() > 500) {
        return Err(Problem::invalid("note must be at most 500 characters"));
    }
    let mut tx = state.pool.begin().await?;
    store::rules::lock(&mut *tx).await?;
    let current = store::rules::active(&mut *tx)
        .await?
        .map_or(0, |r| r.version);
    if i64::from(current) != expected {
        return Err(Problem::stale());
    }
    let version = current + 1;
    rules::Script::compile(version, &req.source, &names(&mut tx).await?).map_err(invalid)?;
    let token = principal.actor().map(|t| *t.as_uuid());
    store::rules::insert(&mut *tx, version, &req.source, note, token).await?;
    let r = store::rules::get(&mut *tx, version)
        .await?
        .ok_or_else(Problem::internal)?;
    tx.commit().await?;
    Ok(respond(StatusCode::CREATED, &r.into()))
}

/// Every version of the rules script, newest first.
#[utoipa::path(get, path = "/rules/versions", tag = "rules", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<RuleVersion>)))]
async fn versions(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<RuleVersion>>, Problem> {
    let before = page
        .cursor
        .as_deref()
        .map(|c| {
            c.parse::<i32>()
                .map_err(|_| Problem::invalid("invalid cursor"))
        })
        .transpose()?;
    let mut db = state.pool.acquire().await?;
    let rows = store::rules::list(&mut *db, before, page.limit() + 1).await?;
    let items = rows
        .into_iter()
        .map(|r| RuleVersion {
            version: r.version,
            lines: u32::try_from(r.source.lines().count()).unwrap_or(u32::MAX),
            note: r.note,
            author: r.author,
            created_at: r.created_at,
        })
        .collect();
    Ok(axum::Json(Page::from_rows(items, page.limit(), |v| {
        v.version.to_string()
    })))
}

/// One version of the rules script.
#[utoipa::path(get, path = "/rules/versions/{version}", tag = "rules", security(("bearer" = [])),
    params(("version" = i32, Path)),
    responses((status = OK, body = RuleScript),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn version(
    State(state): State<AppState>,
    Path(version): Path<i32>,
) -> Result<axum::Json<RuleScript>, Problem> {
    let mut db = state.pool.acquire().await?;
    let r = store::rules::get(&mut *db, version)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(axum::Json(r.into()))
}

/// What a script would set for the ledger's transactions in a date range
/// (those between one of your accounts and the outside, as an imported row
/// becomes), compared with what they have. Runs the active script, or a
/// candidate given in the body (validated first); stores nothing. The
/// bank's own fields (`row.raw`) are empty here, and `fn account` isn't run.
#[utoipa::path(post, path = "/rules/dry-run", tag = "rules", security(("bearer" = [])),
    params(DryRunParams),
    request_body = DryRunRequest,
    responses((status = OK, body = DryRun),
              (status = UNPROCESSABLE_ENTITY, description = "An invalid range or candidate", body = Problem, content_type = "application/problem+json")))]
async fn dry_run(
    State(state): State<AppState>,
    Query(p): Query<DryRunParams>,
    Json(req): Json<DryRunRequest>,
) -> Result<axum::Json<DryRun>, Problem> {
    if p.to < p.from || (p.to - p.from).whole_days() > MAX_DAYS {
        return Err(Problem::invalid(format!(
            "from..to must be in order and at most {MAX_DAYS} days"
        )));
    }
    let mut db = state.pool.acquire().await?;
    let (version, source) = match req.source {
        Some(s) => (None, s),
        None => match store::rules::active(&mut *db).await? {
            Some(r) => (Some(r.version), r.source),
            None => (Some(0), String::new()),
        },
    };
    let names = names(&mut db).await?;
    let script = rules::Script::compile(version.unwrap_or(0), &source, &names).map_err(invalid)?;
    let history = history(&mut db).await?;
    let examples: Vec<(suggest::Example, Vec<suggest::Candidate>)> = history
        .examples
        .iter()
        .filter(|e| e.date >= p.from && e.date <= p.to)
        .map(|e| (e.clone(), history.transfer_candidates(e)))
        .collect();
    drop(db);
    let out = tokio::task::spawn_blocking(move || {
        let mut out = DryRun {
            version,
            rows: 0,
            labeled: 0,
            changed: 0,
            failed: 0,
            items: Vec::new(),
            truncated: false,
        };
        for (e, transfer_candidates) in examples {
            out.rows += 1;
            let row = Row {
                account: Some(e.account),
                date: e.date,
                amount: e.amount,
                description: e.description.clone(),
                raw: serde_json::Value::Null,
                facts: Facts::default(),
                transfer_candidates,
            };
            let mut item = DryRunRow {
                transaction_id: e.transaction,
                date: e.date,
                description: e.description,
                account_id: e.account,
                changes: Vec::new(),
                error: None,
            };
            match script.classify(&row) {
                Ok(set) => {
                    if Label::ALL.into_iter().any(|l| set.get(l).is_some()) {
                        out.labeled += 1;
                    }
                    for l in Label::ALL {
                        if let Some((value, line)) = set.get(l)
                            && value != e.labels.get(l)
                        {
                            let current = e.labels.get(l);
                            item.changes.push(DryRunChange {
                                label: l.into(),
                                current: current.id(),
                                current_transfer_account_id: current.account(),
                                proposed: value.id(),
                                proposed_transfer_account_id: value.account(),
                                line: u32::try_from(line).unwrap_or(0),
                            });
                        }
                    }
                    if item.changes.is_empty() {
                        continue;
                    }
                    out.changed += 1;
                }
                Err(err) => {
                    out.failed += 1;
                    item.error = Some(err);
                }
            }
            if out.items.len() < MAX_ITEMS {
                out.items.push(item);
            } else {
                out.truncated = true;
            }
        }
        out
    })
    .await
    .map_err(|_| Problem::internal())?;
    Ok(axum::Json(out))
}
