//! `/api/v1/transactions` (design §3). A transaction is created and replaced
//! whole, splits included. Every split is checked by `domain` before it's
//! stored, and every response carries each split's derived `kind`.

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use domain::transaction::TransactionError;
use domain::{
    AccountId, Amount, Book, CategoryId, Classified, CommodityId, CounterpartyId, Leg as DomainLeg,
    Month, PartyId, Side, Split as DomainSplit, TransactionId,
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use store::audit::Action;
use store::transactions::{Filter, Full, SplitRow, TransactionRow};
use time::Date;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::{audit, delete_error, write_error};
use crate::conventions::{IfMatch, Page, etag};
use crate::ledger::book_for;
use crate::problem::{Json, Path, Problem, Query};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, replace, delete))
}

const MAX_SPLITS: usize = 200;

// ------------------------------------------------------------- Schemas ----

/// One side of a split: `account_id` (yours) or `counterparty_id` (outside
/// the household). With neither, it's outside the household, unknown who.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Leg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    counterparty_id: Option<Uuid>,
    commodity_id: Uuid,
    /// Positive decimal string, e.g. `"80.00"`.
    #[schema(value_type = String, pattern = r"^[0-9]+(\.[0-9]+)?$", example = "80.00")]
    quantity: Amount,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewSplit {
    from: Leg,
    to: Leg,
    /// Who the money was for. Only on income/expense of liquid accounts and
    /// on liquidations.
    purpose_id: Option<Uuid>,
    /// Only on income, expenses and liquidations.
    category_id: Option<Uuid>,
    description: Option<String>,
    /// Settlement payments only: the month (`YYYY-MM`) this payment settles,
    /// when not the month before it's made.
    #[schema(value_type = Option<String>, pattern = r"^[0-9]{4}-[0-9]{2}$", example = "2026-06")]
    settles: Option<Month>,
}

/// Derived from who is on each side; never stored or entered.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Expense,
    Income,
    Transfer,
    Conversion,
}

