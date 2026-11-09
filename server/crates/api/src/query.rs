//! `POST /api/v1/query` (design §7): one aggregation over postings, filtered,
//! grouped and bucketed. A posting is one split's movement on one of your
//! accounts, signed (+ in, − out), carrying the split's derived kind and
//! (default) purpose. Rows are per commodity: nothing is valued yet (§3.4).
//!
//! Splits are classified by `ledger`, as everywhere else, so a kind or a
//! default purpose here can't disagree with the ledger rules.

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::extract::State;
use ledger::query::{self, Bucket, Dimension, Filter, Key, KindKey, Value};
use ledger::{AccountId, Amount, CategoryId, CommodityId, CounterpartyId, Kind, PartyId};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::Date;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::filter::load_postings;
use crate::problem::{Json, Problem};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(query))
}

/// The most rows one query returns; narrow it (dates, filters, bucket) past that.
const MAX_ROWS: usize = 20_000;

// ------------------------------------------------------------- Schemas ----

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Measure {
    /// Sum of postings dated in the range (or each bucket).
    #[default]
    Flow,
    /// Balance at the end of the range (or of each bucket).
    Balance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BucketName {
    Day,
    /// Weeks start on Monday.
    Week,
    Month,
    Quarter,
    Year,
}

impl From<BucketName> for Bucket {
    fn from(b: BucketName) -> Self {
        match b {
            BucketName::Day => Self::Day,
            BucketName::Week => Self::Week,
            BucketName::Month => Self::Month,
            BucketName::Quarter => Self::Quarter,
            BucketName::Year => Self::Year,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Account,
    Owner,
    /// The split's purpose: on income and expenses of liquid accounts.
    Purpose,
    Category,
    Counterparty,
    Kind,
    /// A transaction with several tags counts once per tag.
    Tag,
}

impl From<GroupBy> for Dimension {
    fn from(g: GroupBy) -> Self {
        match g {
            GroupBy::Account => Self::Account,
            GroupBy::Owner => Self::Owner,
            GroupBy::Purpose => Self::Purpose,
            GroupBy::Category => Self::Category,
            GroupBy::Counterparty => Self::Counterparty,
            GroupBy::Kind => Self::Kind,
            GroupBy::Tag => Self::Tag,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum KindName {
    Expense,
    Income,
    Transfer,
    Conversion,
    /// Opening balances and reconciliations: neither income nor spending.
    Adjustment,
}

impl From<KindKey> for KindName {
    fn from(k: KindKey) -> Self {
        match k {
            KindKey::Expense => Self::Expense,
            KindKey::Income => Self::Income,
            KindKey::Transfer => Self::Transfer,
            KindKey::Conversion => Self::Conversion,
            KindKey::Adjustment => Self::Adjustment,
        }
    }
}

impl From<KindName> for Kind {
    fn from(k: KindName) -> Self {
        match k {
            KindName::Expense => Self::Expense,
            KindName::Income => Self::Income,
            KindName::Transfer => Self::Transfer,
            KindName::Conversion => Self::Conversion,
            KindName::Adjustment => Self::Adjustment,
        }
    }
}

/// Filters combine with AND; the values inside one filter with OR. `null`
/// in a list takes postings with no value there (no category, say). The
/// transactions list takes the same filters (`GET /transactions`).
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryRequest {
    #[serde(default)]
    pub measure: Measure,
    /// First day, inclusive. For a balance, where buckets start (default: `to`).
    pub from: Option<Date>,
    /// Last day, inclusive. Required for a balance.
    pub to: Option<Date>,
    /// Split the range into buckets, each named by its first day.
    pub bucket: Option<BucketName>,
    /// Rows are always per commodity too. A balance groups by `account` or
    /// `owner` only.
    #[serde(default)]
    pub group_by: Vec<GroupBy>,
    #[serde(default)]
    pub account_ids: Vec<Uuid>,
    #[serde(default)]
    pub owner_ids: Vec<Uuid>,
    #[serde(default)]
    pub commodity_ids: Vec<Uuid>,
    /// Flow only. Effective purpose, as in `group_by: purpose`.
    #[serde(default)]
    pub purpose_ids: Vec<Option<Uuid>>,
    /// Flow only.
    #[serde(default)]
    pub category_ids: Vec<Option<Uuid>>,
    /// Flow only.
    #[serde(default)]
    pub counterparty_ids: Vec<Option<Uuid>>,
    /// Flow only.
    #[serde(default)]
    pub kinds: Vec<KindName>,
    /// Flow only: transactions with any of these tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Flow only: case-insensitive text in the description or notes.
    pub text: Option<String>,
}

/// One row. A grouped field is present (`null` for "none", e.g. no
/// category); a field not grouped by is absent.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Row {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<Date>,
    pub commodity_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Uuid>, nullable)]
    pub owner_id: Option<Option<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Uuid>, nullable)]
    pub purpose_id: Option<Option<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Uuid>, nullable)]
    pub category_id: Option<Option<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Uuid>, nullable)]
    pub counterparty_id: Option<Option<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<KindName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, nullable)]
    pub tag: Option<Option<String>>,
    /// At the commodity's precision. Positive: into your accounts.
    #[schema(value_type = String, example = "-80.00")]
    pub amount: Amount,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct QueryResult {
    pub rows: Vec<Row>,
}

