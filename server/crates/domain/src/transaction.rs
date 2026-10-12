//! Transactions, splits, and the derived kind of each split (design §3.2, §3.3).

use time::Date;

use crate::account::{Account, Counterparty};
use crate::amount::{Amount, NumberError, Rate};
use crate::commodity::{Commodity, CommodityError};
use crate::ids::{
    AccountId, BillId, CategoryId, CommodityId, CounterpartyId, PartyId, TransactionId,
};
use crate::month::Month;

/// Where one side of a split is: one of your accounts, or someone outside
/// (`None` when it isn't known who).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Account(AccountId),
    Counterparty(Option<CounterpartyId>),
}

/// One side of a split: how much of which commodity leaves or arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leg {
    pub side: Side,
    pub commodity: CommodityId,
    /// Always positive; the direction comes from being the from- or to-leg.
    pub quantity: Amount,
}

/// One movement inside a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub from: Leg,
    pub to: Leg,
    /// Who the money was for. Only on income/expense of liquid accounts and
    /// on liquidations (design §4.1).
    pub purpose: Option<PartyId>,
    /// The class of an income or expense. Never on transfers or conversions.
    pub category: Option<CategoryId>,
    pub bill: Option<BillId>,
    pub description: Option<String>,
    /// For a settlement payment, the month it settles when that isn't the
    /// default (the month before it's made; design §4.3).
    pub settles: Option<Month>,
}

/// One real-world event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: TransactionId,
    pub date: Date,
    pub description: String,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub splits: Vec<Split>,
}

/// Computed from who is on each side of a split; never stored or entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Yours → external.
    Expense,
    /// External → yours.
    Income,
    /// Yours → yours, same commodity.
    Transfer,
    /// Yours → yours, different commodities (possibly within one account).
    Conversion,
}

/// `1 base = rate quote`, derived from a split whose legs differ in commodity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpliedPrice {
    pub base: CommodityId,
    pub quote: CommodityId,
    pub rate: Rate,
}

/// What the ledger derives from a valid split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub kind: Kind,
    /// A transfer from a non-liquid (investment) account into a liquid one:
    /// takes a purpose like income does (design §4.2).
    pub liquidation: bool,
    /// Present whenever the two legs are in different commodities.
    pub implied_price: Option<ImpliedPrice>,
}