impl From<domain::Kind> for Kind {
    fn from(k: domain::Kind) -> Self {
        match k {
            domain::Kind::Expense => Self::Expense,
            domain::Kind::Income => Self::Income,
            domain::Kind::Transfer => Self::Transfer,
            domain::Kind::Conversion => Self::Conversion,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Split {
    from: Leg,
    to: Leg,
    #[serde(skip_serializing_if = "Option::is_none")]
    purpose_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, pattern = r"^[0-9]{4}-[0-9]{2}$", example = "2026-06")]
    settles: Option<Month>,
    /// Read-only.
    kind: Kind,
    /// Read-only: a transfer from an investment account into a liquid one.
    liquidation: bool,
}

/// Where a transaction came from outside Exchequer, such as its ID in the
/// system it was migrated from. Unique per `source`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalRef {
    /// Lowercase letters, digits, `_` or `-`; e.g. `firefly`.
    #[schema(pattern = r"^[a-z0-9_-]{1,32}$", example = "firefly")]
    source: String,
    /// 1-200 characters.
    #[serde(rename = "ref")]
    #[schema(example = "1234")]
    reference: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewTransaction {
    date: Date,
    description: String,
    notes: Option<String>,
    /// Tag names; unknown ones are created.
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    external_refs: Vec<ExternalRef>,
    /// On create: a migration batch to add the transaction to (see
    /// `POST /import-batches`); omitted, the transaction gets a batch of its
    /// own. On replace: omitted or unchanged, since a transaction never
    /// leaves the batch it entered with.
    import_batch_id: Option<Uuid>,
    splits: Vec<NewSplit>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Transaction {
    id: Uuid,
    date: Date,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    tags: Vec<String>,
    external_refs: Vec<ExternalRef>,
    /// Read-only: the import batch the transaction entered with.
    import_batch_id: Uuid,
    splits: Vec<Split>,
    #[serde(skip)]
    version: i64,
}

fn respond(status: StatusCode, t: &Transaction) -> Response {
    (status, etag(t.version), axum::Json(t)).into_response()
}

// ---------------------------------------------------------- Conversion ----

fn side(leg: &Leg, which: &str, index: usize) -> Result<Side, Problem> {
    match (leg.account_id, leg.counterparty_id) {
        (Some(a), None) => Ok(Side::Account(AccountId::from_uuid(a))),
        (None, c) => Ok(Side::Counterparty(c.map(CounterpartyId::from_uuid))),
        (Some(_), Some(_)) => Err(Problem::invalid(format!(
            "split {index}: `{which}` has both account_id and counterparty_id"
        ))),
    }
}

fn domain_leg(leg: &Leg, which: &str, index: usize) -> Result<DomainLeg, Problem> {
    Ok(DomainLeg {
        side: side(leg, which, index)?,
        commodity: CommodityId::from_uuid(leg.commodity_id),
        quantity: leg.quantity,
    })
}

pub(crate) fn domain_split(s: &NewSplit, index: usize) -> Result<DomainSplit, Problem> {
    Ok(DomainSplit {
        from: domain_leg(&s.from, "from", index)?,
        to: domain_leg(&s.to, "to", index)?,
        purpose: s.purpose_id.map(PartyId::from_uuid),
        category: s.category_id.map(CategoryId::from_uuid),
        bill: None,
        description: s.description.clone(),
        settles: s.settles,
    })
}

fn leg_from_row(
    account: Option<Uuid>,
    counterparty: Option<Uuid>,
    commodity: Uuid,
    qty: rust_decimal::Decimal,
) -> Leg {
    Leg {
        account_id: account,
        counterparty_id: counterparty,
        commodity_id: commodity,
        quantity: Amount::from_decimal(qty),
    }
}

/// The request form of a stored split (to re-check it with `domain`).
pub(crate) fn new_split_from_row(r: &SplitRow) -> NewSplit {
    NewSplit {
        from: leg_from_row(
            r.from_account_id,
            r.from_counterparty_id,
            r.from_commodity_id,
            r.from_quantity,
        ),
        to: leg_from_row(
            r.to_account_id,
            r.to_counterparty_id,
            r.to_commodity_id,
            r.to_quantity,
        ),
        purpose_id: r.purpose_id,
        category_id: r.category_id,
        description: r.description.clone(),
        settles: r.settles.map(Month::of),
    }
}

fn check_count(splits: &[NewSplit]) -> Result<(), Problem> {
    if splits.is_empty() {
        return Err(Problem::invalid("a transaction needs at least one split"));
    }
    if splits.len() > MAX_SPLITS {
        return Err(Problem::invalid(format!("at most {MAX_SPLITS} splits")));
    }
    Ok(())
}

fn domain_splits(splits: &[NewSplit]) -> Result<Vec<DomainSplit>, Problem> {
    splits
        .iter()
        .enumerate()
        .map(|(i, s)| domain_split(s, i))
        .collect()
}

/// Classifies splits with `domain`, against entities already loaded into
/// `book`. Errors name the split and the rule.
fn classify_with(book: &Book, splits: Vec<DomainSplit>) -> Result<Vec<Classified>, Problem> {
    let tx = domain::Transaction {
        id: TransactionId::new(),
        date: Date::MIN,
        description: String::new(),
        notes: None,
        tags: vec![],
        splits,
    };
    tx.classify(book).map_err(|e| match e {
        TransactionError::NoSplits => Problem::invalid(e.to_string()),
        TransactionError::Split { index, error } => Problem::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ledger-rule",
            "Breaks a ledger rule",
        )
        .detail(format!("split {index}: {error}")),
    })
}

/// Checks a request's splits against the database.
async fn classify(db: &mut PgConnection, splits: &[NewSplit]) -> Result<Vec<Classified>, Problem> {
    check_count(splits)?;
    let splits = domain_splits(splits)?;
    let book = book_for(db, &splits).await?;
    classify_with(&book, splits)
}

fn output(
    id: Uuid,
    version: i64,
    batch: Uuid,
    req: NewTransaction,
    classified: Vec<Classified>,
) -> Transaction {
    Transaction {
        id,
        import_batch_id: batch,
        date: req.date,
        description: req.description,
        notes: req.notes,
        tags: req.tags,
        external_refs: req.external_refs,
        splits: req
            .splits
            .into_iter()
            .zip(classified)
            .map(|(s, c)| Split {
                from: s.from,
                to: s.to,
                purpose_id: s.purpose_id,
                category_id: s.category_id,
                description: s.description,
                settles: s.settles,
                kind: c.kind.into(),
                liquidation: c.liquidation,
            })
            .collect(),
        version,
    }
}

/// Rebuilds the API form of stored transactions, kinds included, loading
/// what all of them refer to in one go.
async fn from_full(db: &mut PgConnection, fulls: Vec<Full>) -> Result<Vec<Transaction>, Problem> {
    let requests: Vec<(TransactionRow, NewTransaction)> = fulls
        .into_iter()
        .map(|f| {
            let splits = f.splits.iter().map(new_split_from_row).collect();
            let req = NewTransaction {
                date: f.transaction.date,
                description: f.transaction.description.clone(),
                notes: f.transaction.notes.clone(),
                tags: f.tags,
                external_refs: f
                    .external_refs
                    .into_iter()
                    .map(|(source, reference)| ExternalRef { source, reference })
                    .collect(),
                import_batch_id: Some(f.transaction.import_batch_id),
                splits,
            };
            (f.transaction, req)
        })
        .collect();
    // Stored data passed these checks when written; failing now is a bug.
    let internal = |_| Problem::internal();
    let all: Vec<DomainSplit> = requests
        .iter()
        .map(|(_, r)| domain_splits(&r.splits))
        .collect::<Result<Vec<_>, _>>()
        .map_err(internal)?
        .into_iter()
        .flatten()
        .collect();
    let book = book_for(db, &all).await?;
    requests
        .into_iter()
        .map(|(t, req)| {
            let splits = domain_splits(&req.splits).map_err(internal)?;
            let classified = classify_with(&book, splits).map_err(internal)?;
            Ok(output(t.id, t.version, t.import_batch_id, req, classified))
        })
        .collect()
}

async fn from_one(db: &mut PgConnection, full: Full) -> Result<Transaction, Problem> {
    from_full(db, vec![full])
        .await?
        .pop()
        .ok_or_else(Problem::internal)
}

/// Validates the request's own fields and normalizes them.
fn normalize(mut req: NewTransaction) -> Result<NewTransaction, Problem> {
    req.description = req.description.trim().to_owned();
    if req.description.is_empty() || req.description.chars().count() > 500 {
        return Err(Problem::invalid("description must be 1-500 characters"));
    }
    let mut tags: Vec<String> = req.tags.iter().map(|t| t.trim().to_owned()).collect();
    if tags.iter().any(|t| t.is_empty() || t.chars().count() > 100) {
        return Err(Problem::invalid("tags must be 1-100 characters"));
    }
    tags.sort();
    tags.dedup();
    req.tags = tags;
    for r in &req.external_refs {
        let source_ok = (1..=32).contains(&r.source.len())
            && r.source
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !source_ok {
            return Err(Problem::invalid(
                "external_refs: source must be 1-32 of a-z, 0-9, _ or -",
            ));
        }
        if r.reference.is_empty() || r.reference.chars().count() > 200 {
            return Err(Problem::invalid(
                "external_refs: ref must be 1-200 characters",
            ));
        }
    }
    req.external_refs.sort();
    req.external_refs.dedup();
    Ok(req)
}

fn to_full(id: Uuid, batch: Uuid, req: &NewTransaction) -> Full {
    let leg = |l: &Leg| {
        (
            l.account_id,
            l.counterparty_id,
            l.commodity_id,
            l.quantity.as_decimal(),
        )
    };
    Full {
        transaction: TransactionRow {
            id,
            date: req.date,
            description: req.description.clone(),
            notes: req.notes.clone(),
            version: 0,
            import_batch_id: batch,
        },
        splits: req
            .splits
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let (fa, fc, fcom, fq) = leg(&s.from);
                let (ta, tc, tcom, tq) = leg(&s.to);
                SplitRow {
                    transaction_id: id,
                    position: i16::try_from(i).unwrap_or(i16::MAX),
                    from_account_id: fa,
                    from_counterparty_id: fc,
                    from_commodity_id: fcom,
                    from_quantity: fq,
                    to_account_id: ta,
                    to_counterparty_id: tc,
                    to_commodity_id: tcom,
                    to_quantity: tq,
                    purpose_id: s.purpose_id,
                    category_id: s.category_id,
                    description: s.description.clone(),
                    settles: s.settles.map(|m| m.first_day()),
                }
            })
            .collect(),
        tags: req.tags.clone(),
        external_refs: req
            .external_refs
            .iter()
            .map(|r| (r.source.clone(), r.reference.clone()))
            .collect(),
    }
}

