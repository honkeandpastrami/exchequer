//! Household settlement (design §4.2–§4.5): what a split means between
//! parties, and the monthly statement computed from that.
//!
//! Nothing here is stored. [`effect`] reads one classified split; the caller
//! turns each effect into an [`Item`], and [`Settlement::compute`] routes
//! every item across the settlement tree of its month. Statements are then
//! read off for any month, always from the current data (design §4.4).
//!
//! Signs: on the edge between a child and its parent, a positive amount goes
//! from the child to the parent (the child owes, or the child paid).

use std::collections::{BTreeMap, HashMap};

use time::Date;

use crate::account::Account;
use crate::amount::{Amount, NumberError};
use crate::ids::{CommodityId, PartyId, TransactionId};
use crate::month::Month;
use crate::party::{Party, PartyKind, SettlementTree, TreeError};
use crate::transaction::{Classified, Kind, Leg, Lookup, Side, Split};

/// One split: its transaction and its position there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SplitRef {
    pub transaction: TransactionId,
    pub position: usize,
}

/// What a split means for the settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// `debtor` owes `creditor` the split's value: an income or expense whose
    /// purpose isn't its account's owner, or a liquidation landing with
    /// someone other than its purpose (`liquidation`, shown "in transit").
    Claim {
        debtor: PartyId,
        creditor: PartyId,
        liquidation: bool,
    },
    /// A transfer between liquid accounts of different owners.
    Payment { payer: PartyId, payee: PartyId },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettlementError {
    /// Neither leg is in the household currency, and prices don't exist yet.
    #[error("no value in the household currency")]
    Unpriced,
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error(transparent)]
    Number(#[from] NumberError),
}

/// The split's settlement effect and its value in `currency`, or `None` when
/// it moves nothing between parties. `split` must be valid (`classified` is
/// its classification).
pub fn effect(
    split: &Split,
    classified: &Classified,
    lookup: &impl Lookup,
    household: PartyId,
    currency: CommodityId,
) -> Result<Option<(Effect, Amount)>, SettlementError> {
    let account = |leg: &Leg| match leg.side {
        Side::Account(id) => lookup.account(id),
        Side::Counterparty(_) => None,
    };
    let (from, to) = (account(&split.from), account(&split.to));

    let effect = match (classified.kind, from, to) {
        (Kind::Expense, Some(a), None) | (Kind::Income, None, Some(a)) if a.liquid => {
            let purpose = split.purpose.unwrap_or(a.owner);
            if purpose == a.owner {
                return Ok(None);
            }
            let (debtor, creditor) = if classified.kind == Kind::Expense {
                (purpose, a.owner)
            } else {
                (a.owner, purpose)
            };
            Effect::Claim {
                debtor,
                creditor,
                liquidation: false,
            }
        }
        (Kind::Transfer, Some(_), Some(b)) if classified.liquidation => {
            let purpose = split.purpose.unwrap_or(household);
            if purpose == b.owner {
                return Ok(None);
            }
            Effect::Claim {
                debtor: b.owner,
                creditor: purpose,
                liquidation: true,
            }
        }
        (Kind::Transfer, Some(a), Some(b)) if a.liquid && b.liquid && a.owner != b.owner => {
            Effect::Payment {
                payer: a.owner,
                payee: b.owner,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some((effect, value(split, from, currency)?)))
}

/// The split's value in `currency`: the leg on an account if it's in that
/// currency (what the account was charged), else the other leg.
fn value(
    split: &Split,
    from: Option<&Account>,
    currency: CommodityId,
) -> Result<Amount, SettlementError> {
    let (first, second) = if from.is_some() {
        (&split.from, &split.to)
    } else {
        (&split.to, &split.from)
    };
    [first, second]
        .into_iter()
        .find(|leg| leg.commodity == currency)
        .map(|leg| leg.quantity)
        .ok_or(SettlementError::Unpriced)
}

/// A split's effect, placed in the month it counts in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub source: SplitRef,
    pub date: Date,
    pub effect: Effect,
    /// Positive, in the household currency.
    pub amount: Amount,
    /// The month a payment settles, when set by hand. Claims always count in
    /// their own month; payments by default settle the month before the one
    /// they're made in (the statement for M is paid early in M + 1).
    pub settles: Option<Month>,
}

impl Item {
    /// The month whose `due` this item counts against.
    #[must_use]
    pub fn month(&self) -> Month {
        match self.effect {
            Effect::Claim { .. } => Month::of(self.date),
            Effect::Payment { .. } => self.settles.unwrap_or_else(|| default_settles(self.date)),
        }
    }
}

/// The month a payment made on `date` settles unless linked by hand.
#[must_use]
pub fn default_settles(date: Date) -> Month {
    Month::of(date).previous()
}

/// A month's parameters (design §4.3, step 3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonthParams {
    /// What the Household owes each person for the month.
    pub stipends: HashMap<PartyId, Amount>,
    /// Each business's funder. A business missing here is funded by the
    /// Household.
    pub funders: HashMap<PartyId, PartyId>,
}