// ---------------------------------------------------------- Execution ----

fn check(req: &QueryRequest) -> Result<(), Problem> {
    if let (Some(f), Some(t)) = (req.from, req.to)
        && f > t
    {
        return Err(Problem::invalid("`from` is after `to`"));
    }
    if req.measure == Measure::Balance {
        if req.to.is_none() {
            return Err(Problem::invalid("a balance needs `to`"));
        }
        if req
            .group_by
            .iter()
            .any(|g| !Dimension::from(*g).has_balance())
        {
            return Err(Problem::invalid(
                "a balance groups by account or owner only",
            ));
        }
        if !req.filter().by_holding_only() {
            return Err(Problem::invalid(
                "a balance filters by account_ids, owner_ids and commodity_ids only",
            ));
        }
    }
    let mut seen = HashSet::new();
    if !req.group_by.iter().all(|g| seen.insert(*g)) {
        return Err(Problem::invalid("group_by lists a field twice"));
    }
    Ok(())
}

impl QueryRequest {
    /// The posting filter (dates aside: a balance reads from further back).
    fn filter(&self) -> Filter {
        Filter {
            from: None,
            to: None,
            accounts: self
                .account_ids
                .iter()
                .map(|i| AccountId::from_uuid(*i))
                .collect(),
            owners: self
                .owner_ids
                .iter()
                .map(|i| PartyId::from_uuid(*i))
                .collect(),
            purposes: self
                .purpose_ids
                .iter()
                .map(|i| i.map(PartyId::from_uuid))
                .collect(),
            categories: self
                .category_ids
                .iter()
                .map(|i| i.map(CategoryId::from_uuid))
                .collect(),
            counterparties: self
                .counterparty_ids
                .iter()
                .map(|i| i.map(CounterpartyId::from_uuid))
                .collect(),
            kinds: self.kinds.iter().map(|k| (*k).into()).collect(),
            commodities: self
                .commodity_ids
                .iter()
                .map(|i| CommodityId::from_uuid(*i))
                .collect(),
            tags: self.tags.clone(),
            text: self.text.clone(),
        }
    }
}

/// Opening balances before `day`, per group, for a balance query.
async fn opening(
    db: &mut PgConnection,
    req: &QueryRequest,
    dims: &[Dimension],
    day: Date,
) -> Result<BTreeMap<(Vec<Value>, CommodityId), Amount>, Problem> {
    let rows = store::transactions::balances_before(&mut *db, day).await?;
    let ids: Vec<Uuid> = rows
        .iter()
        .map(|r| r.0)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let owners: HashMap<Uuid, Uuid> = store::accounts::get_many(&mut *db, &ids)
        .await?
        .into_iter()
        .map(|a| (a.id, a.owner_id))
        .collect();
    let mut out: BTreeMap<(Vec<Value>, CommodityId), Amount> = BTreeMap::new();
    for (account, commodity, balance) in rows {
        let owner = *owners.get(&account).ok_or_else(Problem::internal)?;
        let ok = (req.account_ids.is_empty() || req.account_ids.contains(&account))
            && (req.owner_ids.is_empty() || req.owner_ids.contains(&owner))
            && (req.commodity_ids.is_empty() || req.commodity_ids.contains(&commodity));
        if !ok {
            continue;
        }
        let values = dims
            .iter()
            .map(|d| match d {
                Dimension::Account => Value::Account(AccountId::from_uuid(account)),
                _ => Value::Party(Some(PartyId::from_uuid(owner))),
            })
            .collect();
        let sum = out
            .entry((values, CommodityId::from_uuid(commodity)))
            .or_default();
        *sum = sum
            .checked_add(Amount::from_decimal(balance))
            .map_err(|_| Problem::internal())?;
    }
    Ok(out)
}

