//! `/api/v1/tags` (design §3.1). Free-form labels on transactions.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::audit::Action;
use store::tags::TagRow;
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
pub struct Tag {
    id: Uuid,
    name: String,
    #[serde(skip)]
    version: i64,
}

impl From<TagRow> for Tag {
    fn from(r: TagRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            version: r.version,
        }
    }
}

fn respond(status: StatusCode, item: &Tag) -> Response {
    (status, etag(item.version), axum::Json(item)).into_response()
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewTag {
    name: String,
}

/// List tags.
#[utoipa::path(get, path = "/tags", tag = "tags", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<Tag>)))]
async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<Tag>>, Problem> {
    let after = id_cursor(page.cursor.as_deref())?;
    let rows = store::tags::list(&state.pool, after, page.limit() + 1).await?;
    let items = rows.into_iter().map(Tag::from).collect();
    Ok(axum::Json(Page::from_rows(items, page.limit(), |c| {
        c.id.to_string()
    })))
}

/// Get one tag.
#[utoipa::path(get, path = "/tags/{id}", tag = "tags", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Tag, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::tags::get(&state.pool, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &Tag::from(row)))
}

/// Create a tag.
#[utoipa::path(post, path = "/tags", tag = "tags", security(("bearer" = [])),
    request_body = NewTag,
    responses((status = CREATED, body = Tag, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewTag>,
) -> Result<Response, Problem> {
    let name = valid_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let row = store::tags::upsert(&mut *tx, Uuid::now_v7(), name)
        .await
        .map_err(|e| write_error(e, || format!("a tag named {name:?} already exists")))?;
    let item = Tag::from(row);
    audit(
        &mut tx,
        principal,
        Action::Create,
        "tag",
        item.id,
        None,
        Some(&item),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &item);
    if let Ok(loc) = format!("/api/v1/tags/{}", item.id).parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Rename a tag.
#[utoipa::path(patch, path = "/tags/{id}", tag = "tags", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = NewTag,
    responses((status = OK, body = Tag, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json")))]
async fn update(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<NewTag>,
) -> Result<Response, Problem> {
    let name = valid_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let before = store::tags::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    let row = store::tags::upsert(&mut *tx, id, name)
        .await
        .map_err(|e| write_error(e, || format!("a tag named {name:?} already exists")))?;
    let (before, after) = (Tag::from(before), Tag::from(row));
    audit(
        &mut tx,
        principal,
        Action::Update,
        "tag",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a tag nothing refers to.
#[utoipa::path(delete, path = "/tags/{id}", tag = "tags", security(("bearer" = [])),
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
    let before = store::tags::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    store::tags::delete(&mut *tx, id)
        .await
        .map_err(delete_error)?;
    let before = Tag::from(before);
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "tag",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
