//! The fictional household (Alex, Sam, Studio) shared by the settlement and
//! report tests.
#![allow(dead_code)] // each test file uses a different subset

use domain::settlement::{Effect, Item, MonthParams, SettlementError, SplitRef, effect};
use domain::*;
use time::Date;

pub struct H {
    pub book: Book,
    pub parties: Vec<Party>,
    pub household: PartyId,
    pub alex: PartyId,
    pub sam: PartyId,
    pub studio: PartyId,
    pub usd: CommodityId,
    pub eur: CommodityId,
    pub acme: CommodityId,
    pub alex_checking: AccountId,
    pub alex_credit: AccountId,
    pub alex_brokerage: AccountId,
    pub alex_eur: AccountId,
    pub sam_credit: AccountId,
    pub joint_savings: AccountId,
    pub mortgage: AccountId,
    pub store: CounterpartyId,
}

impl H {
    pub fn new() -> Self {
        let mut book = Book::default();
        let mut parties = Vec::new();
        let mut party = |name: &str, kind| {
            let p = Party {
                id: PartyId::new(),
                name: name.to_owned(),
                kind,
            };
            parties.push(p.clone());
            book.add_party(p.clone());
            p.id
        };
        let household = party("Household", PartyKind::Household);
        let alex = party("Alex", PartyKind::Person);
        let sam = party("Sam", PartyKind::Person);
        let studio = party("Studio", PartyKind::Business);

        let mut commodity = |code: &str, kind, precision| {
            let c = Commodity::new(CommodityId::new(), code, kind, precision).unwrap();
            let id = c.id;
            book.add_commodity(c);
            id
        };
        let usd = commodity("USD", CommodityKind::Fiat, 2);
        let eur = commodity("EUR", CommodityKind::Fiat, 2);
        let acme = commodity("ACME", CommodityKind::Security, 0);

        let mut account = |name: &str, kind, owner, liquid| {
            let a = Account {
                id: AccountId::new(),
                name: name.to_owned(),
                kind,
                owner,
                liquid,
            };
            let id = a.id;
            book.add_account(a);
            id
        };
        let alex_checking = account("Alex Checking", AccountKind::Checking, alex, true);
        let alex_credit = account("Alex Credit", AccountKind::CreditCard, alex, true);
        let alex_brokerage = account("Alex Brokerage", AccountKind::Brokerage, alex, false);
        let alex_eur = account("Alex EUR", AccountKind::Checking, alex, true);
        let sam_credit = account("Sam Credit", AccountKind::CreditCard, sam, true);
        let joint_savings = account("Joint Savings", AccountKind::Savings, household, true);
        let mortgage = account("Mortgage", AccountKind::Mortgage, household, true);

        let store = CounterpartyId::new();
        book.add_counterparty(Counterparty {
            id: store,
            name: "Store".into(),
            system: None,
        });

        Self {
            book,
            parties,
            household,
            alex,
            sam,
            studio,
            usd,
            eur,
            acme,
            alex_checking,
            alex_credit,
            alex_brokerage,
            alex_eur,
            sam_credit,
            joint_savings,
            mortgage,
            store,
        }
    }

    pub fn effect(&self, split: &Split) -> Result<Option<(Effect, Amount)>, SettlementError> {
        let c = split.classify(&self.book).unwrap();
        effect(split, &c, &self.book, self.household, self.usd)
    }

    /// The items for `(date, split)` pairs, as the server builds them.
    pub fn items(&self, splits: &[(Date, Split)]) -> Vec<Item> {
        splits
            .iter()
            .filter_map(|(date, split)| {
                let (effect, amount) = self.effect(split).unwrap()?;
                Some(Item {
                    source: SplitRef {
                        transaction: TransactionId::new(),
                        position: 0,
                    },
                    date: *date,
                    effect,
                    amount,
                    settles: None,
                })
            })
            .collect()
    }
}

pub fn amt(s: &str) -> Amount {
    s.parse().unwrap()
}

pub fn month(s: &str) -> Month {
    s.parse().unwrap()
}

pub fn split(from: Side, to: Side, commodity: CommodityId, qty: &str) -> Split {
    Split {
        from: Leg {
            side: from,
            commodity,
            quantity: amt(qty),
        },
        to: Leg {
            side: to,
            commodity,
            quantity: amt(qty),
        },
        purpose: None,
        category: None,
        bill: None,
        description: None,
        settles: None,
    }
}

pub fn spend(h: &H, account: AccountId, qty: &str, purpose: PartyId) -> Split {
    Split {
        purpose: Some(purpose),
        ..split(
            Side::Account(account),
            Side::Counterparty(Some(h.store)),
            h.usd,
            qty,
        )
    }
}

pub fn transfer(h: &H, from: AccountId, to: AccountId, qty: &str) -> Split {
    split(Side::Account(from), Side::Account(to), h.usd, qty)
}

pub fn params(stipends: &[(PartyId, &str)], funders: &[(PartyId, PartyId)]) -> MonthParams {
    MonthParams {
        stipends: stipends.iter().map(|(p, a)| (*p, amt(a))).collect(),
        funders: funders.iter().copied().collect(),
    }
}
