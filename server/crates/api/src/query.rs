//! `POST /api/v1/query` (design §7): one aggregation over postings, filtered,
//! grouped and bucketed. A posting is one split's movement on one of your
//! accounts, signed (+ in, − out), carrying the split's derived kind and
//! (default) purpose. Rows are per commodity: nothing is valued yet (§3.4).
//!
//! Splits are classified by `domain`, as everywhere else, so a kind or a
//! default purpose here can't disagree with the ledger rules.

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::extract::State;
use domain::report::{self, Bucket, Dimension, Key, KindKey, Posting, Value};
use domain::settlement::SplitRef;
use domain::{AccountId, Amount, CommodityId, Kind, PartyId, TransactionId};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::Date;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::ledger::book_for;
use crate::problem::{Json, Problem};
use crate::transactions::{domain_split, new_split_from_row};

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
    /// The split's purpose, or its default (the account's owner for income
    /// and expenses on liquid accounts, the household for liquidations).
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
}

impl From<KindKey> for KindName {
    fn from(k: KindKey) -> Self {
        match k {
            KindKey::Expense => Self::Expense,
            KindKey::Income => Self::Income,
            KindKey::Transfer => Self::Transfer,
            KindKey::Conversion => Self::Conversion,
        }
    }
}

impl KindName {
    fn matches(self, k: Kind) -> bool {
        KindName::from(KindKey::from(k)) == self
    }
}

/// Filters combine with AND; the values inside one filter with OR.
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
    /// Flow only. Effective purpose, as in `group_by: purpose`.
    #[serde(default)]
    pub purpose_ids: Vec<Uuid>,
    /// Flow only.
    #[serde(default)]
    pub category_ids: Vec<Uuid>,
    /// Flow only.
    #[serde(default)]
    pub counterparty_ids: Vec<Uuid>,
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
        let flow_only = !req.purpose_ids.is_empty()
            || !req.category_ids.is_empty()
            || !req.counterparty_ids.is_empty()
            || !req.kinds.is_empty()
            || !req.tags.is_empty()
            || req.text.is_some();
        if flow_only {
            return Err(Problem::invalid(
                "a balance filters by account_ids and owner_ids only",
            ));
        }
    }
    let mut seen = HashSet::new();
    if !req.group_by.iter().all(|g| seen.insert(*g)) {
        return Err(Problem::invalid("group_by lists a field twice"));
    }
    Ok(())
}

/// The postings of every split dated `from`..=`to` that passes the filters.
async fn load_postings(
    db: &mut PgConnection,
    req: &QueryRequest,
    from: Option<Date>,
    to: Option<Date>,
) -> Result<Vec<Posting>, Problem> {
    let Some(household) = store::parties::household(&mut *db).await? else {
        return Ok(Vec::new());
    };
    let household = PartyId::from_uuid(household.id);
    let accounts = (!req.account_ids.is_empty()).then_some(req.account_ids.as_slice());
    let rows = store::transactions::splits_between(&mut *db, from, to, accounts).await?;

    let ids: Vec<Uuid> = rows
        .iter()
        .map(|r| r.split.transaction_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut tags: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (id, name) in store::transactions::tags_of(&mut *db, &ids).await? {
        tags.entry(id).or_default().push(name);
    }

    // Stored splits passed these checks when written; failing now is a bug.
    let splits = rows
        .iter()
        .map(|r| domain_split(&new_split_from_row(&r.split), 0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Problem::internal())?;
    let book = book_for(db, &splits).await?;

    let text = req.text.as_deref().map(str::to_lowercase);
    let has_text = |r: &store::transactions::QuerySplit| {
        text.as_deref().is_none_or(|t| {
            [
                Some(&r.description),
                r.notes.as_ref(),
                r.split.description.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|s| s.to_lowercase().contains(t))
        })
    };
    let none: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (row, split) in rows.iter().zip(&splits) {
        if !has_text(row) {
            continue;
        }
        let tx_tags = tags.get(&row.split.transaction_id).unwrap_or(&none);
        if !req.tags.is_empty() && !tx_tags.iter().any(|t| req.tags.contains(t)) {
            continue;
        }
        let classified = split.classify(&book).map_err(|_| Problem::internal())?;
        let source = SplitRef {
            transaction: TransactionId::from_uuid(row.split.transaction_id),
            position: usize::try_from(row.split.position).map_err(|_| Problem::internal())?,
        };
        for p in report::postings(
            source,
            row.date,
            tx_tags,
            split,
            &classified,
            &book,
            household,
        ) {
            if keep(req, &p) {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// The per-posting filters.
fn keep(req: &QueryRequest, p: &Posting) -> bool {
    let any =
        |ids: &[Uuid], id: Option<Uuid>| ids.is_empty() || id.is_some_and(|i| ids.contains(&i));
    any(&req.account_ids, Some(*p.account.as_uuid()))
        && any(&req.owner_ids, Some(*p.owner.as_uuid()))
        && any(&req.purpose_ids, p.purpose.map(|x| *x.as_uuid()))
        && any(&req.category_ids, p.category.map(|x| *x.as_uuid()))
        && any(&req.counterparty_ids, p.counterparty.map(|x| *x.as_uuid()))
        && (req.kinds.is_empty() || req.kinds.iter().any(|k| k.matches(p.kind)))
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
            && (req.owner_ids.is_empty() || req.owner_ids.contains(&owner));
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
            let postings = load_postings(db, req, req.from, req.to).await?;
            report::flows(&postings, &dims, bucket)
        }
        Measure::Balance => {
            let to = req.to.ok_or_else(Problem::internal)?;
            let from = req.from.unwrap_or(to);
            let start = bucket.map_or(from, |b| b.start(from));
            let postings = load_postings(db, req, Some(start), Some(to)).await?;
            let opening = opening(db, req, &dims, start).await?;
            report::balances(&opening, &postings, &dims, bucket, start, to)
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
