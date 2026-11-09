//! The filter shared by `/query`, the transactions list and the named
//! reports (design §7). An aggregate links to the transactions it was
//! computed from by handing over its filter: the list's `totals` are the
//! aggregate, since both come from the same postings.

use std::collections::{BTreeMap, HashMap, HashSet};

use ledger::query::{self, Filter, Posting};
use ledger::settlement::SplitRef;
use ledger::{
    AccountId, Amount, CategoryId, CommodityId, CounterpartyId, Month, PartyId, TransactionId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sqlx::PgConnection;
use time::Date;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::AppState;
use crate::ledger_from_store::book_for;
use crate::problem::Problem;
use crate::query::KindName;
use crate::settlements::{LineKind, compute};
use crate::transactions::{ledger_split, new_split_from_row};

/// An ID, or `none` for "no value" (no category, say).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdOrNone(pub Option<Uuid>);

impl Serialize for IdOrNone {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(id) => id.serialize(s),
            None => s.serialize_str("none"),
        }
    }
}

impl<'de> Deserialize<'de> for IdOrNone {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s == "none" {
            return Ok(Self(None));
        }
        Uuid::try_parse(&s)
            .map(|id| Self(Some(id)))
            .map_err(|_| serde::de::Error::custom(format!("`{s}` is neither an ID nor `none`")))
    }
}

/// Which transactions to list, and what of each counts. Filters combine with
/// AND; repeat a key for alternatives (`kind=income&kind=expense`). `none`
/// takes splits with no value there (`category_id=none`: uncategorized).
///
/// The amounts that count are the postings that pass, as `/query` sums them;
/// with `settles`, the settlement lines of one party's statement instead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct TransactionFilter {
    /// Earliest date, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Date>,
    /// Latest date, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Date>,
    /// On one of these accounts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub account_id: Vec<Uuid>,
    /// On an account one of these parties owns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owner_id: Vec<Uuid>,
    /// For one of these parties: the effective purpose, as `/query` has it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[param(value_type = Vec<String>)]
    #[schema(value_type = Vec<String>)]
    pub purpose_id: Vec<IdOrNone>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[param(value_type = Vec<String>)]
    #[schema(value_type = Vec<String>)]
    pub category_id: Vec<IdOrNone>,
    /// The outside party of an income or expense.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[param(value_type = Vec<String>)]
    #[schema(value_type = Vec<String>)]
    pub counterparty_id: Vec<IdOrNone>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kind: Vec<KindName>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commodity_id: Vec<Uuid>,
    /// Transactions with one of these tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_id: Vec<Uuid>,
    /// Text in the description or notes, or a split's description
    /// (case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// With `party_id`: the lines on that party's settlement statement for
    /// this month (`YYYY-MM`). A payment can settle a month other than its
    /// date's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[param(value_type = Option<String>, pattern = r"^[0-9]{4}-[0-9]{2}$")]
    #[schema(value_type = Option<String>, pattern = r"^[0-9]{4}-[0-9]{2}$")]
    pub settles: Option<Month>,
    /// With `settles`: the party whose statement it is (the child of its
    /// edge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party_id: Option<Uuid>,
    /// With `settles`: only its claims, or only its payments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineKind>,
    /// Splits the settlement couldn't value: no leg in USD and no price.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unpriced: bool,
}

impl TransactionFilter {
    fn check(&self) -> Result<(), Problem> {
        if let (Some(f), Some(t)) = (self.from, self.to)
            && f > t
        {
            return Err(Problem::invalid("`from` is after `to`"));
        }
        if self.settles.is_none() && (self.party_id.is_some() || self.line.is_some()) {
            return Err(Problem::invalid("`party_id` and `line` go with `settles`"));
        }
        if self.settles.is_some() && self.party_id.is_none() {
            return Err(Problem::invalid("`settles` needs `party_id`"));
        }
        if self.settles.is_some() && self.unpriced {
            return Err(Problem::invalid(
                "`settles` and `unpriced` don't go together",
            ));
        }
        Ok(())
    }

    /// The posting filter, tags looked up by name.
    async fn postings(&self, db: &mut PgConnection) -> Result<Filter, Problem> {
        let mut tags = Vec::new();
        for id in &self.tag_id {
            let tag = store::tags::get(&mut *db, *id, false)
                .await?
                .ok_or_else(|| Problem::invalid(format!("unknown tag {id}")))?;
            tags.push(tag.name);
        }
        let ids = |v: &[IdOrNone]| -> Vec<Option<Uuid>> { v.iter().map(|i| i.0).collect() };
        Ok(Filter {
            from: self.from,
            to: self.to,
            accounts: self
                .account_id
                .iter()
                .map(|i| AccountId::from_uuid(*i))
                .collect(),
            owners: self
                .owner_id
                .iter()
                .map(|i| PartyId::from_uuid(*i))
                .collect(),
            purposes: ids(&self.purpose_id)
                .into_iter()
                .map(|i| i.map(PartyId::from_uuid))
                .collect(),
            categories: ids(&self.category_id)
                .into_iter()
                .map(|i| i.map(CategoryId::from_uuid))
                .collect(),
            counterparties: ids(&self.counterparty_id)
                .into_iter()
                .map(|i| i.map(CounterpartyId::from_uuid))
                .collect(),
            kinds: self.kind.iter().map(|k| (*k).into()).collect(),
            commodities: self
                .commodity_id
                .iter()
                .map(|i| CommodityId::from_uuid(*i))
                .collect(),
            tags,
            text: self.q.clone().filter(|q| !q.trim().is_empty()),
        })
    }
}

