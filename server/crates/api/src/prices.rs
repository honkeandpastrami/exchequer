//! `/api/v1/prices` (design §3.4): `1 base = rate quote` at the end of a
//! day, entered by hand or fetched from a price source, and what one
//! commodity is worth in another.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use domain::valuation::{self, Source, Valuation};
use domain::{CommodityId, Rate};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use store::audit::Action;
use store::prices::PriceRow;
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::{audit, write_error};
use crate::conventions::{IfMatch, Page, etag};
use crate::pricing::{Need, Refresh, same, to_domain};
use crate::problem::{Json, Path, Problem, Query};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(value))
        .routes(routes!(refresh, refresh_status))
        .routes(routes!(get, update, delete))
}

// ------------------------------------------------------------- Schemas ----

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Price {
    id: Uuid,
    base_id: Uuid,
    quote_id: Uuid,
    /// At the end of this day.
    date: Date,
    /// `1 base = rate quote`.
    #[schema(value_type = String, example = "1.0842")]
    rate: Rate,
    /// `manual`, or `provider:<name>` (`provider:tiingo`, `provider:ecb`).
    source: String,
    /// For a day still in progress: replaced by the close once available.
    provisional: bool,
    /// When it was entered or fetched.
    #[serde(with = "time::serde::rfc3339")]
    fetched_at: OffsetDateTime,
    #[serde(skip)]
    version: i64,
}