/// Refuses a change to transactions dated in a closed month (design §4.4).
async fn check_open(db: &mut PgConnection, dates: &[Date]) -> Result<(), Problem> {
    match store::settlements::first_closed(&mut *db, dates).await? {
        Some(month) => Err(Problem::month_closed(Month::of(month))),
        None => Ok(()),
    }
}

/// The batch a new transaction joins: the migration batch it names, or a
/// new batch of its own.
async fn batch_for(
    db: &mut PgConnection,
    principal: Principal,
    requested: Option<Uuid>,
) -> Result<Uuid, Problem> {
    let Some(id) = requested else {
        let id = Uuid::now_v7();
        let token = principal.actor().map(|t| *t.as_uuid());
        store::import_batches::insert(&mut *db, id, "manual", None, token).await?;
        return Ok(id);
    };
    let batch = store::import_batches::get(&mut *db, id)
        .await?
        .ok_or_else(|| Problem::invalid(format!("unknown import batch {id}")))?;
    if batch.kind != "migration" {
        return Err(Problem::invalid(
            "only migration batches take new transactions",
        ));
    }
    Ok(id)
}

fn duplicate_ref() -> String {
    "another transaction already has one of these external_refs".to_owned()
}

// ------------------------------------------------------------ Handlers ----

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Earliest date, inclusive.
    from: Option<Date>,
    /// Latest date, inclusive.
    to: Option<Date>,
    /// Only transactions with a split on this account.
    account_id: Option<Uuid>,
    /// Only transactions with a split involving this counterparty.
    counterparty_id: Option<Uuid>,
    /// Only transactions with a split in this category.
    category_id: Option<Uuid>,
    /// Only transactions with this tag.
    tag_id: Option<Uuid>,
    /// Only transactions whose description, notes or a split's description
    /// contain this text (case-insensitive).
    q: Option<String>,
    /// With `external_ref`: the transaction with this external reference.
    external_source: Option<String>,
    /// With `external_source`.
    external_ref: Option<String>,
    /// Only transactions in this import batch.
    import_batch_id: Option<Uuid>,
    /// Items per page (default 100, max 500).
    limit: Option<u32>,
    /// `next_cursor` from the previous page. Opaque.
    cursor: Option<String>,
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

