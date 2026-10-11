//! `/api/v1/commodities` (design §3.1): currencies, securities, funds, crypto.
//! Precision can only increase: lowering it could invalidate stored amounts.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::audit::Action;
use store::commodities::CommodityRow;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::{audit, delete_error, id_cursor, write_error};
use crate::conventions::{IfMatch, Page, PageParams, etag};
use crate::problem::{Json, Path, Problem, Query};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, update, delete))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommodityKind {
    Fiat,
    Security,
    Fund,
    Crypto,
    Other,
}

impl CommodityKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Fiat => "fiat",
            Self::Security => "security",
            Self::Fund => "fund",
            Self::Crypto => "crypto",
            Self::Other => "other",
        }
    }

    const fn to_domain(self) -> domain::CommodityKind {
        match self {
            Self::Fiat => domain::CommodityKind::Fiat,
            Self::Security => domain::CommodityKind::Security,
            Self::Fund => domain::CommodityKind::Fund,
            Self::Crypto => domain::CommodityKind::Crypto,
            Self::Other => domain::CommodityKind::Other,
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        Ok(match s {
            "fiat" => Self::Fiat,
            "security" => Self::Security,
            "fund" => Self::Fund,
            "crypto" => Self::Crypto,
            "other" => Self::Other,
            _ => return Err(Problem::internal()),
        })
    }
}

#[derive(Serialize, ToSchema)]
pub struct Commodity {
    id: Uuid,
    /// `USD`, `EUR`, `VTI`, `BRK.B`…
    code: String,
    kind: CommodityKind,
    /// Decimal places a quantity may have: 2 for USD, 0 for JPY.
    precision: u32,
    #[serde(skip)]
    version: i64,
}

impl TryFrom<CommodityRow> for Commodity {
    type Error = Problem;
    fn try_from(r: CommodityRow) -> Result<Self, Problem> {
        Ok(Self {
            id: r.id,
            code: r.code,
            kind: CommodityKind::parse(&r.kind)?,
            precision: u32::try_from(r.precision).map_err(|_| Problem::internal())?,
            version: r.version,
        })
    }
}

/// Validates through `domain` and builds the row to store.
fn row(id: Uuid, code: &str, kind: CommodityKind, precision: u32) -> Result<CommodityRow, Problem> {
    let c = domain::Commodity::new(
        domain::CommodityId::from_uuid(id),
        code,
        kind.to_domain(),
        precision,
    )
    .map_err(|e| Problem::invalid(e.to_string()))?;
    Ok(CommodityRow {
        id,
        code: c.code().to_owned(),
        kind: kind.as_str().to_owned(),
        precision: i16::try_from(c.precision()).map_err(|_| Problem::internal())?,
        version: 0,
    })
}

fn respond(status: StatusCode, item: &Commodity) -> Response {
    (status, etag(item.version), axum::Json(item)).into_response()
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewCommodity {
    code: String,
    kind: CommodityKind,
    precision: u32,
}

/// Fields to change; omitted ones stay as they are.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CommodityUpdate {
    code: Option<String>,
    kind: Option<CommodityKind>,
    /// Can only increase.
    precision: Option<u32>,
}

/// List commodities.
#[utoipa::path(get, path = "/commodities", tag = "commodities", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<Commodity>)))]
async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<Commodity>>, Problem> {
    let after = id_cursor(page.cursor.as_deref())?;
    let rows = store::commodities::list(&state.pool, after, page.limit() + 1).await?;
    let items = rows
        .into_iter()
        .map(Commodity::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(axum::Json(Page::from_rows(items, page.limit(), |c| {
        c.id.to_string()
    })))
}

/// Get one commodity.
#[utoipa::path(get, path = "/commodities/{id}", tag = "commodities", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Commodity, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::commodities::get(&state.pool, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &Commodity::try_from(row)?))
}

/// Create a commodity.
#[utoipa::path(post, path = "/commodities", tag = "commodities", security(("bearer" = [])),
    request_body = NewCommodity,
    responses((status = CREATED, body = Commodity, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewCommodity>,
) -> Result<Response, Problem> {
    let new = row(Uuid::now_v7(), &req.code, req.kind, req.precision)?;
    let mut tx = state.pool.begin().await?;
    let saved = store::commodities::upsert(&mut *tx, &new)
        .await
        .map_err(|e| write_error(e, || format!("commodity {} already exists", new.code)))?;
    let item = Commodity::try_from(saved)?;
    audit(
        &mut tx,
        principal,
        Action::Create,
        "commodity",
        item.id,
        None,
        Some(&item),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &item);
    if let Ok(loc) = format!("/api/v1/commodities/{}", item.id).parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Change a commodity.
#[utoipa::path(patch, path = "/commodities/{id}", tag = "commodities", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = CommodityUpdate,
    responses((status = OK, body = Commodity, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json")))]
async fn update(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<CommodityUpdate>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let before = store::commodities::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    let before = Commodity::try_from(before)?;
    let precision = req.precision.unwrap_or(before.precision);
    if precision < before.precision {
        return Err(Problem::invalid(
            "precision can only increase: lowering it could invalidate stored amounts",
        ));
    }
    let changed = row(
        id,
        req.code.as_deref().unwrap_or(&before.code),
        req.kind.unwrap_or(before.kind),
        precision,
    )?;
    let saved = store::commodities::upsert(&mut *tx, &changed)
        .await
        .map_err(|e| write_error(e, || format!("commodity {} already exists", changed.code)))?;
    let after = Commodity::try_from(saved)?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "commodity",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a commodity nothing refers to.
#[utoipa::path(delete, path = "/commodities/{id}", tag = "commodities", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = NO_CONTENT),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "Still in use", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json")))]
async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
) -> Result<StatusCode, Problem> {
    let mut tx = state.pool.begin().await?;
    let before = store::commodities::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    store::commodities::delete(&mut *tx, id)
        .await
        .map_err(delete_error)?;
    let before = Commodity::try_from(before)?;
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "commodity",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
