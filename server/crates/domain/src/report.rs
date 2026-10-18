//! The query engine's core (design §7): postings, grouping and buckets.
//!
//! Every split moves value out of one side and into the other. A *posting* is
//! one of those movements on one of your accounts, signed (+ in, − out) and
//! carrying the split's derived labels. Flows sum postings per group and
//! bucket; balances accumulate them. Amounts of different commodities are
//! never added: the commodity is always part of the group (valuation comes
//! with prices, §3.4).

use std::collections::{BTreeMap, BTreeSet};

use time::{Date, Duration, Month as CalendarMonth};

use crate::amount::{Amount, NumberError};
use crate::ids::{AccountId, CategoryId, CommodityId, CounterpartyId, PartyId};
use crate::settlement::SplitRef;
use crate::transaction::{Classified, Kind, Lookup, Side, Split};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posting {
    pub source: SplitRef,
    pub date: Date,
    pub account: AccountId,
    pub owner: PartyId,
    pub commodity: CommodityId,
    /// Positive into the account, negative out of it.
    pub quantity: Amount,
    /// The split's kind, except on the non-liquid side of a crossing split
    /// (money into or out of an investment or a loan): a transfer there, so
    /// the income or expense counts once, on the liquid side.
    pub kind: Kind,
    /// The split's purpose, or its default: the account's owner for income
    /// and expenses on liquid accounts, the household for crossing splits
    /// (design §4.1–§4.2). `None` where a purpose doesn't apply.
    pub purpose: Option<PartyId>,
    pub category: Option<CategoryId>,
    /// The outside party of an income or expense.
    pub counterparty: Option<CounterpartyId>,
    /// The transaction's tags.
    pub tags: Vec<String>,
}

/// The postings of one valid split: one per side that is an account.
pub fn postings(
    source: SplitRef,
    date: Date,
    tags: &[String],
    split: &Split,
    classified: &Classified,
    lookup: &impl Lookup,
    household: PartyId,
) -> Vec<Posting> {
    let counterparty = [&split.from, &split.to]
        .into_iter()
        .find_map(|l| match l.side {
            Side::Counterparty(c) => c,
            Side::Account(_) => None,
        });
    let mut out = Vec::with_capacity(2);
    for (leg, sign) in [(&split.from, false), (&split.to, true)] {
        let Side::Account(id) = leg.side else {
            continue;
        };
        let Some(account) = lookup.account(id) else {
            continue;
        };
        let flow = matches!(classified.kind, Kind::Expense | Kind::Income);
        let inside = classified.crossing && !account.liquid;
        let purpose = match () {
            () if inside || !flow || !account.liquid => None,
            () if classified.crossing => Some(split.purpose.unwrap_or(household)),
            () => Some(split.purpose.unwrap_or(account.owner)),
        };
        out.push(Posting {
            source,
            date,
            account: id,
            owner: account.owner,
            commodity: leg.commodity,
            quantity: if sign { leg.quantity } else { -leg.quantity },
            kind: if inside {
                Kind::Transfer
            } else {
                classified.kind
            },
            purpose,
            category: if inside { None } else { split.category },
            counterparty,
            tags: tags.to_vec(),
        });
    }
    out
}

/// What rows can be grouped by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dimension {
    Account,
    Owner,
    Purpose,
    Category,
    Counterparty,
    Kind,
    /// A posting with several tags counts in each of them.
    Tag,
}

impl Dimension {
    /// Balances only make sense per account or owner.
    #[must_use]
    pub const fn has_balance(self) -> bool {
        matches!(self, Self::Account | Self::Owner)
    }
}

/// One dimension's value in a row. `None` is "none" (e.g. no category).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    Account(AccountId),
    Party(Option<PartyId>),
    Category(Option<CategoryId>),
    Counterparty(Option<CounterpartyId>),
    Kind(KindKey),
    Tag(Option<String>),
}

/// `Kind`, ordered, for use as a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KindKey {
    Expense,
    Income,
    Transfer,
    Conversion,
    Adjustment,
}

impl From<Kind> for KindKey {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Expense => Self::Expense,
            Kind::Income => Self::Income,
            Kind::Transfer => Self::Transfer,
            Kind::Conversion => Self::Conversion,
            Kind::Adjustment => Self::Adjustment,
        }
    }
}

/// Time buckets; each is named by its first day. Weeks start on Monday.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl Bucket {
    /// The first day of the bucket `date` falls in.
    #[must_use]
    pub fn start(self, date: Date) -> Date {
        let month_start =
            |m: CalendarMonth| Date::from_calendar_date(date.year(), m, 1).expect("day 1 exists");
        match self {
            Self::Day => date,
            Self::Week => {
                date - Duration::days(i64::from(date.weekday().number_days_from_monday()))
            }
            Self::Month => month_start(date.month()),
            Self::Quarter => {
                let first = (u8::from(date.month()) - 1) / 3 * 3 + 1;
                month_start(CalendarMonth::try_from(first).expect("1, 4, 7 or 10"))
            }
            Self::Year => month_start(CalendarMonth::January),
        }
    }

    /// The first day of the next bucket after the one starting on `start`.
    #[must_use]
    pub fn next(self, start: Date) -> Date {
        let months = |n: u8| {
            let mut d = start;
            for _ in 0..n {
                let m = d.month();
                let y = if m == CalendarMonth::December {
                    d.year() + 1
                } else {
                    d.year()
                };
                d = Date::from_calendar_date(y, m.next(), 1).expect("day 1 exists");
            }
            d
        };
        match self {
            Self::Day => start + Duration::days(1),
            Self::Week => start + Duration::days(7),
            Self::Month => months(1),
            Self::Quarter => months(3),
            Self::Year => months(12),
        }
    }
}