/// The postings of every split that passes `filter`.
pub(crate) async fn load_postings(
    db: &mut PgConnection,
    filter: &Filter,
) -> Result<Vec<Posting>, Problem> {
    let accounts: Vec<Uuid> = filter.accounts.iter().map(|a| *a.as_uuid()).collect();
    let accounts = (!accounts.is_empty()).then_some(accounts.as_slice());
    let rows =
        store::transactions::splits_between(&mut *db, filter.from, filter.to, accounts).await?;

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
        .map(|r| ledger_split(&new_split_from_row(&r.split), 0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Problem::internal())?;
    let book = book_for(db, &splits).await?;

    let none: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (row, split) in rows.iter().zip(&splits) {
        let texts = [
            Some(&row.description),
            row.notes.as_ref(),
            row.split.description.as_ref(),
        ];
        if !filter.has_text(texts.into_iter().flatten().map(String::as_str)) {
            continue;
        }
        let tx_tags = tags.get(&row.split.transaction_id).unwrap_or(&none);
        let classified = split.classify(&book).map_err(|_| Problem::internal())?;
        let source = SplitRef {
            transaction: TransactionId::from_uuid(row.split.transaction_id),
            position: usize::try_from(row.split.position).map_err(|_| Problem::internal())?,
        };
        for p in query::postings(source, row.date, tx_tags, split, &classified, &book) {
            if filter.keeps(&p) {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// One transaction the filter matched.
pub(crate) struct Match {
    pub(crate) date: Date,
    pub(crate) id: Uuid,
    /// What its matching splits contributed, per commodity.
    pub(crate) amounts: BTreeMap<CommodityId, Amount>,
}

/// What a filter matched: newest first, and the totals over all of it.
pub(crate) struct Matched {
    pub(crate) transactions: Vec<Match>,
    pub(crate) totals: BTreeMap<CommodityId, Amount>,
}

/// The transactions `f` matches, each with what counted of it: the same
/// amounts `/query` (or, with `settles`, the statement) adds up.
pub(crate) async fn matching(
    state: &AppState,
    db: &mut PgConnection,
    f: &TransactionFilter,
) -> Result<Matched, Problem> {
    f.check()?;
    let filter = f.postings(db).await?;
    let postings = load_postings(db, &filter).await?;
    let mut counted: Vec<(Date, Uuid, CommodityId, Amount)> = Vec::new();
    if let (Some(month), Some(party)) = (f.settles, f.party_id) {
        // A line counts when its split passes the other filters too.
        let passed: HashSet<SplitRef> = postings.iter().map(|p| p.source).collect();
        let c = compute(&state.pool, db, &state.pricing, state.today(), month).await?;
        let statement = c
            .settlement
            .statement(month)
            .map_err(|_| Problem::internal())?;
        let lines = statement
            .edges
            .into_iter()
            .filter(|e| *e.child.as_uuid() == party)
            .flat_map(|e| e.lines);
        for l in lines {
            if f.line
                .is_some_and(|k| ledger::settlement::LineKind::from(k) != l.kind)
                || !passed.contains(&l.source)
            {
                continue;
            }
            counted.push((
                l.date,
                *l.source.transaction.as_uuid(),
                c.currency,
                l.amount,
            ));
        }
    } else {
        let unpriced = if f.unpriced {
            let through = f.to.map_or_else(|| Month::of(state.today()), Month::of);
            let c = compute(&state.pool, db, &state.pricing, state.today(), through).await?;
            Some(c.unpriced_splits())
        } else {
            None
        };
        for p in postings {
            if unpriced.as_ref().is_some_and(|u| !u.contains(&p.source)) {
                continue;
            }
            counted.push((
                p.date,
                *p.source.transaction.as_uuid(),
                p.commodity,
                p.quantity,
            ));
        }
    }

    let mut by_tx: BTreeMap<(Date, Uuid), BTreeMap<CommodityId, Amount>> = BTreeMap::new();
    let mut totals: BTreeMap<CommodityId, Amount> = BTreeMap::new();
    let add = |m: &mut BTreeMap<CommodityId, Amount>, c: CommodityId, a: Amount| {
        let sum = m.entry(c).or_default();
        *sum = sum.checked_add(a).map_err(|_| Problem::internal())?;
        Ok::<_, Problem>(())
    };
    for (date, id, commodity, amount) in counted {
        add(by_tx.entry((date, id)).or_default(), commodity, amount)?;
        add(&mut totals, commodity, amount)?;
    }
    Ok(Matched {
        transactions: by_tx
            .into_iter()
            .rev()
            .map(|((date, id), amounts)| Match { date, id, amounts })
            .collect(),
        totals,
    })
}
