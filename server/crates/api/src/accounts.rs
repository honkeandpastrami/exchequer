//! `/api/v1/accounts` (design §3.1, §4.1): things of yours that hold value, in
//! any number of commodities. The owner is set once, at creation.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use domain::{AccountNumber, Amount};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use store::account_numbers::NumberRow;
use store::accounts::AccountRow;
use store::audit::Action;
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
        .routes(routes!(add_number))
        .routes(routes!(remove_number))
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

/// A credit card's limit: display metadata, never part of the balance.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreditLimit {
    commodity_id: Uuid,
    /// Positive decimal string, e.g. `"9000.00"`.
    #[schema(value_type = String, pattern = r"^[0-9]+(\.[0-9]+)?$", example = "9000.00")]
    quantity: Amount,
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
    /// Kept for history, hidden from everyday views.
    archived: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    credit_limit: Option<CreditLimit>,
    /// Numbers bank files name this account by, oldest first.
    numbers: Vec<Number>,
    #[serde(skip)]
    version: i64,
}

/// An account number, shown only by its last four characters.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Number {
    id: Uuid,
    #[schema(example = "6789")]
    last4: String,
}

impl Account {
    /// The account, with whichever of `numbers` are its own.
    fn from_row(r: AccountRow, numbers: &[NumberRow]) -> Result<Self, Problem> {
        let numbers = numbers
            .iter()
            .filter(|n| n.account_id == r.id)
            .map(|n| Number {
                id: n.id,
                last4: n.number.last4().to_owned(),
            })
            .collect();
        Ok(Self {
            numbers,
            id: r.id,
            name: r.name,
            kind: AccountKind::parse(&r.kind)?,
            owner_id: r.owner_id,
            liquid: r.liquid,
            archived: r.archived,
            credit_limit: match (r.credit_limit_commodity_id, r.credit_limit) {
                (Some(commodity_id), Some(q)) => Some(CreditLimit {
                    commodity_id,
                    quantity: Amount::from_decimal(q),
                }),
                (None, None) => None,
                _ => return Err(Problem::internal()),
            },
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
            archived: self.archived,
            credit_limit_commodity_id: self.credit_limit.as_ref().map(|c| c.commodity_id),
            credit_limit: self.credit_limit.as_ref().map(|c| c.quantity.as_decimal()),
            version: self.version,
        }
    }
}

/// The limit must be positive and fit its commodity's precision.
async fn check_credit_limit(
    db: &mut PgConnection,
    limit: Option<&CreditLimit>,
) -> Result<(), Problem> {
    let Some(limit) = limit else { return Ok(()) };
    if !limit.quantity.is_positive() {
        return Err(Problem::invalid("credit_limit must be positive"));
    }
    let commodity = store::commodities::get(&mut *db, limit.commodity_id, false)
        .await?
        .ok_or_else(|| Problem::invalid("credit_limit: unknown commodity"))?;
    if i64::from(limit.quantity.significant_scale()) > i64::from(commodity.precision) {
        return Err(Problem::invalid(format!(
            "credit_limit: {} has more decimals than {} allows ({})",
            limit.quantity, commodity.code, commodity.precision
        )));
    }
    Ok(())
}

/// The account with its numbers.
async fn load(db: &mut PgConnection, r: AccountRow) -> Result<Account, Problem> {
    let numbers = store::account_numbers::of(&mut *db, &[r.id]).await?;
    Account::from_row(r, &numbers)
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
    #[serde(default)]
    archived: bool,
    credit_limit: Option<CreditLimit>,
}

/// Fields to change; omitted ones stay as they are. The owner can't change.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccountUpdate {
    name: Option<String>,
    kind: Option<AccountKind>,
    liquid: Option<bool>,
    archived: Option<bool>,
    /// `null` removes the limit.
    #[serde(default, deserialize_with = "nullable")]
    #[schema(value_type = Option<CreditLimit>)]
    credit_limit: Option<Option<CreditLimit>>,
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
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let numbers = store::account_numbers::of(&state.pool, &ids).await?;
    let items = rows
        .into_iter()
        .map(|r| Account::from_row(r, &numbers))
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
    let mut db = state.pool.acquire().await?;
    let row = store::accounts::get(&mut *db, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &load(&mut db, row).await?))
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
        archived: req.archived,
        credit_limit: req.credit_limit,
        numbers: Vec::new(),
        version: 0,
    };
    let mut tx = state.pool.begin().await?;
    check_credit_limit(&mut tx, new.credit_limit.as_ref()).await?;
    let saved = store::accounts::upsert(&mut *tx, &new.row())
        .await
        .map_err(|e| {
            write_error(e, || {
                format!("the owner already has an account named {:?}", new.name)
            })
        })?;
    let item = Account::from_row(saved, &[])?;
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
    let before = load(&mut tx, before).await?;
    let changed = Account {
        id,
        numbers: before.numbers.clone(),
        owner_id: before.owner_id,
        version: before.version,
        name: match &req.name {
            Some(n) => valid_name(n)?.to_owned(),
            None => before.name.clone(),
        },
        kind: req.kind.unwrap_or(before.kind),
        liquid: req.liquid.unwrap_or(before.liquid),
        archived: req.archived.unwrap_or(before.archived),
        credit_limit: match req.credit_limit {
            Some(new) => new,
            None => before.credit_limit.clone(),
        },
    };
    check_credit_limit(&mut tx, changed.credit_limit.as_ref()).await?;
    let saved = store::accounts::upsert(&mut *tx, &changed.row())
        .await
        .map_err(|e| {
            write_error(e, || {
                format!("the owner already has an account named {:?}", changed.name)
            })
        })?;
    let after = load(&mut tx, saved).await?;
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
    let before = load(&mut tx, before).await?;
    store::accounts::delete(&mut *tx, id)
        .await
        .map_err(delete_error)?;
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

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewNumber {
    /// The full number, as a bank file shows it: letters and digits, up to
    /// 34; spaces and dashes are ignored. Never shown again in full.
    number: String,
}