/// List transactions, newest first.
#[utoipa::path(get, path = "/transactions", tag = "transactions", security(("bearer" = [])),
    params(ListParams),
    responses((status = OK, body = Page<Transaction>)))]
async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<axum::Json<Page<Transaction>>, Problem> {
    let after = p.cursor.as_deref().map(parse_cursor).transpose()?;
    let limit = i64::from(p.limit.unwrap_or(100).clamp(1, 500));
    let filter = Filter {
        from: p.from,
        to: p.to,
        account: p.account_id,
        counterparty: p.counterparty_id,
        category: p.category_id,
        tag: p.tag_id,
        text: p.q.filter(|q| !q.trim().is_empty()),
        external_ref: match (p.external_source, p.external_ref) {
            (Some(s), Some(r)) => Some((s, r)),
            (None, None) => None,
            _ => {
                return Err(Problem::invalid(
                    "external_source and external_ref go together",
                ));
            }
        },
        import_batch: p.import_batch_id,
    };
    let mut conn = state.pool.acquire().await?;
    let rows = store::transactions::list(&mut *conn, &filter, after, limit + 1).await?;
    let fulls = store::transactions::complete(&mut conn, rows).await?;
    let items = from_full(&mut conn, fulls).await?;
    Ok(axum::Json(Page::from_rows(items, limit, |t| {
        format!("{}_{}", t.date, t.id)
    })))
}

