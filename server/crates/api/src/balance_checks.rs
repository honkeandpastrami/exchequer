//! `/api/v1/balance-checks` (design §7.1): the balance the bank shows for
//! an account on a date, compared with the ledger. A match marks the account
//! verified through that date; a mismatch comes with candidate explanations
//! from the deterministic discrepancy finder, searched since the last
//! verified date.
//!
//! Amounts are as the bank shows them: for credit cards and mortgages the
//! amount owed is positive. They're stored in the ledger's sign.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use domain::discrepancy::{self, Entry, Reason};
use domain::settlement::SplitRef;
use domain::{Amount, TransactionId};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use store::audit::Action;
use store::balance_checks::CheckRow;
use time::{Date, Duration, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::audit;
use crate::conventions::{IfMatch, Page, etag};
use crate::problem::{Json, Path, Problem, Query};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, delete))
}

/// Checks never change; their ETag is fixed.
const VERSION: i64 = 1;

// ------------------------------------------------------------- Schemas ----

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewBalanceCheck {
    account_id: Uuid,
    /// Default: the household currency.
    commodity_id: Option<Uuid>,
    /// Default: today (UTC).
    date: Option<Date>,
    /// As the bank shows it (owed amounts positive on cards and mortgages).
    #[schema(value_type = String, example = "1234.56")]
    amount: Amount,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CandidateReason {
    /// Two postings of the same amount a few days apart, one equal to the
    /// difference.
    DuplicatePair,
    /// One posting equal to the difference: recorded twice, or not in the
    /// bank yet.
    EqualsDifference,
    /// Dated around the check date; the bank may count it on the other side.
    DateBoundary,
    /// Recorded with the wrong sign (the difference is twice its amount).
    SignFlipped,
    /// Two or three postings that together equal the difference.
    Subset,
    /// Two adjacent digits swapped in one posting.
    Transposition,
    /// Two postings of the same amount a few days apart.
    NearDuplicate,
    /// The difference is a multiple of 9 in the smallest unit, typical of a
    /// transposition typo.
    MultipleOfNine,
}

impl From<Reason> for CandidateReason {
    fn from(r: Reason) -> Self {
        match r {
            Reason::DuplicatePair => Self::DuplicatePair,
            Reason::EqualsDifference => Self::EqualsDifference,
            Reason::DateBoundary => Self::DateBoundary,
            Reason::SignFlipped => Self::SignFlipped,
            Reason::Subset => Self::Subset,
            Reason::Transposition => Self::Transposition,
            Reason::NearDuplicate => Self::NearDuplicate,
            Reason::MultipleOfNine => Self::MultipleOfNine,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CandidateSplit {
    transaction_id: Uuid,
    position: usize,
}

/// A possible explanation for the difference, most likely first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Candidate {
    reason: CandidateReason,
    splits: Vec<CandidateSplit>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BalanceCheck {
    id: Uuid,
    account_id: Uuid,
    commodity_id: Uuid,
    date: Date,
    /// What the bank showed.
    #[schema(value_type = String)]
    amount: Amount,
    /// `user` or `ofx`.
    source: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    /// The ledger's balance at the end of `date`, signed like `amount`.
    #[schema(value_type = String)]
    ledger: Amount,
    /// `ledger − amount`.
    #[schema(value_type = String)]
    difference: Amount,
    matches: bool,
    /// The latest date up to this check's on which a check of the same
    /// account and commodity matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    verified_through: Option<Date>,
    /// For a mismatch: candidates were searched after this date (the last
    /// verified date before the check), or from the beginning if absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    searched_after: Option<Date>,
    /// For a mismatch, on a single check only.
    #[serde(skip_serializing_if = "Option::is_none")]
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    account_id: Option<Uuid>,
    commodity_id: Option<Uuid>,
    /// Items per page (default 100, max 500).
    limit: Option<u32>,
    /// `next_cursor` from the previous page. Opaque.
    cursor: Option<String>,
}

// ------------------------------------------------------------- Helpers ----

/// Banks show what's owed on these as positive.
fn owed_positive(kind: &str) -> bool {
    matches!(kind, "credit_card" | "mortgage")
}

fn shown(a: Amount, owed: bool) -> Amount {
    if owed { -a } else { a }
}

fn scaled(a: Amount, scale: u32) -> Amount {
    let mut d = a.as_decimal();
    if d.is_zero() {
        d = rust_decimal::Decimal::ZERO;
    }
    d.rescale(scale);
    Amount::from_decimal(d)
}

async fn account_kind(db: &mut PgConnection, id: Uuid) -> Result<String, Problem> {
    Ok(store::accounts::get(&mut *db, id, false)
        .await?
        .ok_or_else(Problem::internal)?
        .kind)
}

async fn scale_of(db: &mut PgConnection, commodity: Uuid) -> Result<u32, Problem> {
    let c = store::commodities::get(&mut *db, commodity, false)
        .await?
        .ok_or_else(Problem::internal)?;
    u32::try_from(c.precision).map_err(|_| Problem::internal())
}

/// Every check of the account and commodity, oldest first, with whether it
/// matches.
async fn history(db: &mut PgConnection, row: &CheckRow) -> Result<Vec<(Date, bool)>, Problem> {
    let mut all: Vec<(Date, bool)> = store::balance_checks::list(
        &mut *db,
        None,
        Some(row.account_id),
        Some(row.commodity_id),
        None,
        i64::MAX,
    )
    .await?
    .into_iter()
    .map(|c| (c.date, c.ledger == c.balance))
    .collect();
    all.reverse();
    Ok(all)
}

/// The API form of a stored check, with candidates if asked for.
async fn output(
    db: &mut PgConnection,
    row: CheckRow,
    with_candidates: bool,
) -> Result<BalanceCheck, Problem> {
    let owed = owed_positive(&account_kind(db, row.account_id).await?);
    let scale = scale_of(db, row.commodity_id).await?;
    let matches = row.ledger == row.balance;
    let checks = history(db, &row).await?;
    let verified_through = checks
        .iter()
        .filter(|(d, ok)| *ok && *d <= row.date)
        .map(|(d, _)| *d)
        .max();
    let searched_after = checks
        .iter()
        .filter(|(d, ok)| *ok && *d < row.date)
        .map(|(d, _)| *d)
        .max();
    let difference = Amount::from_decimal(row.ledger)
        .checked_sub(Amount::from_decimal(row.balance))
        .map_err(|_| Problem::internal())?;

    let candidates = if with_candidates && !matches {
        let until = row.date + Duration::days(discrepancy::NEAR_DAYS);
        let entries = store::balance_checks::postings(
            &mut *db,
            row.account_id,
            row.commodity_id,
            searched_after,
            until,
        )
        .await?
        .into_iter()
        .map(|(t, p, date, q)| {
            Ok(Entry {
                source: SplitRef {
                    transaction: TransactionId::from_uuid(t),
                    position: usize::try_from(p).map_err(|_| Problem::internal())?,
                },
                date,
                quantity: Amount::from_decimal(q),
            })
        })
        .collect::<Result<Vec<_>, Problem>>()?;
        Some(
            discrepancy::candidates(&entries, row.date, difference, scale)
                .into_iter()
                .map(|c| Candidate {
                    reason: c.reason.into(),
                    splits: c
                        .sources
                        .into_iter()
                        .map(|s| CandidateSplit {
                            transaction_id: *s.transaction.as_uuid(),
                            position: s.position,
                        })
                        .collect(),
                })
                .collect(),
        )
    } else {
        None
    };

    Ok(BalanceCheck {
        id: row.id,
        account_id: row.account_id,
        commodity_id: row.commodity_id,
        date: row.date,
        amount: scaled(shown(Amount::from_decimal(row.balance), owed), scale),
        source: row.source,
        created_at: row.created_at,
        ledger: scaled(shown(Amount::from_decimal(row.ledger), owed), scale),
        difference: scaled(shown(difference, owed), scale),
        matches,
        verified_through,
        searched_after: (!matches).then_some(searched_after).flatten(),
        candidates,
    })
}

async fn one(db: &mut PgConnection, id: Uuid) -> Result<CheckRow, Problem> {
    store::balance_checks::list(&mut *db, Some(id), None, None, None, 1)
        .await?
        .pop()
        .ok_or_else(Problem::not_found)
}

fn parse_cursor(c: &str) -> Result<(Date, Uuid), Problem> {
    let bad = || Problem::invalid("invalid cursor");
    let (d, id) = c.split_once('_').ok_or_else(bad)?;
    let format = time::macros::format_description!("[year]-[month]-[day]");
    Ok((
        Date::parse(d, &format).map_err(|_| bad())?,
        Uuid::try_parse(id).map_err(|_| bad())?,
    ))
}

// ------------------------------------------------------------ Handlers ----

/// List balance checks, newest first, each compared with the current ledger.
#[utoipa::path(get, path = "/balance-checks", tag = "balance-checks", security(("bearer" = [])),
    params(ListParams),
    responses((status = OK, body = Page<BalanceCheck>)))]
async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<axum::Json<Page<BalanceCheck>>, Problem> {
    let after = p.cursor.as_deref().map(parse_cursor).transpose()?;
    let limit = i64::from(p.limit.unwrap_or(100).clamp(1, 500));
    let mut conn = state.pool.acquire().await?;
    let rows = store::balance_checks::list(
        &mut *conn,
        None,
        p.account_id,
        p.commodity_id,
        after,
        limit + 1,
    )
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for r in rows {
        items.push(output(&mut conn, r, false).await?);
    }
    Ok(axum::Json(Page::from_rows(items, limit, |c| {
        format!("{}_{}", c.date, c.id)
    })))
}

/// One balance check, with candidate explanations if it doesn't match.
#[utoipa::path(get, path = "/balance-checks/{id}", tag = "balance-checks", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = BalanceCheck, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let mut conn = state.pool.acquire().await?;
    let row = one(&mut conn, id).await?;
    let check = output(&mut conn, row, true).await?;
    Ok((StatusCode::OK, etag(VERSION), axum::Json(check)).into_response())
}

/// Record what the bank shows and compare it with the ledger. A mismatch
/// comes with candidate explanations.
#[utoipa::path(post, path = "/balance-checks", tag = "balance-checks", security(("bearer" = [])),
    request_body = NewBalanceCheck,
    responses((status = CREATED, body = BalanceCheck, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, description = "No commodity given and no household currency", body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewBalanceCheck>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let account = store::accounts::get(&mut *tx, req.account_id, false)
        .await?
        .ok_or_else(|| Problem::invalid(format!("unknown account {}", req.account_id)))?;
    let commodity = match req.commodity_id {
        Some(c) => c,
        None => store::parties::household(&mut *tx)
            .await?
            .and_then(|h| h.currency_id)
            .ok_or_else(|| {
                Problem::conflict("give commodity_id, or set the household currency first")
            })?,
    };
    let precision = store::commodities::get(&mut *tx, commodity, false)
        .await?
        .ok_or_else(|| Problem::invalid(format!("unknown commodity {commodity}")))?
        .precision;
    if i64::from(req.amount.significant_scale()) > i64::from(precision) {
        return Err(Problem::invalid(format!(
            "amount has more than {precision} decimals"
        )));
    }
    let date = req.date.unwrap_or_else(|| OffsetDateTime::now_utc().date());
    let id = Uuid::now_v7();
    let balance = shown(req.amount, owed_positive(&account.kind));
    store::balance_checks::insert(
        &mut *tx,
        id,
        req.account_id,
        commodity,
        date,
        balance.as_decimal(),
        "user",
    )
    .await?;
    let row = one(&mut tx, id).await?;
    let check = output(&mut tx, row, true).await?;
    let mut logged = check.clone();
    logged.candidates = None;
    audit(
        &mut tx,
        principal,
        Action::Create,
        "balance_check",
        id,
        None,
        Some(&logged),
    )
    .await?;
    tx.commit().await?;
    let mut res = (StatusCode::CREATED, etag(VERSION), axum::Json(check)).into_response();
    if let Ok(loc) = format!("/api/v1/balance-checks/{id}").parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Delete a balance check.
#[utoipa::path(delete, path = "/balance-checks/{id}", tag = "balance-checks", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = NO_CONTENT),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json")))]
async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
) -> Result<StatusCode, Problem> {
    if version != VERSION {
        return Err(Problem::stale());
    }
    let mut tx = state.pool.begin().await?;
    let row = one(&mut tx, id).await?;
    let before = output(&mut tx, row, false).await?;
    store::balance_checks::delete(&mut *tx, id).await?;
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "balance_check",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
