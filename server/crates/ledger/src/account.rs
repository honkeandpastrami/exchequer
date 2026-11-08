//! Accounts (yours) and counterparties (everyone else), design §3.1.

use crate::ids::{AccountId, CounterpartyId, PartyId};

/// Affects display only (sign convention, icon); never the kind of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Brokerage,
    Mortgage,
    Cash,
    Other,
}

/// Something of yours that holds value, in any number of commodities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub kind: AccountKind,
    /// Who the account belongs to (design §4.1).
    pub owner: PartyId,
    /// Checking, savings, cards and cash are liquid; brokerages are not.
    /// Activity inside a non-liquid account is never shared (design §4.1).
    pub liquid: bool,
}

/// Counterparties the system creates itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemCounterparty {
    OpeningBalances,
    Reconciliation,
}

/// Anyone outside the household. Has no direction: the same store is where
/// you spend and who refunds you.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterparty {
    pub id: CounterpartyId,
    pub name: String,
    pub system: Option<SystemCounterparty>,
}
