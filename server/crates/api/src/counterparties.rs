//! `/api/v1/counterparties` (design §3.1). Anyone outside the household. The two built-in
//! counterparties (`opening_balances`, `reconciliation`) can be renamed but not
//! deleted.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::audit::Action;
use store::counterparties::CounterpartyRow;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::{audit, delete_error, id_cursor, valid_name, write_error};
use crate::conventions::{IfMatch, Page, PageParams, etag};
use crate::problem::{Json, Path, Problem, Query};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, update, delete))
}

#[derive(Serialize, ToSchema)]
pub struct Counterparty {
    id: Uuid,
    name: String,
    /// `opening_balances` or `reconciliation` for the built-in ones.
    system: Option<String>,
    #[serde(skip)]
    version: i64,
}

impl From<CounterpartyRow> for Counterparty {
    fn from(r: CounterpartyRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            system: r.system,
            version: r.version,
        }
    }
}

fn respond(status: StatusCode, item: &Counterparty) -> Response {
    (status, etag(item.version), axum::Json(item)).into_response()
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewCounterparty {
    name: String,
}

/// List counterparties.
#[utoipa::path(get, path = "/counterparties", tag = "counterparties", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<Counterparty>)))]
async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<Counterparty>>, Problem> {
    let after = id_cursor(page.cursor.as_deref())?;
    let rows = store::counterparties::list(&state.pool, after, page.limit() + 1).await?;
    let items = rows.into_iter().map(Counterparty::from).collect();
    Ok(axum::Json(Page::from_rows(items, page.limit(), |c| {
        c.id.to_string()
    })))
}

/// Get one counterparty.
#[utoipa::path(get, path = "/counterparties/{id}", tag = "counterparties", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Counterparty, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::counterparties::get(&state.pool, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &Counterparty::from(row)))
}

/// Create a counterparty.
#[utoipa::path(post, path = "/counterparties", tag = "counterparties", security(("bearer" = [])),
    request_body = NewCounterparty,
    responses((status = CREATED, body = Counterparty, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewCounterparty>,
) -> Result<Response, Problem> {
    let name = valid_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let row = store::counterparties::upsert(&mut *tx, Uuid::now_v7(), name)
        .await
        .map_err(|e| {
            write_error(e, || {
                format!("a counterparty named {name:?} already exists")
            })
        })?;
    let item = Counterparty::from(row);
    audit(
        &mut tx,
        principal,
        Action::Create,
        "counterparty",
        item.id,
        None,
        Some(&item),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &item);
    if let Ok(loc) = format!("/api/v1/counterparties/{}", item.id).parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Rename a counterparty.
#[utoipa::path(patch, path = "/counterparties/{id}", tag = "counterparties", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = NewCounterparty,
    responses((status = OK, body = Counterparty, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json")))]
async fn update(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<NewCounterparty>,
) -> Result<Response, Problem> {
    let name = valid_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let before = store::counterparties::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    let row = store::counterparties::upsert(&mut *tx, id, name)
        .await
        .map_err(|e| {
            write_error(e, || {
                format!("a counterparty named {name:?} already exists")
            })
        })?;
    let (before, after) = (Counterparty::from(before), Counterparty::from(row));
    audit(
        &mut tx,
        principal,
        Action::Update,
        "counterparty",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a counterparty nothing refers to.
#[utoipa::path(delete, path = "/counterparties/{id}", tag = "counterparties", security(("bearer" = [])),
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
    let before = store::counterparties::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    if before.system.is_some() {
        return Err(Problem::conflict(
            "built-in counterparties can't be deleted",
        ));
    }
    store::counterparties::delete(&mut *tx, id)
        .await
        .map_err(delete_error)?;
    let before = Counterparty::from(before);
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "counterparty",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