/// How an item shows on an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Claim,
    /// A liquidation claim, kept apart from the regular due.
    InTransit,
    Payment,
    /// A payment matched to an in-transit liquidation, not a regular payment.
    Clearing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub source: SplitRef,
    pub date: Date,
    pub kind: LineKind,
    /// Positive: from the child towards the parent.
    pub amount: Amount,
}

/// A liquidation shown on its own line until a payment clears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InTransit {
    pub source: SplitRef,
    pub date: Date,
    /// Positive: the child holds proceeds that belong to the parent's side.
    pub amount: Amount,
    /// The payment that cleared it, if made by the end of the month.
    pub cleared_by: Option<SplitRef>,
}

/// One edge of the settlement tree for one month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub child: PartyId,
    pub parent: PartyId,
    /// Regular claims routed across the edge this month.
    pub claims: Amount,
    /// What the parent owes the child for the month as stipends.
    pub stipend: Amount,
    /// `claims − stipend`.
    pub due: Amount,
    /// Regular payments that settle this month.
    pub paid: Amount,
    /// Everything up to and including this month: claims and in-transit
    /// proceeds, minus stipends and every payment.
    pub outstanding: Amount,
    /// `outstanding` without the proceeds still in transit: the regular
    /// transfer to make.
    pub to_settle: Amount,
    /// In-transit liquidations from this month, and earlier ones not yet
    /// cleared by its start.
    pub in_transit: Vec<InTransit>,
    /// This month's items on the edge, in date order.
    pub lines: Vec<Line>,
}

