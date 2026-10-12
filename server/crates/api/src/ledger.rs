//! Loads what a set of splits refers to from the database into a
//! `domain::Book`, so `domain` can check the ledger invariants.

use std::collections::HashSet;

use domain::{
    Account, AccountId, AccountKind, Book, Commodity, CommodityId, CommodityKind, Counterparty,
    CounterpartyId, Party, PartyId, PartyKind, Side, Split, SystemCounterparty,
};
use sqlx::PgConnection;
use store::parties::PartyRow;
use uuid::Uuid;

use crate::problem::Problem;

/// Every account, counterparty, commodity and purpose the splits mention.
/// IDs that don't exist are left out, so `domain` reports them as unknown.
pub async fn book_for(db: &mut PgConnection, splits: &[Split]) -> Result<Book, Problem> {
    let mut accounts = HashSet::new();
    let mut counterparties = HashSet::new();
    let mut commodities = HashSet::new();
    let mut parties = HashSet::new();
    for s in splits {
        for leg in [&s.from, &s.to] {
            match leg.side {
                Side::Account(id) => {
                    accounts.insert(*id.as_uuid());
                }
                Side::Counterparty(Some(id)) => {
                    counterparties.insert(*id.as_uuid());
                }
                Side::Counterparty(None) => {}
            }
            commodities.insert(*leg.commodity.as_uuid());
        }
        if let Some(p) = s.purpose {
            parties.insert(*p.as_uuid());
        }
    }
    let ids = |set: HashSet<Uuid>| set.into_iter().collect::<Vec<_>>();

    let mut book = Book::default();
    for r in store::accounts::get_many(&mut *db, &ids(accounts)).await? {
        book.add_account(Account {
            id: AccountId::from_uuid(r.id),
            name: r.name,
            kind: account_kind(&r.kind)?,
            owner: PartyId::from_uuid(r.owner_id),
            liquid: r.liquid,
        });
    }
    for r in store::counterparties::get_many(&mut *db, &ids(counterparties)).await? {
        book.add_counterparty(Counterparty {
            id: CounterpartyId::from_uuid(r.id),
            name: r.name,
            system: match r.system.as_deref() {
                None => None,
                Some("opening_balances") => Some(SystemCounterparty::OpeningBalances),
                Some("reconciliation") => Some(SystemCounterparty::Reconciliation),
                Some(_) => return Err(Problem::internal()),
            },
        });
    }
    for r in store::commodities::get_many(&mut *db, &ids(commodities)).await? {
        let kind = match r.kind.as_str() {
            "fiat" => CommodityKind::Fiat,
            "security" => CommodityKind::Security,
            "fund" => CommodityKind::Fund,
            "crypto" => CommodityKind::Crypto,
            "other" => CommodityKind::Other,
            _ => return Err(Problem::internal()),
        };
        let precision = u32::try_from(r.precision).map_err(|_| Problem::internal())?;
        let c = Commodity::new(CommodityId::from_uuid(r.id), &r.code, kind, precision)
            .map_err(|_| Problem::internal())?;
        book.add_commodity(c);
    }
    for r in store::parties::get_many(&mut *db, &ids(parties)).await? {
        book.add_party(party(r)?);
    }
    Ok(book)
}

/// The domain form of a stored party.
pub fn party(r: PartyRow) -> Result<Party, Problem> {
    Ok(Party {
        id: PartyId::from_uuid(r.id),
        name: r.name,
        kind: match r.kind.as_str() {
            "household" => PartyKind::Household,
            "person" => PartyKind::Person,
            "business" => PartyKind::Business,
            _ => return Err(Problem::internal()),
        },
    })
}

fn account_kind(s: &str) -> Result<AccountKind, Problem> {
    Ok(match s {
        "checking" => AccountKind::Checking,
        "savings" => AccountKind::Savings,
        "credit_card" => AccountKind::CreditCard,
        "brokerage" => AccountKind::Brokerage,
        "mortgage" => AccountKind::Mortgage,
        "cash" => AccountKind::Cash,
        "other" => AccountKind::Other,
        _ => return Err(Problem::internal()),
    })
}