/// Get one transaction.
#[utoipa::path(get, path = "/transactions/{id}", tag = "transactions", security(("bearer" = [])),
    params(("id" = Uuid, Path)),
    responses((status = OK, body = Transaction, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json")))]
async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Problem> {
    let mut conn = state.pool.acquire().await?;
    let full = store::transactions::get(&mut conn, id, false)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(respond(StatusCode::OK, &from_one(&mut conn, full).await?))
}

/// Create a transaction with its splits.
#[utoipa::path(post, path = "/transactions", tag = "transactions", security(("bearer" = [])),
    request_body = NewTransaction,
    responses((status = CREATED, body = Transaction, headers(("ETag" = String), ("Location" = String))),
              (status = CONFLICT, description = "Dated in a closed month, or a duplicate external ref", body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, description = "Invalid, or breaks a ledger rule", body = Problem, content_type = "application/problem+json")))]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewTransaction>,
) -> Result<Response, Problem> {
    let req = normalize(req)?;
    let id = Uuid::now_v7();
    let mut tx = state.pool.begin().await?;
    check_open(&mut tx, &[req.date]).await?;
    let classified = classify(&mut tx, &req.splits).await?;
    let batch = batch_for(&mut tx, principal, req.import_batch_id).await?;
    let version = store::transactions::upsert(&mut tx, &to_full(id, batch, &req))
        .await
        .map_err(|e| write_error(e, duplicate_ref))?;
    let created = output(id, version, batch, req, classified);
    audit(
        &mut tx,
        principal,
        Action::Create,
        "transaction",
        id,
        None,
        Some(&created),
    )
    .await?;
    tx.commit().await?;
    let mut res = respond(StatusCode::CREATED, &created);
    if let Ok(loc) = format!("/api/v1/transactions/{id}").parse() {
        res.headers_mut().insert(header::LOCATION, loc);
    }
    Ok(res)
}

/// Replace a transaction entirely, splits and tags included.
#[utoipa::path(put, path = "/transactions/{id}", tag = "transactions", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    request_body = NewTransaction,
    responses((status = OK, body = Transaction, headers(("ETag" = String))),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "Dated in a closed month, or a duplicate external ref", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_REQUIRED, body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, description = "Invalid, or breaks a ledger rule", body = Problem, content_type = "application/problem+json")))]
async fn replace(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
    Json(req): Json<NewTransaction>,
) -> Result<Response, Problem> {
    let req = normalize(req)?;
    let mut tx = state.pool.begin().await?;
    let before = store::transactions::get(&mut tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.transaction.version != version {
        return Err(Problem::stale());
    }
    let batch = before.transaction.import_batch_id;
    if req.import_batch_id.is_some_and(|b| b != batch) {
        return Err(Problem::invalid(
            "import_batch_id never changes: omit it or send the current one",
        ));
    }
    check_open(&mut tx, &[before.transaction.date, req.date]).await?;
    let before = from_one(&mut tx, before).await?;
    let classified = classify(&mut tx, &req.splits).await?;
    let new_version = store::transactions::upsert(&mut tx, &to_full(id, batch, &req))
        .await
        .map_err(|e| write_error(e, duplicate_ref))?;
    let after = output(id, new_version, batch, req, classified);
    audit(
        &mut tx,
        principal,
        Action::Update,
        "transaction",
        id,
        Some(&before),
        Some(&after),
    )
    .await?;
    tx.commit().await?;
    Ok(respond(StatusCode::OK, &after))
}

/// Delete a transaction.
#[utoipa::path(delete, path = "/transactions/{id}", tag = "transactions", security(("bearer" = [])),
    params(("id" = Uuid, Path), ("If-Match" = String, Header, description = "ETag from your last read")),
    responses((status = NO_CONTENT),
              (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
              (status = CONFLICT, description = "Dated in a closed month", body = Problem, content_type = "application/problem+json"),
              (status = PRECONDITION_FAILED, body = Problem, content_type = "application/problem+json")))]
async fn delete(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    IfMatch(version): IfMatch,
) -> Result<StatusCode, Problem> {
    let mut tx = state.pool.begin().await?;
    let current = store::transactions::get(&mut tx, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    if current.transaction.version != version {
        return Err(Problem::stale());
    }
    delete_one(&mut tx, principal, id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a transaction, if its month is open, and records it.
pub(crate) async fn delete_one(
    db: &mut PgConnection,
    principal: Principal,
    id: Uuid,
) -> Result<(), Problem> {
    let before = store::transactions::get(&mut *db, id, true)
        .await?
        .ok_or_else(Problem::not_found)?;
    check_open(&mut *db, &[before.transaction.date]).await?;
    let before = from_one(&mut *db, before).await?;
    store::transactions::delete(&mut *db, id)
        .await
        .map_err(delete_error)?;
    audit(
        db,
        principal,
        Action::Delete,
        "transaction",
        id,
        Some(&before),
        None,
    )
    .await
}