impl Edge {
    /// `claims − paid`: what the stipend must have been if the month was
    /// settled exactly (design §4.5).
    pub fn implied_stipend(&self) -> Result<Amount, NumberError> {
        self.claims.checked_sub(self.paid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub month: Month,
    /// Where the month's parameters come from: the month itself, an earlier
    /// month they're carried forward from, or `None` (none set yet).
    pub params_from: Option<Month>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Default, Clone)]
struct Totals {
    parent: Option<PartyId>,
    claims: Amount,
    in_transit: Amount,
    stipend: Amount,
    paid: Amount,
    cleared: Amount,
    lines: Vec<Line>,
}

impl Totals {
    fn net(&self) -> Result<Amount, NumberError> {
        Amount::sum([
            self.claims,
            self.in_transit,
            -self.stipend,
            -self.paid,
            -self.cleared,
        ])
    }
}

/// Every item routed, ready to produce statements.
#[derive(Debug, Clone)]
pub struct Settlement {
    parties: Vec<Party>,
    children: Vec<PartyId>,
    params: BTreeMap<Month, MonthParams>,
    totals: BTreeMap<Month, HashMap<PartyId, Totals>>,
    /// Liquidations per edge, with the payment that cleared them (and when).
    liquidations: HashMap<PartyId, Vec<(Month, InTransit, Option<Date>)>>,
}

impl Settlement {
    /// Routes `items` across each month's tree, for every month from the
    /// first with data through `through` (stipends are due every month, with
    /// or without items). `params` holds the months that have parameters set;
    /// other months use the latest earlier ones.
    pub fn compute(
        parties: &[Party],
        params: &BTreeMap<Month, MonthParams>,
        items: &[Item],
        through: Month,
    ) -> Result<Self, SettlementError> {
        let household = SettlementTree::new(parties, &household_funded(parties, None))?.household();
        let mut s = Self {
            parties: parties.to_vec(),

            children: parties
                .iter()
                .filter(|p| p.kind != PartyKind::Household)
                .map(|p| p.id)
                .collect(),
            params: params.clone(),
            totals: BTreeMap::new(),
            liquidations: HashMap::new(),
        };

        let mut items: Vec<&Item> = items.iter().collect();
        items.sort_by_key(|i| (i.date, i.source));
        let clearing = match_clearing(&items);

        let mut bounds: Vec<Month> = items.iter().map(|i| i.month()).collect();
        bounds.extend(clearing.values().map(|(_, d)| Month::of(*d)));
        bounds.extend(params.keys().copied());
        bounds.push(through);
        let (first, last) = (bounds.iter().min(), bounds.iter().max());
        let mut months = Vec::new();
        if let (Some(&first), Some(&last)) = (first, last) {
            let mut m = first;
            while m <= last {
                months.push(m);
                m = m.next();
            }
        }

        let mut trees = HashMap::new();
        for &m in &months {
            let p = s.params_for(m).1.cloned();
            let tree = SettlementTree::new(
                parties,
                &household_funded(parties, p.as_ref().map(|p| &p.funders)),
            )?;
            let totals = s.totals.entry(m).or_default();
            for &c in &s.children {
                totals.entry(c).or_default().parent = tree.parent(c);
            }
            for (&person, &stipend) in p.iter().flat_map(|p| &p.stipends) {
                for (child, sign) in route(&tree, household, person)? {
                    let t = totals.entry(child).or_default();
                    t.stipend = t.stipend.checked_sub(signed(stipend, sign))?;
                }
            }
            trees.insert(m, tree);
        }

        for item in items {
            let (from, to) = match item.effect {
                Effect::Claim {
                    debtor, creditor, ..
                } => (debtor, creditor),
                Effect::Payment { payer, payee } => (payer, payee),
            };
            let (kind, month) = match item.effect {
                Effect::Claim {
                    liquidation: false, ..
                } => (LineKind::Claim, item.month()),
                Effect::Claim {
                    liquidation: true, ..
                } => (LineKind::InTransit, item.month()),
                Effect::Payment { .. } if clearing.values().any(|(p, _)| *p == item.source) => {
                    (LineKind::Clearing, Month::of(item.date))
                }
                Effect::Payment { .. } => (LineKind::Payment, item.month()),
            };
            for (child, sign) in route(&trees[&month], from, to)? {
                let amount = signed(item.amount, sign);
                let t = s.totals.entry(month).or_default().entry(child).or_default();
                let field = match kind {
                    LineKind::Claim => &mut t.claims,
                    LineKind::InTransit => &mut t.in_transit,
                    LineKind::Payment => &mut t.paid,
                    LineKind::Clearing => &mut t.cleared,
                };
                *field = field.checked_add(amount)?;
                t.lines.push(Line {
                    source: item.source,
                    date: item.date,
                    kind,
                    amount,
                });
                if kind == LineKind::InTransit {
                    let cleared = clearing.get(&item.source);
                    s.liquidations.entry(child).or_default().push((
                        month,
                        InTransit {
                            source: item.source,
                            date: item.date,
                            amount,
                            cleared_by: cleared.map(|(p, _)| *p),
                        },
                        cleared.map(|(_, d)| *d),
                    ));
                }
            }
        }
        Ok(s)
    }

    /// The month's parameters, and the month they were set for.
    fn params_for(&self, month: Month) -> (Option<Month>, Option<&MonthParams>) {
        self.params
            .range(..=month)
            .next_back()
            .map_or((None, None), |(m, p)| (Some(*m), Some(p)))
    }

