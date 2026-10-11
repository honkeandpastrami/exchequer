//! `/api/v1/accounts` (design §3.1, §4.1): things of yours that hold value, in
//! any number of commodities. The owner is set once, at creation.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use store::accounts::AccountRow;
use store::audit::Action;
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

/// Affects display only (sign convention, icon), never a split's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Brokerage,
    Mortgage,
    Cash,
    Other,
}

impl AccountKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Checking => "checking",
            Self::Savings => "savings",
            Self::CreditCard => "credit_card",
            Self::Brokerage => "brokerage",
            Self::Mortgage => "mortgage",
            Self::Cash => "cash",
            Self::Other => "other",
        }
    }

    fn parse(s: &str) -> Result<Self, Problem> {
        Ok(match s {
            "checking" => Self::Checking,
            "savings" => Self::Savings,
            "credit_card" => Self::CreditCard,
            "brokerage" => Self::Brokerage,
            "mortgage" => Self::Mortgage,
            "cash" => Self::Cash,
            "other" => Self::Other,
            _ => return Err(Problem::internal()),
        })
    }
}

#[derive(Serialize, ToSchema)]
pub struct Account {
    id: Uuid,
    name: String,
    kind: AccountKind,
    /// The party the account belongs to.
    owner_id: Uuid,
    /// Investment accounts are not liquid; activity inside them is never
    /// shared (design §4.1).
    liquid: bool,
    #[serde(skip)]
    version: i64,
}

impl TryFrom<AccountRow> for Account {
    type Error = Problem;
    fn try_from(r: AccountRow) -> Result<Self, Problem> {
        Ok(Self {
            id: r.id,
            name: r.name,
            kind: AccountKind::parse(&r.kind)?,
            owner_id: r.owner_id,
            liquid: r.liquid,
            version: r.version,
        })
    }
}

impl Account {
    fn row(&self) -> AccountRow {
        AccountRow {
            id: self.id,
            name: self.name.clone(),
            kind: self.kind.as_str().to_owned(),
            owner_id: self.owner_id,
            liquid: self.liquid,
            version: self.version,
        }
    }
}

fn respond(status: StatusCode, item: &Account) -> Response {
    (status, etag(item.version), axum::Json(item)).into_response()
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewAccount {
    name: String,
    kind: AccountKind,
    owner_id: Uuid,
    /// Defaults to `false` for brokerage accounts, `true` otherwise.
    liquid: Option<bool>,
}

/// Fields to change; omitted ones stay as they are. The owner can't change.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccountUpdate {
    name: Option<String>,
    kind: Option<AccountKind>,
    liquid: Option<bool>,
}

/// List accounts.
#[utoipa::path(get, path = "/accounts", tag = "accounts", security(("bearer" = [])),
    params(PageParams),
    responses((status = OK, body = Page<Account>)))]
async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> Result<axum::Json<Page<Account>>, Problem> {
    let after = id_cursor(page.cursor.as_deref())?;
    let rows = store::accounts::list(&state.pool, after, page.limit() + 1).await?;
    let items = rows
        .into_iter()
        .map(Account::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(axum::Json(Page::from_rows(items, page.limit(), |c| {
        c.id.to_string()
    })))
}

/// Get one account.
#[utoipa::path(get, path = "/accounts/{id}", tag = "accounts", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Account, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let row = store::accounts::get(&state.pool, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &Account::try_from(row)?))
}

/// Create a account.
#[utoipa::path(post, path = "/accounts", tag = "accounts", security(("bearer" = [])),
    request_body = NewAccount,
    responses((status = CREATED, body = Account, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewAccount>,
) -> Result<Response, Problem> {
    let new = Account {
        id: Uuid::now_v7(),
        name: valid_name(&req.name)?.to_owned(),
        kind: req.kind,
        owner_id: req.owner_id,
        liquid: req.liquid.unwrap_or(req.kind != AccountKind::Brokerage),
        version: 0,
    };
    let mut tx = state.pool.begin().await?;
    let saved = store::accounts::upsert(&mut *tx, &new.row())
        .await
        .map_err(|e| {
            write_error(e, || {
                format!("an account named {:?} already exists", new.name)
            })
        })?;
    let item = Account::try_from(saved)?;
    audit(
        &mut tx,
        principal,
        Action::Create,
        "account",
        item.id,
        None,
        Some(&item),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &item);
    if let Ok(loc) = format!("/api/v1/accounts/{}", item.id).parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Change a account.
#[utoipa::path(patch, path = "/accounts/{id}", tag = "accounts", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = AccountUpdate,
    responses((status = OK, body = Account, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json")))]
async fn update(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<AccountUpdate>,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let before = store::accounts::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    let before = Account::try_from(before)?;
    let changed = Account {
        id,
        owner_id: before.owner_id,
        version: before.version,
        name: match &req.name {
            Some(n) => valid_name(n)?.to_owned(),
            None => before.name.clone(),
        },
        kind: req.kind.unwrap_or(before.kind),
        liquid: req.liquid.unwrap_or(before.liquid),
    };
    let saved = store::accounts::upsert(&mut *tx, &changed.row())
        .await
        .map_err(|e| {
            write_error(e, || {
                format!("an account named {:?} already exists", changed.name)
            })
        })?;
    let after = Account::try_from(saved)?;
    audit(
        &mut tx,
        principal,
        Action::Update,
        "account",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a account nothing refers to.
#[utoipa::path(delete, path = "/accounts/{id}", tag = "accounts", security(("bearer" = [])),
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
    let before = store::accounts::get(&mut *tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.version != version {
        return Err(Problem::stale());
    }
    store::accounts::delete(&mut *tx, id)
        .await
        .map_err(delete_error)?;
    let before = Account::try_from(before)?;
    audit(
        &mut tx,
        principal,
        Action::Delete,
        "account",
        id,
        Some(&before),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
