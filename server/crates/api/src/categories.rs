//! `/api/v1/categories` (design §3.1). A category is the class of an income or expense,
//! never where the money went.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::audit::Action;
use store::categories::CategoryRow;
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
pub struct Category {
    id: Uuid,
    name: String,
    #[serde(skip)]
    version: i64,
}

impl From<CategoryRow> for Category {
    fn from(r: CategoryRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            version: r.version,
        }
    }
}

fn respond(status: StatusCode, item: &Category) -> Response {
    (status, etag(item.version), axum::Json(item)).into_response()
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewCategory {
    name: String,
}

/// List categories.
#[utoipa::path(get, path = "/categories", tag = "categories", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<Category>)))]
async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<Category>>, Problem> {
    let after = id_cursor(page.cursor.as_deref())?;
    let rows = store::categories::list(&state.pool, after, page.limit() + 1).await?;
    let items = rows.into_iter().map(Category::from).collect();
    Ok(axum::Json(Page::from_rows(items, page.limit(), |c| {
        c.id.to_string()
    })))
}

/// Get one category.
#[utoipa::path(get, path = "/categories/{id}", tag = "categories", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Category, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::categories::get(&state.pool, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &Category::from(row)))
}

/// Create a category.
#[utoipa::path(post, path = "/categories", tag = "categories", security(("bearer" = [])),
    request_body = NewCategory,
    responses((status = CREATED, body = Category, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewCategory>,
) -> Result<Response, Problem> {
    let name = valid_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let row = store::categories::upsert(&mut *tx, Uuid::now_v7(), name)
        .await
        .map_err(|e| write_error(e, || format!("a category named {name:?} already exists")))?;
    let item = Category::from(row);
    audit(
        &mut tx,
        principal,
        Action::Create,
        "category",
        item.id,
        None,
        Some(&item),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &item);
    if let Ok(loc) = format!("/api/v1/categories/{}", item.id).parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Rename a category.
#[utoipa::path(patch, path = "/categories/{id}", tag = "categories", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = NewCategory,
    responses((status = OK, body = Category, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json")))]
async fn update(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<NewCategory>,
) -> Result<Response, Problem> {
    let name = valid_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let before = store::categories::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    let row = store::categories::upsert(&mut *tx, id, name)
        .await
        .map_err(|e| write_error(e, || format!("a category named {name:?} already exists")))?;
    let (before, after) = (Category::from(before), Category::from(row));
    audit(
        &mut tx,
        principal,
        Action::Update,
        "category",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a category nothing refers to.
#[utoipa::path(delete, path = "/categories/{id}", tag = "categories", security(("bearer" = [])),
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
    let before = store::categories::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    store::categories::delete(&mut *tx, id)
        .await
        .map_err(delete_error)?;
    let before = Category::from(before);
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "category",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
