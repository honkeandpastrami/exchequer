//! The ledger core of Exchequer (design §3): exact numbers, commodities,
//! parties and the settlement tree, accounts, and transactions whose kind is
//! derived from who is on each side. Pure logic: no I/O, no database.

pub mod account;
pub mod account_number;
pub mod amount;
pub mod commodity;
pub mod ids;
pub mod party;
pub mod transaction;

pub use account::{Account, AccountKind, Counterparty, SystemCounterparty};
pub use account_number::AccountNumber;
pub use amount::{Amount, NumberError, Rate};
pub use commodity::{Commodity, CommodityKind};
pub use ids::*;
pub use party::{Party, PartyKind, SettlementTree};
pub use transaction::{Classified, Kind, Leg, Lookup, Side, Split, SplitError, Transaction};