/// Locks the account and checks it's the version the client read.
async fn lock_fresh(db: &mut PgConnection, id: Uuid, version: i64) -> Result<Account, Problem> {
    let row = store::accounts::get(&mut *db, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if row.version != version {
        return Err(Problem::stale());
    }
    load(db, row).await
}

/// Records a change to the account's numbers and returns it as it is now.
async fn numbers_changed(
    tx: &mut PgConnection,
    principal: Principal,
    before: &Account,
) -> Result<Response, Problem> {
    store::accounts::touch(&mut *tx, before.id).await?;
    let row = store::accounts::get(&mut *tx, before.id, false)
        .await?
        .ok_or_else(Problem::internal)?;
    let after = load(tx, row).await?;
    audit(
        tx,
        principal,
        Action::Update,
        "account",
        before.id,
        Some(before),
        Some(&after),
    )
    .await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Add a number bank files name this account by. A number belongs to one
/// account only; adding one the account already has changes nothing.
#[utoipa::path(post, path = "/accounts/{id}/numbers", tag = "accounts", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = NewNumber,
    responses((status = OK, body = Account, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "Another account has the number", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn add_number(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<NewNumber>,
) -> Result<Response, Problem> {
    let number = AccountNumber::new(&req.number)
        .filter(|n| {
            let n = n.reveal();
            n.len() <= 34 && n.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .ok_or_else(|| {
            Problem::invalid("number must be 1-34 letters and digits (spaces and dashes ignored)")
        })?;
    let mut tx = state.pool.begin().await?;
    let before = lock_fresh(&mut tx, id, version).await?;
    if let Some(owner) = store::account_numbers::find(&mut *tx, &number).await? {
        if owner.account_id == id {
            return Ok(respond(StatusCode::OK, &before));
        }
        let other = store::accounts::get(&mut *tx, owner.account_id, false)
            .await?
            .map_or_else(String::new, |a| format!(" {:?}", a.name));
        return Err(Problem::conflict(format!(
            "{number} already belongs to the account{other}"
        )));
    }
    store::account_numbers::insert(&mut *tx, Uuid::now_v7(), id, &number)
        .await
        .map_err(|e| write_error(e, || format!("{number} already belongs to another account")))?;
    let res = numbers_changed(&mut tx, principal, &before).await?;
    tx.commit().await?;
    Ok(res)
}

/// Remove one of the account's numbers.
#[utoipa::path(delete, path = "/accounts/{id}/numbers/{number_id}", tag = "accounts", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("number_id" = Uuid, Path),
           ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = OK, body = Account, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json")))]
async fn remove_number(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path((id, number_id)): Path<(Uuid, Uuid)>,
    IfMatch(version): IfMatch,
) -> Result<Response, Problem> {
    let mut tx = state.pool.begin().await?;
    let before = lock_fresh(&mut tx, id, version).await?;
    if !store::account_numbers::delete(&mut *tx, id, number_id).await? {
        return Err(Problem::not_found());
    }
    let res = numbers_changed(&mut tx, principal, &before).await?;
    tx.commit().await?;
    Ok(res)
}