impl TryFrom<PriceRow> for Price {
    type Error = Problem;
    fn try_from(r: PriceRow) -> Result<Self, Problem> {
        Ok(Self {
            id: r.id,
            base_id: r.base_id,
            quote_id: r.quote_id,
            date: r.date,
            rate: r.rate.parse().map_err(|_| Problem::internal())?,
            source: r.source,
            provisional: r.provisional,
            fetched_at: r.fetched_at,
            version: r.version,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewPrice {
    base_id: Uuid,
    quote_id: Uuid,
    date: Date,
    #[schema(value_type = String, example = "1.0842")]
    rate: Rate,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PriceUpdate {
    #[schema(value_type = String, example = "1.0842")]
    rate: Rate,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Prices with this commodity on either side.
    commodity_id: Option<Uuid>,
    /// `manual` or `provider:<name>`.
    source: Option<String>,
    /// Items per page (default 100, max 500).
    limit: Option<u32>,
    /// `next_cursor` from the previous page. Opaque.
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ValueParams {
    /// A commodity code (`VTI`).
    from: String,
    /// A commodity code (`USD`).
    to: String,
    /// At the end of this day. Default: today in the server's time zone.
    date: Option<Date>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RefreshRequest {
    /// A commodity code. Default: `USD`.
    value_in: Option<String>,
    /// Default: `to`.
    from: Option<Date>,
    /// Default (and at most): today in the server's time zone.
    to: Option<Date>,
}

/// A refresh: running, or how it went.
#[derive(Debug, Serialize, ToSchema)]
pub struct Refreshed {
    /// The configured sources, in the order asked.
    sources: Vec<String>,
    /// Still fetching: ask `GET /prices/refresh` again shortly.
    running: bool,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    finished_at: Option<OffsetDateTime>,
    /// Prices stored or replaced.
    stored: usize,
    /// `source: what went wrong`, for each failure; the next source was
    /// asked instead.
    errors: Vec<String>,
}

impl Refreshed {
    fn new(sources: Vec<String>, r: Refresh) -> Self {
        Self {
            sources,
            running: r.finished_at.is_none(),
            started_at: r.started_at,
            finished_at: r.finished_at,
            stored: r.stored,
            errors: r.errors,
        }
    }
}

/// A valuation, and whether fresher prices for it are being fetched.
#[derive(Debug, Serialize, ToSchema)]
pub struct Valued {
    #[serde(flatten)]
    valuation: ValuationOut,
    /// Missing or stale prices are being fetched: asked again shortly, the
    /// answer may be fresher.
    fetching: bool,
}

/// A stored price as a valuation used it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PriceUsed {
    id: Uuid,
    base_id: Uuid,
    quote_id: Uuid,
    date: Date,
    #[schema(value_type = String)]
    rate: Rate,
    source: String,
    /// Used as `1 quote = 1/rate base`.
    inverted: bool,
}

/// What one commodity was worth in another, and which prices say so.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ValuationOut {
    /// `1 from = rate to`.
    #[schema(value_type = String, example = "312.45")]
    rate: Rate,
    /// One price, or two chained through a pivot commodity (`derived`).
    prices: Vec<PriceUsed>,
    /// Chained through another commodity, not one price.
    derived: bool,
    /// No price close enough to the date: the newest older one was used.
    stale: bool,
    /// Uses a price for a day still in progress.
    provisional: bool,
}

impl ValuationOut {
    /// IDs aren't part of a domain price: they're looked up by what is.
    pub fn new(v: &Valuation, rows: &[PriceRow]) -> Self {
        let id = |p: &valuation::Price| {
            rows.iter()
                .find(|r| same(r, p))
                .map(|r| r.id)
                .unwrap_or_default()
        };
        Self {
            rate: v.rate.clone(),
            prices: v
                .used
                .iter()
                .map(|u| PriceUsed {
                    id: id(&u.price),
                    base_id: *u.price.base.as_uuid(),
                    quote_id: *u.price.quote.as_uuid(),
                    date: u.price.date,
                    rate: u.price.rate.clone(),
                    source: u.price.source.to_string(),
                    inverted: u.inverted,
                })
                .collect(),
            derived: v.derived(),
            stale: v.stale,
            provisional: v.provisional(),
        }
    }
}

// ------------------------------------------------------------- Helpers ----

fn respond(status: StatusCode, item: &Price) -> Response {
    (status, etag(item.version), axum::Json(item)).into_response()
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

fn checked(p: &valuation::Price) -> Result<(), Problem> {
    p.check().map_err(|e| Problem::invalid(e.to_string()))
}

async fn commodity(db: &mut PgConnection, code: &str) -> Result<Uuid, Problem> {
    Ok(store::commodities::by_code(&mut *db, code)
        .await?
        .ok_or_else(|| Problem::invalid(format!("no commodity {code}")))?
        .id)
}

// ------------------------------------------------------------ Handlers ----

/// List prices, newest first.
#[utoipa::path(get, path = "/prices", tag = "prices", security(("bearer" = [])),
    params(ListParams),
    responses((status = OK, body = Page<Price>),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<axum::Json<Page<Price>>, Problem> {
    let after = p.cursor.as_deref().map(parse_cursor).transpose()?;
    let limit = i64::from(p.limit.unwrap_or(100).clamp(1, 500));
    let rows = store::prices::list(
        &state.pool,
        p.commodity_id,
        p.source.as_deref(),
        after,
        limit + 1,
    )
    .await?;
    let items = rows
        .into_iter()
        .map(Price::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(axum::Json(Page::from_rows(items, limit, |p| {
        format!("{}_{}", p.date, p.id)
    })))
}

/// Get one price.
#[utoipa::path(get, path = "/prices/{id}", tag = "prices", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Price, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::prices::get(&state.pool, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &Price::try_from(row)?))
}

/// Enter a price by hand. It wins over fetched ones (design §3.4).
#[utoipa::path(post, path = "/prices", tag = "prices", security(("bearer" = [])),
    request_body = NewPrice,
    responses((status = CREATED, body = Price, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, description = "A manual price for that pair and day exists", body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewPrice>,
) -> Result<Response, Problem> {
    checked(&valuation::Price {
        base: CommodityId::from_uuid(req.base_id),
        quote: CommodityId::from_uuid(req.quote_id),
        date: req.date,
        rate: req.rate.clone(),
        source: Source::Manual,
        provisional: false,
    })?;
    let mut tx = state.pool.begin().await?;
    let saved = store::prices::insert(
        &mut *tx,
        &PriceRow {
            id: Uuid::now_v7(),
            base_id: req.base_id,
            quote_id: req.quote_id,
            date: req.date,
            rate: req.rate.to_string(),
            source: Source::Manual.to_string(),
            provisional: false,
            fetched_at: OffsetDateTime::now_utc(),
            version: 1,
        },
    )
    .await
    .map_err(|e| {
        write_error(e, || {
            format!(
                "a manual price for that pair on {} exists; change it instead",
                req.date
            )
        })
    })?;
    let item = Price::try_from(saved)?;
    audit(
        &mut tx,
        principal,
        Action::Create,
        "price",
        item.id,
        None,
        Some(&item),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &item);
    if let Ok(loc) = format!("/api/v1/prices/{}", item.id).parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Change a manual price's rate.
#[utoipa::path(patch, path = "/prices/{id}", tag = "prices", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = PriceUpdate,
    responses((status = OK, body = Price, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, description = "A fetched price", body = Problem, content_type = "application/problem+json")))]
async fn update(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<PriceUpdate>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let before = store::prices::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    if before.source != Source::Manual.to_string() {
        return Err(Problem::invalid(
            "a fetched price can't be changed; enter a manual price for that day instead",
        ));
    }
    let mut domain_price = to_domain(&before)?;
    domain_price.rate = req.rate.clone();
    checked(&domain_price)?;
    let before = Price::try_from(before)?;
    let saved = store::prices::set_rate(&mut *tx, id, &req.rate.to_string())
        .await
        .map_err(|e| write_error(e, String::new))?;
    let after = Price::try_from(saved)?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "price",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a price. A fetched one may be fetched again when next needed.
#[utoipa::path(delete, path = "/prices/{id}", tag = "prices", security(("bearer" = [])),
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
    let mut tx = state.pool.begin().await?;
    let before = store::prices::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    store::prices::delete(&mut *tx, id).await?;
    let before = Price::try_from(before)?;
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "price",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// What one commodity was worth in another at the end of a day, from
/// stored prices. Missing ones are fetched after it answers.
#[utoipa::path(get, path = "/prices/value", tag = "prices", security(("bearer" = [])),
    params(ValueParams),
    responses((status = OK, body = Valued),
              (status = NOT_FOUND, description = "No stored price connects them (yet, if being fetched)", body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn value(
    State(state): State<AppState>,
    Query(p): Query<ValueParams>,
) -> Result<axum::Json<Valued>, Problem> {
    let date = p.date.unwrap_or_else(|| state.today());
    let mut conn = state.pool.acquire().await?;
    let from = commodity(&mut conn, &p.from).await?;
    let to = commodity(&mut conn, &p.to).await?;
    let need = Need { from, to, date };
    let loaded = state
        .pricing
        .load(&state.pool, &mut conn, state.today(), &[need])
        .await?;
    let v = loaded
        .prices
        .value(
            CommodityId::from_uuid(from),
            CommodityId::from_uuid(to),
            date,
        )
        .ok_or_else(|| {
            let yet = if loaded.fetching {
                " yet; fetching, ask again shortly"
            } else {
                ""
            };
            Problem::not_found().detail(format!("no price connects {} and {}{yet}", p.from, p.to))
        })?;
    Ok(axum::Json(Valued {
        valuation: ValuationOut::new(&v, &loaded.rows),
        fetching: loaded.fetching,
    }))
}

/// Start fetching every commodity's prices in `value_in` for a range of
/// days, again, from the configured sources in order. It answers at once;
/// `GET /prices/refresh` tells how it went. While one runs, another isn't
/// started: the one running is returned.
#[utoipa::path(post, path = "/prices/refresh", tag = "prices", security(("bearer" = [])),
    request_body = RefreshRequest,
    responses((status = ACCEPTED, body = Refreshed),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<(StatusCode, axum::Json<Refreshed>), Problem> {
    let today = state.today();
    let to_date = req.to.unwrap_or(today).min(today);
    let from_date = req.from.unwrap_or(to_date);
    if from_date > to_date {
        return Err(Problem::invalid("from is after to"));
    }
    let mut conn = state.pool.acquire().await?;
    let target = commodity(&mut conn, req.value_in.as_deref().unwrap_or("USD")).await?;
    let needs: Vec<Need> = store::commodities::list(&mut *conn, None, i64::MAX)
        .await?
        .into_iter()
        .filter(|c| c.id != target)
        .flat_map(|c| {
            [from_date, to_date].map(|date| Need {
                from: c.id,
                to: target,
                date,
            })
        })
        .collect();
    let r = state
        .pricing
        .refresh(&state.pool, &mut conn, today, &needs)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        axum::Json(Refreshed::new(state.pricing.names(), r)),
    ))
}

/// The latest refresh since the server started: running, or how it went.
#[utoipa::path(get, path = "/prices/refresh", tag = "prices", security(("bearer" = [])),
    responses((status = OK, body = Refreshed),
              (status = NOT_FOUND, description = "None since the server started", body = Problem, content_type = "application/problem+json")))]
async fn refresh_status(State(state): State<AppState>) -> Result<axum::Json<Refreshed>, Problem> {
    let r = state
        .pricing
        .last_refresh()
        .ok_or_else(Problem::not_found)?;
    Ok(axum::Json(Refreshed::new(state.pricing.names(), r)))
}