    /// The statement for `month`, with `outstanding` counted from the first
    /// month with data.
    pub fn statement(&self, month: Month) -> Result<Statement, SettlementError> {
        let funders = household_funded(&self.parties, self.params_for(month).1.map(|p| &p.funders));
        let tree = SettlementTree::new(&self.parties, &funders)?;
        let empty = HashMap::new();
        let this = self.totals.get(&month).unwrap_or(&empty);
        let mut edges = Vec::new();
        for &child in &self.children {
            let mut outstanding = Amount::ZERO;
            for (_, totals) in self.totals.range(..=month) {
                if let Some(t) = totals.get(&child) {
                    outstanding = outstanding.checked_add(t.net()?)?;
                }
            }
            let mut in_transit = Vec::new();
            let mut open = Amount::ZERO;
            for (m, line, cleared_on) in self.liquidations.get(&child).into_iter().flatten() {
                if *m > month {
                    continue;
                }
                let cleared_by_end = cleared_on.is_some_and(|d| d <= month.last_day());
                if !cleared_by_end {
                    open = open.checked_add(line.amount)?;
                }
                let cleared_before = cleared_on.is_some_and(|d| d < month.first_day());
                if *m == month || !cleared_before {
                    let mut line = line.clone();
                    if !cleared_by_end {
                        line.cleared_by = None;
                    }
                    in_transit.push(line);
                }
            }
            let t = this.get(&child).cloned().unwrap_or_default();
            let mut lines = t.lines.clone();
            lines.sort_by_key(|l| (l.date, l.source));
            edges.push(Edge {
                child,
                parent: tree.parent(child).expect("every child has a parent"),
                claims: t.claims,
                stipend: t.stipend,
                due: t.claims.checked_sub(t.stipend)?,
                paid: t.paid,
                outstanding,
                to_settle: outstanding.checked_sub(open)?,
                in_transit,
                lines,
            });
        }
        Ok(Statement {
            month,
            params_from: self.params_for(month).0,
            edges,
        })
    }
}

/// The funders to build a tree with: the given ones, and the Household for
/// every other business.
fn household_funded(
    parties: &[Party],
    funders: Option<&HashMap<PartyId, PartyId>>,
) -> HashMap<PartyId, PartyId> {
    let household = parties
        .iter()
        .find(|p| p.kind == PartyKind::Household)
        .map(|p| p.id);
    parties
        .iter()
        .filter(|p| p.kind == PartyKind::Business)
        .filter_map(|p| {
            let f = funders.and_then(|f| f.get(&p.id).copied()).or(household)?;
            Some((p.id, f))
        })
        .collect()
}

/// The edges a claim or payment from `from` to `to` crosses, each with the
/// sign it has there (+1: towards the parent).
fn route(
    tree: &SettlementTree,
    from: PartyId,
    to: PartyId,
) -> Result<Vec<(PartyId, bool)>, TreeError> {
    Ok(tree
        .path(from, to)?
        .into_iter()
        .map(|step| (step.child, step.upward))
        .collect())
}

fn signed(amount: Amount, upward: bool) -> Amount {
    if upward { amount } else { -amount }
}

/// Pairs each in-transit liquidation with the first later payment that moves
/// exactly its amount from the same party to its purpose, and wasn't linked
/// to a month by hand (design §4.3). Returns liquidation → (payment, date).
fn match_clearing(items: &[&Item]) -> HashMap<SplitRef, (SplitRef, Date)> {
    let mut used = Vec::new();
    let mut out = HashMap::new();
    for liq in items {
        let Effect::Claim {
            debtor,
            creditor,
            liquidation: true,
        } = liq.effect
        else {
            continue;
        };
        let found = items.iter().find(|p| {
            p.date >= liq.date
                && p.settles.is_none()
                && p.amount == liq.amount
                && p.effect
                    == Effect::Payment {
                        payer: debtor,
                        payee: creditor,
                    }
                && !used.contains(&p.source)
        });
        if let Some(p) = found {
            used.push(p.source);
            out.insert(liq.source, (p.source, p.date));
        }
    }
    out
}
