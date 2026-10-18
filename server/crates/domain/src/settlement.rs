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
    /// purpose isn't its liquid account's owner.
    Claim { debtor: PartyId, creditor: PartyId },
    /// A transfer between liquid accounts of different owners.
    Payment { payer: PartyId, payee: PartyId },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettlementError {
    /// Neither leg is in the settlement currency (USD).
    #[error("no value in the settlement currency (USD)")]
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
    currency: CommodityId,
) -> Result<Option<(Effect, Amount)>, SettlementError> {
    let Some(effect) = what(split, classified, lookup) else {
        return Ok(None);
    };
    Ok(Some((effect, value(split, lookup, currency)?)))
}

/// The split's settlement effect, or `None` when it moves nothing between
/// parties.
#[must_use]
pub fn what(split: &Split, classified: &Classified, lookup: &impl Lookup) -> Option<Effect> {
    let account = |leg: &Leg| match leg.side {
        Side::Account(id) => lookup.account(id),
        Side::Counterparty(_) => None,
    };
    let (from, to) = (account(&split.from), account(&split.to));

    Some(match (classified.kind, from, to) {
        // The liquid account: money left it (an expense) or arrived (income).
        (Kind::Expense, Some(a), _) | (Kind::Income, _, Some(a)) if a.liquid => {
            // A valid split says who it was for.
            let purpose = split.purpose?;
            if purpose == a.owner {
                return None;
            }
            let (debtor, creditor) = if classified.kind == Kind::Expense {
                (purpose, a.owner)
            } else {
                (a.owner, purpose)
            };
            Effect::Claim { debtor, creditor }
        }
        (Kind::Transfer, Some(a), Some(b)) if a.liquid && b.liquid && a.owner != b.owner => {
            Effect::Payment {
                payer: a.owner,
                payee: b.owner,
            }
        }
        _ => return None,
    })
}

/// The leg whose value counts: the one on an account (what the account was
/// charged), the `from` leg if both are.
#[must_use]
pub fn valued_leg<'a>(split: &'a Split, lookup: &impl Lookup) -> &'a Leg {
    match split.from.side {
        Side::Account(id) if lookup.account(id).is_some() => &split.from,
        _ => &split.to,
    }
}

/// The split's value in `currency`: the [`valued_leg`] if it's in that
/// currency, else the other leg. Neither is [`SettlementError::Unpriced`]:
/// the caller may value the [`valued_leg`] at a price.
pub fn value(
    split: &Split,
    lookup: &impl Lookup,
    currency: CommodityId,
) -> Result<Amount, SettlementError> {
    let first = valued_leg(split, lookup);
    let second = if std::ptr::eq(first, &split.from) {
        &split.to
    } else {
        &split.from
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
    /// Positive, in the settlement currency (USD).
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
    Payment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub source: SplitRef,
    pub date: Date,
    pub kind: LineKind,
    /// Positive: from the child towards the parent.
    pub amount: Amount,
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
    /// `outstanding` at the end of the previous month: past differences,
    /// including corrections to closed months, carried into this one.
    pub carried: Amount,
    /// Everything from the first month with parameters through this one:
    /// claims, minus stipends and every payment. What's left to settle.
    pub outstanding: Amount,
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
    stipend: Amount,
    paid: Amount,
    lines: Vec<Line>,
}

impl Totals {
    fn net(&self) -> Result<Amount, NumberError> {
        Amount::sum([self.claims, -self.stipend, -self.paid])
    }
}

/// Every item routed, ready to produce statements.
#[derive(Debug, Clone)]
pub struct Settlement {
    parties: Vec<Party>,
    children: Vec<PartyId>,
    params: BTreeMap<Month, MonthParams>,
    totals: BTreeMap<Month, HashMap<PartyId, Totals>>,
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
        };

        let mut items: Vec<&Item> = items.iter().collect();
        items.sort_by_key(|i| (i.date, i.source));

        let mut bounds: Vec<Month> = items.iter().map(|i| i.month()).collect();
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
            let (from, to, kind) = match item.effect {
                Effect::Claim { debtor, creditor } => (debtor, creditor, LineKind::Claim),
                Effect::Payment { payer, payee } => (payer, payee, LineKind::Payment),
            };
            let month = item.month();
            for (child, sign) in route(&trees[&month], from, to)? {
                let amount = signed(item.amount, sign);
                let t = s.totals.entry(month).or_default().entry(child).or_default();
                let field = match kind {
                    LineKind::Claim => &mut t.claims,
                    LineKind::Payment => &mut t.paid,
                };
                *field = field.checked_add(amount)?;
                t.lines.push(Line {
                    source: item.source,
                    date: item.date,
                    kind,
                    amount,
                });
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
        // Settlement starts with the first month that has parameters: earlier
        // months (e.g. before migrated history begins) aren't carried in.
        let start = self.params.keys().next().copied().filter(|s| *s <= month);
        let since = |m: Month| start.is_none_or(|s| m >= s);
        let mut edges = Vec::new();
        for &child in &self.children {
            let mut outstanding = Amount::ZERO;
            for (_, totals) in self.totals.range(..=month).filter(|(m, _)| since(**m)) {
                if let Some(t) = totals.get(&child) {
                    outstanding = outstanding.checked_add(t.net()?)?;
                }
            }
            let t = this.get(&child).cloned().unwrap_or_default();
            let carried = outstanding.checked_sub(t.net()?)?;
            let mut lines = t.lines.clone();
            lines.sort_by_key(|l| (l.date, l.source));
            edges.push(Edge {
                child,
                parent: tree.parent(child).expect("every child has a parent"),
                claims: t.claims,
                stipend: t.stipend,
                due: t.claims.checked_sub(t.stipend)?,
                paid: t.paid,
                carried,
                outstanding,
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
