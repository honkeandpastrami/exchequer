//! `/api/v1/import-batches` (design §6.7). Every transaction belongs to the
//! batch it entered with: a migration, an uploaded file, or a transaction
//! entered by hand (a batch of its own). Undoing a batch deletes all of its
//! transactions in one go.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::audit::Action;
use store::import_batches::BatchRow;
use time::OffsetDateTime;
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
        .routes(routes!(get, undo))
}

// ------------------------------------------------------------- Schemas ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BatchKind {
    /// One transaction entered by hand.
    Manual,
    /// Transactions brought over from another system.
    Migration,
    /// Rows of an uploaded bank file.
    File,
}

impl BatchKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Migration => "migration",
            Self::File => "file",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        match s {
            "manual" => Ok(Self::Manual),
            "migration" => Ok(Self::Migration),
            "file" => Ok(Self::File),
            _ => Err(Problem::internal()),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewImportBatch {
    /// Only `migration`: manual batches come with each new transaction, and
    /// file batches from committing an import.
    kind: BatchKind,
    /// 1-200 characters, e.g. where the transactions came from.
    label: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportBatch {
    id: Uuid,
    kind: BatchKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    /// How many transactions are in it now.
    transaction_count: i64,
    #[serde(skip)]
    version: i64,
}

fn output(r: BatchRow) -> Result<ImportBatch, Problem> {
    Ok(ImportBatch {
        id: r.id,
        kind: BatchKind::parse(&r.kind)?,
        label: r.label,
        created_at: r.created_at,
        transaction_count: r.transaction_count,
        version: r.version,
    })
}

fn respond(status: StatusCode, b: &ImportBatch) -> Response {
    (status, etag(b.version), axum::Json(b)).into_response()
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Only batches of this kind.
    kind: Option<BatchKind>,
    /// Items per page (default 100, max 500).
    limit: Option<u32>,
    /// `next_cursor` from the previous page. Opaque.
    cursor: Option<String>,
}

// ------------------------------------------------------------ Handlers ----

/// List import batches, newest first.
#[utoipa::path(get, path = "/import-batches", tag = "import-batches", security(("bearer" = [])),
    params(ListParams),
    responses((status = OK, body = Page<ImportBatch>)))]
async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<axum::Json<Page<ImportBatch>>, Problem> {
    let after = p
        .cursor
        .as_deref()
        .map(|c| Uuid::try_parse(c).map_err(|_| Problem::invalid("invalid cursor")))
        .transpose()?;
    let limit = i64::from(p.limit.unwrap_or(100).clamp(1, 500));
    let rows = store::import_batches::list(
        &state.pool,
        None,
        p.kind.map(BatchKind::as_str),
        after,
        limit + 1,
    )
    .await?;
    let items = rows.into_iter().map(output).collect::<Result<_, _>>()?;
    Ok(axum::Json(Page::from_rows(items, limit, |b| {
        b.id.to_string()
    })))
}

/// Get one import batch.
#[utoipa::path(get, path = "/import-batches/{id}", tag = "import-batches", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = ImportBatch, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::import_batches::get(&state.pool, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &output(row)?))
}

/// Start a migration batch; transactions join it by naming it when they're
/// created.
#[utoipa::path(post, path = "/import-batches", tag = "import-batches", security(("bearer" = [])),
    request_body = NewImportBatch,
    responses((status = CREATED, body = ImportBatch, headers(("ETag" = String), ("Location" = String))),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewImportBatch>,
) -> Result<Response, Problem> {
    if req.kind != BatchKind::Migration {
        return Err(Problem::invalid(
            "only migration batches are created directly",
        ));
    }
    let label = req.label.as_deref().map(str::trim);
    if label.is_some_and(|l| l.is_empty() || l.chars().count() > 200) {
        return Err(Problem::invalid("label must be 1-200 characters"));
    }
    let id = Uuid::now_v7();
    let mut tx = state.pool.begin().await?;
    let token = principal.actor().map(|t| *t.as_uuid());
    store::import_batches::insert(&mut *tx, id, req.kind.as_str(), label, token).await?;
    let created = output(
        store::import_batches::get(&mut *tx, id)
            .await?
            .ok_or_else(Problem::internal)?,
    )?;
    audit(
        &mut tx,
        principal,
        Action::Create,
        "import_batch",
        id,
        None,
        Some(&created),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &created);
    if let Ok(loc) = format!("/api/v1/import-batches/{id}").parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Undo a batch: delete every transaction in it, and the batch. All or
/// nothing; refused if any of them is dated in a closed month.
#[utoipa::path(delete, path = "/import-batches/{id}", tag = "import-batches", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = NO_CONTENT),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "A transaction is dated in a closed month", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json")))]
async fn undo(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
) -> Result<StatusCode, Problem> {
    let mut tx = state.pool.begin().await?;
    let current = store::import_batches::lock(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current != version {
        return Err(Problem::stale());
    }
    let before = output(
        store::import_batches::get(&mut *tx, id)
            .await?
            .ok_or_else(Problem::internal)?,
    )?;
    for t in store::import_batches::transaction_ids(&mut *tx, id).await? {
        crate::transactions::delete_one(&mut tx, principal, t).await?;
    }
    store::import_batches::delete(&mut *tx, id).await?;
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "import_batch",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
