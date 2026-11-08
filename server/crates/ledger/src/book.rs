//! An in-memory [`Lookup`]: the entities a set of splits refers to, loaded
//! up front (from the database, or built by hand in tests).

use std::collections::HashMap;

use crate::account::{Account, Counterparty};
use crate::commodity::Commodity;
use crate::ids::{AccountId, CommodityId, CounterpartyId, PartyId};
use crate::party::Party;
use crate::transaction::Lookup;

#[derive(Debug, Default)]
pub struct Book {
    accounts: HashMap<AccountId, Account>,
    counterparties: HashMap<CounterpartyId, Counterparty>,
    commodities: HashMap<CommodityId, Commodity>,
    parties: HashMap<PartyId, Party>,
}

impl Book {
    pub fn add_account(&mut self, a: Account) {
        self.accounts.insert(a.id, a);
    }

    pub fn add_counterparty(&mut self, c: Counterparty) {
        self.counterparties.insert(c.id, c);
    }

    pub fn add_commodity(&mut self, c: Commodity) {
        self.commodities.insert(c.id, c);
    }

    pub fn add_party(&mut self, p: Party) {
        self.parties.insert(p.id, p);
    }
}

impl Lookup for Book {
    fn account(&self, id: AccountId) -> Option<&Account> {
        self.accounts.get(&id)
    }

    fn counterparty(&self, id: CounterpartyId) -> Option<&Counterparty> {
        self.counterparties.get(&id)
    }

    fn commodity(&self, id: CommodityId) -> Option<&Commodity> {
        self.commodities.get(&id)
    }

    fn party_exists(&self, id: PartyId) -> bool {
        self.parties.contains_key(&id)
    }
}