/// Read access to the entities a split refers to.
pub trait Lookup {
    fn account(&self, id: AccountId) -> Option<&Account>;
    fn counterparty(&self, id: CounterpartyId) -> Option<&Counterparty>;
    fn commodity(&self, id: CommodityId) -> Option<&Commodity>;
    fn party_exists(&self, id: PartyId) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    #[error("unknown account {0}")]
    UnknownAccount(AccountId),
    #[error("unknown counterparty {0}")]
    UnknownCounterparty(CounterpartyId),
    #[error("unknown commodity {0}")]
    UnknownCommodity(CommodityId),
    #[error("unknown party {0}")]
    UnknownParty(PartyId),
    #[error("quantities must be positive, got {0}")]
    NotPositive(Amount),
    #[error(transparent)]
    Precision(#[from] CommodityError),
    #[error(transparent)]
    Number(#[from] NumberError),
    #[error("neither side is yours: the split doesn't involve the household")]
    BothExternal,
    #[error("moves a commodity from an account into the same account")]
    SameAccount,
    #[error("same commodity on both legs, but {from} out and {to} in")]
    UnequalQuantities { from: Amount, to: Amount },
    /// Purpose is only for income/expense of liquid accounts, and liquidations.
    #[error("this {0:?} can't have a purpose")]
    PurposeNotAllowed(Kind),
    #[error("a {0:?} can't have a category")]
    CategoryNotAllowed(Kind),
    /// Only transfers between liquid accounts of different owners settle a month.
    #[error("only a settlement payment can settle a month")]
    SettlesNotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransactionError {
    #[error("a transaction needs at least one split")]
    NoSplits,
    #[error("split {index}: {error}")]
    Split { index: usize, error: SplitError },
}

impl Split {
    /// Checks every ledger invariant and derives the split's kind.
    pub fn classify(&self, lookup: &impl Lookup) -> Result<Classified, SplitError> {
        let from = resolve(&self.from.side, lookup)?;
        let to = resolve(&self.to.side, lookup)?;
        check_leg(&self.from, lookup)?;
        check_leg(&self.to, lookup)?;

        let same_commodity = self.from.commodity == self.to.commodity;
        if same_commodity && self.from.quantity != self.to.quantity {
            return Err(SplitError::UnequalQuantities {
                from: self.from.quantity,
                to: self.to.quantity,
            });
        }

        let kind = match (from, to) {
            (None, None) => return Err(SplitError::BothExternal),
            (Some(_), None) => Kind::Expense,
            (None, Some(_)) => Kind::Income,
            (Some(a), Some(b)) if same_commodity && a.id == b.id => {
                return Err(SplitError::SameAccount);
            }
            (Some(_), Some(_)) if same_commodity => Kind::Transfer,
            (Some(_), Some(_)) => Kind::Conversion,
        };
        let liquidation = kind == Kind::Transfer
            && matches!((from, to), (Some(a), Some(b)) if !a.liquid && b.liquid);

        if let Some(party) = self.purpose {
            if !lookup.party_exists(party) {
                return Err(SplitError::UnknownParty(party));
            }
            // The one account on an income/expense split.
            let liquid = from.or(to).is_some_and(|a| a.liquid);
            let allowed = liquidation || (matches!(kind, Kind::Expense | Kind::Income) && liquid);
            if !allowed {
                return Err(SplitError::PurposeNotAllowed(kind));
            }
        }
        if self.category.is_some() && !(liquidation || matches!(kind, Kind::Expense | Kind::Income))
        {
            return Err(SplitError::CategoryNotAllowed(kind));
        }

        if self.settles.is_some() {
            let payment = kind == Kind::Transfer
                && matches!((from, to), (Some(a), Some(b)) if a.liquid && b.liquid && a.owner != b.owner);
            if !payment {
                return Err(SplitError::SettlesNotAllowed);
            }
        }

        let implied_price = if same_commodity {
            None
        } else {
            // Paid `from.quantity` of one commodity for `to.quantity` of another.
            Some(ImpliedPrice {
                base: self.to.commodity,
                quote: self.from.commodity,
                rate: Rate::implied(self.from.quantity, self.to.quantity)?,
            })
        };

        Ok(Classified {
            kind,
            liquidation,
            implied_price,
        })
    }
}

impl Transaction {
    /// Validates every split; returns their classifications in order.
    pub fn classify(&self, lookup: &impl Lookup) -> Result<Vec<Classified>, TransactionError> {
        if self.splits.is_empty() {
            return Err(TransactionError::NoSplits);
        }
        self.splits
            .iter()
            .enumerate()
            .map(|(index, s)| {
                s.classify(lookup)
                    .map_err(|error| TransactionError::Split { index, error })
            })
            .collect()
    }
}

/// The account behind a side, or `None` for someone outside (a known
/// counterparty, or an unknown one).
fn resolve<'a>(side: &Side, lookup: &'a impl Lookup) -> Result<Option<&'a Account>, SplitError> {
    match *side {
        Side::Account(id) => lookup
            .account(id)
            .map(Some)
            .ok_or(SplitError::UnknownAccount(id)),
        Side::Counterparty(None) => Ok(None),
        Side::Counterparty(Some(id)) => lookup
            .counterparty(id)
            .map(|_| None)
            .ok_or(SplitError::UnknownCounterparty(id)),
    }
}

fn check_leg(leg: &Leg, lookup: &impl Lookup) -> Result<(), SplitError> {
    let commodity = lookup
        .commodity(leg.commodity)
        .ok_or(SplitError::UnknownCommodity(leg.commodity))?;
    if !leg.quantity.is_positive() {
        return Err(SplitError::NotPositive(leg.quantity));
    }
    commodity.check(leg.quantity)?;
    Ok(())
}