/// Runs a query; the rows come in key order (bucket, groups, commodity).
pub async fn run(db: &mut PgConnection, req: &QueryRequest) -> Result<Vec<Row>, Problem> {
    check(req)?;
    let dims: Vec<Dimension> = req.group_by.iter().map(|g| (*g).into()).collect();
    let bucket = req.bucket.map(Bucket::from);
    let result = match req.measure {
        Measure::Flow => {
            let filter = Filter {
                from: req.from,
                to: req.to,
                ..req.filter()
            };
            let postings = load_postings(db, &filter).await?;
            query::flows(&postings, &dims, bucket)
        }
        Measure::Balance => {
            let to = req.to.ok_or_else(Problem::internal)?;
            let from = req.from.unwrap_or(to);
            let start = bucket.map_or(from, |b| b.start(from));
            let filter = Filter {
                from: Some(start),
                to: Some(to),
                ..req.filter()
            };
            let postings = load_postings(db, &filter).await?;
            let opening = opening(db, req, &dims, start).await?;
            query::balances(&opening, &postings, &dims, bucket, start, to)
        }
    }
    .map_err(|_| Problem::internal())?;
    if result.len() > MAX_ROWS {
        return Err(Problem::invalid(format!(
            "more than {MAX_ROWS} rows; narrow the dates or filters, or use a larger bucket"
        )));
    }

    let commodities: Vec<Uuid> = result
        .keys()
        .map(|k| *k.commodity.as_uuid())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let scale: HashMap<Uuid, u32> = store::commodities::get_many(&mut *db, &commodities)
        .await?
        .into_iter()
        .map(|c| (c.id, u32::try_from(c.precision).unwrap_or(0)))
        .collect();
    result
        .into_iter()
        .map(|(key, amount)| row(&req.group_by, key, amount, &scale))
        .collect()
}

fn row(
    group_by: &[GroupBy],
    key: Key,
    amount: Amount,
    scale: &HashMap<Uuid, u32>,
) -> Result<Row, Problem> {
    let commodity_id = *key.commodity.as_uuid();
    let mut d = amount.as_decimal();
    if d.is_zero() {
        d = rust_decimal::Decimal::ZERO;
    }
    d.rescale(*scale.get(&commodity_id).ok_or_else(Problem::internal)?);
    let mut r = Row {
        bucket: key.bucket,
        commodity_id,
        account_id: None,
        owner_id: None,
        purpose_id: None,
        category_id: None,
        counterparty_id: None,
        kind: None,
        tag: None,
        amount: Amount::from_decimal(d),
    };
    for (g, v) in group_by.iter().zip(key.values) {
        match (g, v) {
            (GroupBy::Account, Value::Account(a)) => r.account_id = Some(*a.as_uuid()),
            (GroupBy::Owner, Value::Party(p)) => r.owner_id = Some(p.map(|p| *p.as_uuid())),
            (GroupBy::Purpose, Value::Party(p)) => r.purpose_id = Some(p.map(|p| *p.as_uuid())),
            (GroupBy::Category, Value::Category(c)) => {
                r.category_id = Some(c.map(|c| *c.as_uuid()));
            }
            (GroupBy::Counterparty, Value::Counterparty(c)) => {
                r.counterparty_id = Some(c.map(|c| *c.as_uuid()));
            }
            (GroupBy::Kind, Value::Kind(k)) => r.kind = Some(k.into()),
            (GroupBy::Tag, Value::Tag(t)) => r.tag = Some(t),
            _ => return Err(Problem::internal()),
        }
    }
    Ok(r)
}

// ------------------------------------------------------------ Handlers ----

/// Aggregate postings: filter × group by × bucket (design §7). Read-only,
/// despite being a POST (the filters don't fit a query string).
#[utoipa::path(post, path = "/query", tag = "reports", security(("bearer" = [])),
    request_body = QueryRequest,
    responses((status = OK, body = QueryResult),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn query(
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> Result<axum::Json<QueryResult>, Problem> {
    let mut conn = state.pool.acquire().await?;
    Ok(axum::Json(QueryResult {
        rows: run(&mut conn, &req).await?,
    }))
}
