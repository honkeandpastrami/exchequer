//! `/api/v1/settlements` (design §4.3–§4.5): each month's parameters
//! (stipends, funders), its statement, closing and reopening, and the
//! history used to validate migrated data (implied stipends).
//!
//! Statements are computed by `domain::settlement` from the whole ledger on
//! every request; nothing about them is stored.

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use domain::settlement::{
    self as ds, Item, LineKind as DLineKind, MonthParams, SettlementError, SplitRef,
};
use domain::{Amount, CommodityId, Month, PartyId, PartyKind, TransactionId};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use store::audit::Action;
use store::settlements::{Params, SettlementRow};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::audit;
use crate::conventions::{IfMatch, etag};
use crate::ledger::{book_for, party};
use crate::prices::ValuationOut;
use crate::pricing::{Need, Pricing};
use crate::problem::{Json, Path, Problem, Query};
use crate::transactions::{domain_split, new_split_from_row};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(history))
        .routes(routes!(get, put))
        .routes(routes!(statement))
        .routes(routes!(close))
        .routes(routes!(reopen))
}

/// The most months one history request covers.
const MAX_MONTHS: usize = 120;

// ------------------------------------------------------------- Schemas ----

/// What the Household owes a person for the month.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Stipend {
    party_id: Uuid,
    /// In USD; zero or more.
    #[schema(value_type = String, example = "650.00")]
    amount: Amount,
}

/// Who funds a business for the month: the household or a person.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Funder {
    business_id: Uuid,
    funder_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SettlementParams {
    #[serde(default)]
    stipends: Vec<Stipend>,
    /// Businesses left out are funded by the household.
    #[serde(default)]
    funders: Vec<Funder>,
}

/// A month's parameters and state.
#[derive(Debug, Serialize, ToSchema)]
pub struct Settlement {
    #[schema(value_type = String, example = "2026-06")]
    month: Month,
    closed: bool,
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    closed_at: Option<OffsetDateTime>,
    /// The month these parameters were set for: this one, an earlier one
    /// they're carried forward from, or absent when none were ever set.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>)]
    params_from: Option<Month>,
    stipends: Vec<Stipend>,
    funders: Vec<Funder>,
    #[serde(skip)]
    version: i64,
}

/// One split, by its transaction and position.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct SplitPointer {
    transaction_id: Uuid,
    position: usize,
}

