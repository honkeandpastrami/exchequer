//! `/api/v1/parties` (design §4.1). A party's kind is fixed at creation;
//! its name can change, and the household's currency (the one settlement
//! values are in) can be set.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::audit::Action;
use store::parties::PartyRow;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::{audit, delete_error, id_cursor, nullable, valid_name, write_error};
use crate::conventions::{IfMatch, Page, PageParams, etag};
use crate::problem::{Json, Path, Problem, Query};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, update, delete))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PartyKind {
    Household,
    Person,
    Business,
}

impl PartyKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Household => "household",
            Self::Person => "person",
            Self::Business => "business",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        match s {
            "household" => Ok(Self::Household),
            "person" => Ok(Self::Person),
            "business" => Ok(Self::Business),
            _ => Err(Problem::internal()),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct Party {
    id: Uuid,
    name: String,
    kind: PartyKind,
    /// The household currency: what settlement statements are in. Only on
    /// the household.
    #[serde(skip_serializing_if = "Option::is_none")]
    currency_id: Option<Uuid>,
    #[serde(skip)]
    version: i64,
}

impl TryFrom<PartyRow> for Party {
    type Error = Problem;
    fn try_from(r: PartyRow) -> Result<Self, Problem> {
        Ok(Self {
            id: r.id,
            name: r.name,
            kind: PartyKind::parse(&r.kind)?,
            currency_id: r.currency_id,
            version: r.version,
        })
    }
}

/// The party with its ETag.
fn respond(status: StatusCode, party: &Party) -> Response {
    (status, etag(party.version), axum::Json(party)).into_response()
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewParty {
    name: String,
    kind: PartyKind,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PartyUpdate {
    name: Option<String>,
    /// Household only; `null` clears it.
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<Uuid>)]
    currency_id: Option<Option<Uuid>>,
}

/// List parties.
#[utoipa::path(get, path = "/parties", tag = "parties", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<Party>)))]
async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<Party>>, Problem> {
    let after = id_cursor(page.cursor.as_deref())?;
    let rows = store::parties::list(&state.pool, after, page.limit() + 1).await?;
    let parties = rows
        .into_iter()
        .map(Party::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(axum::Json(Page::from_rows(parties, page.limit(), |p| {
        p.id.to_string()
    })))
}

/// Get one party.
#[utoipa::path(get, path = "/parties/{id}", tag = "parties", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Party, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::parties::get(&state.pool, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &Party::try_from(row)?))
}

/// Create a party. There can be only one household.
#[utoipa::path(post, path = "/parties", tag = "parties", security(("bearer" = [])),
    request_body = NewParty,
    responses((status = CREATED, body = Party, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewParty>,
) -> Result<Response, Problem> {
    let name = valid_name(&req.name)?;
    let mut tx = state.pool.begin().await?;
    let row = store::parties::insert(&mut *tx, Uuid::now_v7(), name, req.kind.as_str())
        .await
        .map_err(|e| {
            write_error(e, || {
                if req.kind == PartyKind::Household {
                    "a household already exists, or the name is taken".to_owned()
                } else {
                    format!("a party named {name:?} already exists")
                }
            })
        })?;
    let party = Party::try_from(row)?;
    audit(
        &mut tx,
        principal,
        Action::Create,
        "party",
        party.id,
        None,
        Some(&party),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &party);
    if let Ok(loc) = format!("/api/v1/parties/{}", party.id).parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Rename a party, or set the household currency.
#[utoipa::path(patch, path = "/parties/{id}", tag = "parties", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = PartyUpdate,
    responses((status = OK, body = Party, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn update(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<PartyUpdate>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let before = store::parties::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    let before = Party::try_from(before)?;
    let name = match &req.name {
        Some(n) => valid_name(n)?,
        None => &before.name,
    };
    let currency_id = req.currency_id.unwrap_or(before.currency_id);
    if let Some(c) = currency_id {
        if before.kind != PartyKind::Household {
            return Err(Problem::invalid("only the household has a currency"));
        }
        if store::commodities::get(&mut *tx, c, false).await?.is_none() {
            return Err(Problem::invalid(format!("unknown commodity {c}")));
        }
    }
    let row = store::parties::update(&mut *tx, id, name, currency_id)
        .await
        .map_err(|e| write_error(e, || format!("a party named {name:?} already exists")))?;
    let after = Party::try_from(row)?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "party",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a party nothing refers to.
#[utoipa::path(delete, path = "/parties/{id}", tag = "parties", security(("bearer" = [])),
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
    let before = store::parties::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    store::parties::delete(&mut *tx, id)
        .await
        .map_err(delete_error)?;
    let before = Party::try_from(before)?;
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "party",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