/// A row's key: its bucket (if bucketed), its group values in the order
/// asked for, and the commodity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub bucket: Option<Date>,
    pub values: Vec<Value>,
    pub commodity: CommodityId,
}

fn values(p: &Posting, dims: &[Dimension]) -> Vec<Vec<Value>> {
    let mut rows: Vec<Vec<Value>> = vec![Vec::with_capacity(dims.len())];
    for d in dims {
        let options: Vec<Value> = match d {
            Dimension::Account => vec![Value::Account(p.account)],
            Dimension::Owner => vec![Value::Party(Some(p.owner))],
            Dimension::Purpose => vec![Value::Party(p.purpose)],
            Dimension::Category => vec![Value::Category(p.category)],
            Dimension::Counterparty => vec![Value::Counterparty(p.counterparty)],
            Dimension::Kind => vec![Value::Kind(p.kind.into())],
            Dimension::Tag if p.tags.is_empty() => vec![Value::Tag(None)],
            Dimension::Tag => {
                let unique: BTreeSet<&String> = p.tags.iter().collect();
                unique
                    .into_iter()
                    .map(|t| Value::Tag(Some(t.clone())))
                    .collect()
            }
        };
        rows = rows
            .into_iter()
            .flat_map(|r| {
                options.iter().map(move |o| {
                    let mut r = r.clone();
                    r.push(o.clone());
                    r
                })
            })
            .collect();
    }
    rows
}

/// Sums postings per group (and bucket). Rows that sum to zero are kept:
/// "nothing net" is an answer too.
pub fn flows(
    postings: &[Posting],
    dims: &[Dimension],
    bucket: Option<Bucket>,
) -> Result<BTreeMap<Key, Amount>, NumberError> {
    let mut out: BTreeMap<Key, Amount> = BTreeMap::new();
    for p in postings {
        for values in values(p, dims) {
            let key = Key {
                bucket: bucket.map(|b| b.start(p.date)),
                values,
                commodity: p.commodity,
            };
            let sum = out.entry(key).or_default();
            *sum = sum.checked_add(p.quantity)?;
        }
    }
    Ok(out)
}

/// Balances at the end of each bucket from `from` through `to` (or only at
/// `to` without a bucket). `postings` must include everything before `from`
/// that should count (or `opening` must hold it, per group). `dims` must all
/// have balances. Groups whose balance is zero throughout are left out.
pub fn balances(
    opening: &BTreeMap<(Vec<Value>, CommodityId), Amount>,
    postings: &[Posting],
    dims: &[Dimension],
    bucket: Option<Bucket>,
    from: Date,
    to: Date,
) -> Result<BTreeMap<Key, Amount>, NumberError> {
    debug_assert!(dims.iter().all(|d| d.has_balance()));
    // Bucket ends, as the start of each bucket mapped to its last day.
    let starts: Vec<Date> = match bucket {
        None => vec![to],
        Some(b) => {
            let mut v = Vec::new();
            let mut s = b.start(from);
            while s <= to {
                v.push(s);
                s = b.next(s);
            }
            v
        }
    };
    let end = |start: Date| match bucket {
        None => to,
        Some(b) => (b.next(start) - Duration::days(1)).min(to),
    };

    let mut running: BTreeMap<(Vec<Value>, CommodityId), Amount> = opening.clone();
    let mut sorted: Vec<&Posting> = postings.iter().filter(|p| p.date <= to).collect();
    sorted.sort_by_key(|p| p.date);
    let mut next = 0;
    let mut out = BTreeMap::new();
    for &start in &starts {
        let until = end(start);
        while next < sorted.len() && sorted[next].date <= until {
            let p = sorted[next];
            for values in values(p, dims) {
                let b = running.entry((values, p.commodity)).or_default();
                *b = b.checked_add(p.quantity)?;
            }
            next += 1;
        }
        for ((values, commodity), amount) in &running {
            out.insert(
                Key {
                    bucket: bucket.map(|_| start),
                    values: values.clone(),
                    commodity: *commodity,
                },
                *amount,
            );
        }
    }
    // Drop groups that are zero in every bucket.
    let nonzero: BTreeSet<(Vec<Value>, CommodityId)> = out
        .iter()
        .filter(|(_, a)| !a.is_zero())
        .map(|(k, _)| (k.values.clone(), k.commodity))
        .collect();
    out.retain(|k, _| nonzero.contains(&(k.values.clone(), k.commodity)));
    // A group is zero in the buckets before its first posting.
    for (values, commodity) in nonzero {
        for &start in &starts {
            out.entry(Key {
                bucket: bucket.map(|_| start),
                values: values.clone(),
                commodity,
            })
            .or_insert(Amount::ZERO);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;

    #[test]
    fn buckets() {
        let d = date!(2026 - 08 - 19); // a Wednesday
        assert_eq!(Bucket::Day.start(d), d);
        assert_eq!(Bucket::Week.start(d), date!(2026 - 08 - 17));
        assert_eq!(Bucket::Month.start(d), date!(2026 - 08 - 01));
        assert_eq!(Bucket::Quarter.start(d), date!(2026 - 07 - 01));
        assert_eq!(Bucket::Year.start(d), date!(2026 - 01 - 01));
        assert_eq!(
            Bucket::Quarter.next(date!(2026 - 10 - 01)),
            date!(2027 - 01 - 01)
        );
        assert_eq!(
            Bucket::Month.next(date!(2026 - 12 - 01)),
            date!(2027 - 01 - 01)
        );
        assert_eq!(
            Bucket::Week.next(date!(2026 - 08 - 17)),
            date!(2026 - 08 - 24)
        );
    }
}