impl From<SplitRef> for SplitPointer {
    fn from(r: SplitRef) -> Self {
        Self {
            transaction_id: *r.transaction.as_uuid(),
            position: r.position,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    /// Income or expense whose purpose isn't its liquid account's owner.
    Claim,
    /// A settlement payment.
    Payment,
}

impl From<DLineKind> for LineKind {
    fn from(k: DLineKind) -> Self {
        match k {
            DLineKind::Claim => Self::Claim,
            DLineKind::Payment => Self::Payment,
        }
    }
}

/// An item on an edge. Positive amounts go from the child to the parent.
#[derive(Debug, Serialize, ToSchema)]
pub struct Line {
    #[serde(flatten)]
    split: SplitPointer,
    date: Date,
    kind: LineKind,
    #[schema(value_type = String)]
    amount: Amount,
    /// For a split with no leg in USD: the price that valued it.
    #[serde(skip_serializing_if = "Option::is_none")]
    valuation: Option<ValuationOut>,
}

/// One edge of the settlement tree (a child and its parent) for one month.
/// Positive amounts go from the child to the parent.
#[derive(Debug, Serialize, ToSchema)]
pub struct Edge {
    child_id: Uuid,
    parent_id: Uuid,
    /// Regular claims across the edge this month.
    #[schema(value_type = String)]
    claims: Amount,
    /// What the parent owes the child as stipends this month.
    #[schema(value_type = String)]
    stipend: Amount,
    /// `claims − stipend`.
    #[schema(value_type = String)]
    due: Amount,
    /// Regular payments that settle this month.
    #[schema(value_type = String)]
    paid: Amount,
    /// `outstanding` at the end of the previous month: past differences,
    /// including corrections to closed months, carried into this one.
    #[schema(value_type = String)]
    carried: Amount,
    /// `carried + due − paid`: everything up to the end of this month, the
    /// transfer to make (positive: the child pays).
    #[schema(value_type = String)]
    outstanding: Amount,
    /// `claims − paid`: the stipend that would explain the payments made.
    #[schema(value_type = String)]
    implied_stipend: Amount,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<Vec<Line>>,
}

/// A split the settlement couldn't value: neither leg is in USD, and no
/// price connects its commodity to USD.
#[derive(Debug, Serialize, ToSchema)]
pub struct Unpriced {
    #[serde(flatten)]
    split: SplitPointer,
    date: Date,
}

/// The statement for one month (design §4.3).
#[derive(Debug, Serialize, ToSchema)]
pub struct Statement {
    #[schema(value_type = String, example = "2026-06")]
    month: Month,
    closed: bool,
    currency_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>)]
    params_from: Option<Month>,
    edges: Vec<Edge>,
    /// Splits left out because they have no value in USD.
    unpriced: Vec<Unpriced>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct History {
    currency_id: Uuid,
    /// One statement per month, without lines.
    months: Vec<Statement>,
    unpriced: Vec<Unpriced>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryParams {
    /// First month, `YYYY-MM`.
    #[param(value_type = String)]
    from: Month,
    /// Last month, `YYYY-MM` (at most 120 months in all).
    #[param(value_type = String)]
    to: Month,
}

// ------------------------------------------------------------- Helpers ----

fn month_of(d: Date) -> Month {
    Month::of(d)
}

fn respond(status: StatusCode, s: &Settlement) -> Response {
    (status, etag(s.version), axum::Json(s)).into_response()
}

fn domain_params(p: &Params) -> MonthParams {
    MonthParams {
        stipends: p
            .stipends
            .iter()
            .map(|(id, a)| (PartyId::from_uuid(*id), Amount::from_decimal(*a)))
            .collect(),
        funders: p
            .funders
            .iter()
            .map(|(b, f)| (PartyId::from_uuid(*b), PartyId::from_uuid(*f)))
            .collect(),
    }
}

async fn all_params(db: &mut PgConnection) -> Result<BTreeMap<Month, Params>, Problem> {
    Ok(store::settlements::all_params(db)
        .await?
        .into_iter()
        .map(|(d, p)| (month_of(d), p))
        .collect())
}

/// The month's own row and the parameters in force (its own, or carried
/// forward from the latest earlier month).
async fn settlement_for(db: &mut PgConnection, month: Month) -> Result<Settlement, Problem> {
    let row = store::settlements::get(&mut *db, month.first_day(), false).await?;
    let params = all_params(db).await?;
    let (from, p) = params
        .range(..=month)
        .next_back()
        .map_or((None, Params::default()), |(m, p)| (Some(*m), p.clone()));
    let mut stipends: Vec<Stipend> = p
        .stipends
        .into_iter()
        .map(|(party_id, a)| Stipend {
            party_id,
            amount: Amount::from_decimal(a),
        })
        .collect();
    stipends.sort_by_key(|s| s.party_id);
    let mut funders: Vec<Funder> = p
        .funders
        .into_iter()
        .map(|(business_id, funder_id)| Funder {
            business_id,
            funder_id,
        })
        .collect();
    funders.sort_by_key(|f| f.business_id);
    Ok(Settlement {
        month,
        closed: row.as_ref().is_some_and(|r| r.closed_at.is_some()),
        closed_at: row.as_ref().and_then(|r| r.closed_at),
        params_from: from,
        stipends,
        funders,
        version: row.map_or(0, |r| r.version),
    })
}

/// Settlement is always in USD (design §4.3).
pub(crate) const CURRENCY: &str = "USD";

/// The settlement currency's commodity, if it exists yet.
pub(crate) async fn currency(db: &mut PgConnection) -> Result<Option<CommodityId>, Problem> {
    Ok(store::commodities::by_code(&mut *db, CURRENCY)
        .await?
        .map(|c| CommodityId::from_uuid(c.id)))
}

/// The household and the settlement currency.
pub(crate) async fn household(db: &mut PgConnection) -> Result<(PartyId, CommodityId), Problem> {
    let h = store::parties::household(&mut *db)
        .await?
        .ok_or_else(|| Problem::conflict("create the household party first"))?;
    let currency = currency(db).await?.ok_or_else(|| {
        Problem::conflict(format!(
            "settlement is in {CURRENCY}: create the {CURRENCY} commodity first"
        ))
    })?;
    Ok((PartyId::from_uuid(h.id), currency))
}

pub(crate) struct Computed {
    pub(crate) currency: CommodityId,
    /// The currency's precision: every amount is shown at it.
    pub(crate) scale: u32,
    pub(crate) settlement: ds::Settlement,
    unpriced: Vec<(SplitRef, Date)>,
    /// Splits with no leg in USD, valued at a price.
    valued: HashMap<SplitRef, ValuationOut>,
    closed: HashSet<Month>,
}

/// Every settlement item in the ledger, routed through `through`. A split
/// with no leg in USD is valued at its date's price (fetched if missing).
pub(crate) async fn compute(
    db: &mut PgConnection,
    pricing: &Pricing,
    today: Date,
    through: Month,
) -> Result<Computed, Problem> {
    let (_, currency) = household(db).await?;
    let scale = currency_scale(db, currency).await?;
    let parties = store::parties::all(&mut *db)
        .await?
        .into_iter()
        .map(party)
        .collect::<Result<Vec<_>, _>>()?;

    let rows = store::transactions::settlement_splits(&mut *db).await?;
    // Stored splits passed these checks when written; failing now is a bug.
    let splits = rows
        .iter()
        .map(|r| domain_split(&new_split_from_row(&r.split), 0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Problem::internal())?;
    let book = book_for(db, &splits).await?;

    let mut items = Vec::new();
    // Items with no leg in USD, and the leg to value.
    let mut pending = Vec::new();
    for (row, split) in rows.iter().zip(&splits) {
        let source = SplitRef {
            transaction: TransactionId::from_uuid(row.split.transaction_id),
            position: usize::try_from(row.split.position).map_err(|_| Problem::internal())?,
        };
        let classified = split.classify(&book).map_err(|_| Problem::internal())?;
        let Some(effect) = ds::what(split, &classified, &book) else {
            continue;
        };
        let item = |amount| Item {
            source,
            date: row.date,
            effect,
            amount,
            settles: split.settles,
        };
        match ds::value(split, &book, currency) {
            Ok(amount) => items.push(item(amount)),
            Err(SettlementError::Unpriced) => {
                pending.push((item(Amount::ZERO), ds::valued_leg(split, &book).clone()));
            }
            Err(_) => return Err(Problem::internal()),
        }
    }

    let mut unpriced = Vec::new();
    let mut valued = HashMap::new();
    if !pending.is_empty() {
        let needs: Vec<Need> = pending
            .iter()
            .map(|(item, leg)| Need {
                from: *leg.commodity.as_uuid(),
                to: *currency.as_uuid(),
                date: item.date,
            })
            .collect();
        let loaded = pricing.load(db, today, &needs, false).await?;
        for (mut item, leg) in pending {
            match loaded.prices.value(leg.commodity, currency, item.date) {
                Some(v) => {
                    item.amount = v
                        .rate
                        .convert(leg.quantity, scale)
                        .map_err(|_| Problem::internal())?;
                    valued.insert(item.source, ValuationOut::new(&v, &loaded.rows));
                    items.push(item);
                }
                None => unpriced.push((item.source, item.date)),
            }
        }
    }

    let params: BTreeMap<Month, MonthParams> = all_params(db)
        .await?
        .iter()
        .map(|(m, p)| (*m, domain_params(p)))
        .collect();
    let settlement = ds::Settlement::compute(&parties, &params, &items, through)
        .map_err(|_| Problem::internal())?;
    let closed = store::settlements::all(&mut *db)
        .await?
        .into_iter()
        .filter(|r| r.closed_at.is_some())
        .map(|r| month_of(r.month))
        .collect();
    Ok(Computed {
        currency,
        scale,
        settlement,
        unpriced,
        valued,
        closed,
    })
}

async fn currency_scale(db: &mut PgConnection, currency: CommodityId) -> Result<u32, Problem> {
    let precision = store::commodities::get(&mut *db, *currency.as_uuid(), false)
        .await?
        .ok_or_else(Problem::internal)?
        .precision;
    u32::try_from(precision).map_err(|_| Problem::internal())
}

/// `a` at exactly `scale` decimals, never `-0`. Settlement values are sums
/// of amounts in the currency, so this never rounds.
pub(crate) fn money(a: Amount, scale: u32) -> Amount {
    let mut d = a.as_decimal();
    if d.is_zero() {
        d = rust_decimal::Decimal::ZERO;
    }
    d.rescale(scale);
    Amount::from_decimal(d)
}

fn statement_out(c: &Computed, month: Month, with_lines: bool) -> Result<Statement, Problem> {
    let m = |a: Amount| money(a, c.scale);
    let st = c
        .settlement
        .statement(month)
        .map_err(|_| Problem::internal())?;
    let edges = st
        .edges
        .into_iter()
        .map(|e| {
            Ok(Edge {
                child_id: *e.child.as_uuid(),
                parent_id: *e.parent.as_uuid(),
                implied_stipend: m(e.implied_stipend().map_err(|_| Problem::internal())?),
                claims: m(e.claims),
                stipend: m(e.stipend),
                due: m(e.due),
                paid: m(e.paid),
                carried: m(e.carried),
                outstanding: m(e.outstanding),
                lines: with_lines.then(|| {
                    e.lines
                        .into_iter()
                        .map(|l| Line {
                            split: l.source.into(),
                            date: l.date,
                            kind: l.kind.into(),
                            amount: m(l.amount),
                            valuation: c.valued.get(&l.source).cloned(),
                        })
                        .collect()
                }),
            })
        })
        .collect::<Result<Vec<_>, Problem>>()?;
    Ok(Statement {
        month,
        closed: c.closed.contains(&month),
        currency_id: *c.currency.as_uuid(),
        params_from: st.params_from,
        edges,
        unpriced: Vec::new(),
    })
}

fn unpriced_out(c: &Computed, through: Month) -> Vec<Unpriced> {
    c.unpriced
        .iter()
        .filter(|(_, d)| month_of(*d) <= through)
        .map(|(r, d)| Unpriced {
            split: (*r).into(),
            date: *d,
        })
        .collect()
}

/// Checks a month's parameters against the parties and the currency.
async fn check_params(db: &mut PgConnection, req: &SettlementParams) -> Result<Params, Problem> {
    let (_, currency) = household(db).await?;
    let precision = currency_scale(db, currency).await?;
    let kinds: HashMap<Uuid, PartyKind> = store::parties::all(&mut *db)
        .await?
        .into_iter()
        .map(|r| party(r).map(|p| (*p.id.as_uuid(), p.kind)))
        .collect::<Result<_, _>>()?;

    let mut out = Params::default();
    let mut seen = HashSet::new();
    for s in &req.stipends {
        if kinds.get(&s.party_id) != Some(&PartyKind::Person) {
            return Err(Problem::invalid(format!(
                "stipends: {} is not a person",
                s.party_id
            )));
        }
        if !seen.insert(s.party_id) {
            return Err(Problem::invalid(format!(
                "stipends: {} appears twice",
                s.party_id
            )));
        }
        if s.amount < Amount::ZERO || i64::from(s.amount.significant_scale()) > i64::from(precision)
        {
            return Err(Problem::invalid(format!(
                "stipends: {} must be zero or more, with at most {precision} decimals",
                s.amount
            )));
        }
        out.stipends.push((s.party_id, s.amount.as_decimal()));
    }
    let mut seen = HashSet::new();
    for f in &req.funders {
        if kinds.get(&f.business_id) != Some(&PartyKind::Business) {
            return Err(Problem::invalid(format!(
                "funders: {} is not a business",
                f.business_id
            )));
        }
        if !matches!(
            kinds.get(&f.funder_id),
            Some(PartyKind::Household | PartyKind::Person)
        ) {
            return Err(Problem::invalid(format!(
                "funders: {} must be the household or a person",
                f.funder_id
            )));
        }
        if !seen.insert(f.business_id) {
            return Err(Problem::invalid(format!(
                "funders: {} appears twice",
                f.business_id
            )));
        }
        out.funders.push((f.business_id, f.funder_id));
    }
    Ok(out)
}

/// What closing still needs: every person's stipend and every business's
/// funder set for the month itself.
async fn missing_params(db: &mut PgConnection, month: Month) -> Result<Vec<String>, Problem> {
    let params = all_params(db).await?;
    let own = params.get(&month);
    let mut missing = Vec::new();
    for p in store::parties::all(&mut *db).await? {
        let set = match p.kind.as_str() {
            "person" => own.is_some_and(|o| o.stipends.iter().any(|(id, _)| *id == p.id)),
            "business" => own.is_some_and(|o| o.funders.iter().any(|(id, _)| *id == p.id)),
            _ => true,
        };
        if !set {
            let what = if p.kind == "person" {
                "stipend"
            } else {
                "funder"
            };
            missing.push(format!("{what} for {}", p.name));
        }
    }
    Ok(missing)
}

async fn locked(
    db: &mut PgConnection,
    month: Month,
    version: i64,
) -> Result<Option<SettlementRow>, Problem> {
    let row = store::settlements::get(&mut *db, month.first_day(), true).await?;
    if row.as_ref().map_or(0, |r| r.version) != version {
        return Err(Problem::stale());
    }
    Ok(row)
}

// ------------------------------------------------------------ Handlers ----

/// Statements for a range of months, without their lines: the history used
/// to check migrated data (each edge's `implied_stipend`, design §4.5).
#[utoipa::path(get, path = "/settlements", tag = "settlements", security(("bearer" = [])),
    params(HistoryParams),
    responses((status = OK, body = History),
              (status = CONFLICT, description = "No USD commodity yet", body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn history(
    State(state): State<AppState>,
    Query(p): Query<HistoryParams>,
) -> Result<axum::Json<History>, Problem> {
    let mut months = Vec::new();
    let mut m = p.from;
    while m <= p.to {
        if months.len() == MAX_MONTHS {
            return Err(Problem::invalid(format!("at most {MAX_MONTHS} months")));
        }
        months.push(m);
        m = m.next();
    }
    if months.is_empty() {
        return Err(Problem::invalid("`from` is after `to`"));
    }
    let mut conn = state.pool.acquire().await?;
    let c = compute(&mut conn, &state.pricing, state.today(), p.to).await?;
    let mut out = Vec::new();
    for m in months {
        out.push(statement_out(&c, m, false)?);
    }
    Ok(axum::Json(History {
        currency_id: *c.currency.as_uuid(),
        months: out,
        unpriced: unpriced_out(&c, p.to),
    }))
}

/// A month's parameters (its own, or carried forward) and whether it's closed.
#[utoipa::path(get, path = "/settlements/{month}", tag = "settlements", security(("bearer" = [])),
    params(("month" = String, Path, description = "`YYYY-MM`")),
    responses((status = OK, body = Settlement, headers(("ETag" = String))),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(month): Path<Month>) -> Result<Response, Problem> {
    let mut conn = state.pool.acquire().await?;
    Ok(respond(
        StatusCode::OK,
        &settlement_for(&mut conn, month).await?,
    ))
}

/// Set a month's stipends and funders. `If-Match` is the ETag from GET
/// (`"0"` for a month never set).
#[utoipa::path(put, path = "/settlements/{month}", tag = "settlements", security(("bearer" = [])),
    params(("month" = String, Path, description = "`YYYY-MM`"),
           ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = SettlementParams,
    responses((status = OK, body = Settlement, headers(("ETag" = String))),
              (status = CONFLICT, description = "The month is closed, or no USD commodity yet", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn put(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(month): Path<Month>,
    IfMatch(version): IfMatch,
    Json(req): Json<SettlementParams>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let row = locked(&mut tx, month, version).await?;
    if row.as_ref().is_some_and(|r| r.closed_at.is_some()) {
        return Err(Problem::month_closed(month));
    }
    let params = check_params(&mut tx, &req).await?;
    let before = match &row {
        Some(_) => Some(settlement_for(&mut tx, month).await?),
        None => None,
    };
    let row = store::settlements::put(&mut tx, month.first_day(), &params).await?;
    let after = settlement_for(&mut tx, month).await?;
    let action = if before.is_some() {
        Action::Update
    } else {
        Action::Create
    };
    audit(
        &mut tx,
        principal,
        action,
        "settlement",
        row.id,
        before.as_ref(),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// The month's statement: each edge's claims, stipend, due, paid,
/// outstanding and lines (design §4.3). Always computed from current data.
#[utoipa::path(get, path = "/settlements/{month}/statement", tag = "settlements", security(("bearer" = [])),
    params(("month" = String, Path, description = "`YYYY-MM`")),
    responses((status = OK, body = Statement),
              (status = CONFLICT, description = "No USD commodity yet", body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn statement(
    State(state): State<AppState>,
    Path(month): Path<Month>,
) -> Result<axum::Json<Statement>, Problem> {
    let mut conn = state.pool.acquire().await?;
    let c = compute(&mut conn, &state.pricing, state.today(), month).await?;
    let mut st = statement_out(&c, month, true)?;
    st.unpriced = unpriced_out(&c, month);
    Ok(axum::Json(st))
}

/// Close a month: edits to transactions dated in it are refused until it's
/// reopened. Needs every person's stipend and every business's funder set
/// for the month itself.
#[utoipa::path(post, path = "/settlements/{month}/close", tag = "settlements", security(("bearer" = [])),
    params(("month" = String, Path, description = "`YYYY-MM`"),
           ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = OK, body = Settlement, headers(("ETag" = String))),
              (status = CONFLICT, description = "Already closed, or parameters missing", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json")))]
async fn close(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(month): Path<Month>,
    IfMatch(version): IfMatch,
) -> Result<Response, Problem> {
    set_closed(state, principal, month, version, true).await
}

/// Reopen a closed month. Always allowed (design §4.4).
#[utoipa::path(post, path = "/settlements/{month}/reopen", tag = "settlements", security(("bearer" = [])),
    params(("month" = String, Path, description = "`YYYY-MM`"),
           ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = OK, body = Settlement, headers(("ETag" = String))),
              (status = CONFLICT, description = "Not closed", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json")))]
async fn reopen(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(month): Path<Month>,
    IfMatch(version): IfMatch,
) -> Result<Response, Problem> {
    set_closed(state, principal, month, version, false).await
}

async fn set_closed(
    state: AppState,
    principal: Principal,
    month: Month,
    version: i64,
    closing: bool,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let row = locked(&mut tx, month, version).await?;
    let is_closed = row.as_ref().is_some_and(|r| r.closed_at.is_some());
    if closing {
        if is_closed {
            return Err(Problem::conflict(format!("{month} is already closed")));
        }
        household(&mut tx).await?;
        let missing = missing_params(&mut tx, month).await?;
        if !missing.is_empty() {
            return Err(Problem::conflict(format!(
                "set the month's parameters first; missing: {}",
                missing.join(", ")
            )));
        }
    } else if !is_closed {
        return Err(Problem::conflict(format!("{month} is not closed")));
    }
    let before = settlement_for(&mut tx, month).await?;
    let row = store::settlements::set_closed(&mut *tx, month.first_day(), closing).await?;
    let after = settlement_for(&mut tx, month).await?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "settlement",
        row.id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}
