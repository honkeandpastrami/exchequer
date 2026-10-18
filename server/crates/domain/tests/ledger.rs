//! The examples table of design §3.3, plus the invariants of §3.2–§3.5, on the
//! fictional household (Alex, Sam, Studio).

use domain::transaction::{ImpliedPrice, TransactionError};
use domain::*;
use proptest::prelude::*;

struct Household {
    book: Book,
    // Parties
    household: PartyId,
    alex: PartyId,
    // Commodities
    usd: CommodityId,
    eur: CommodityId,
    acme: CommodityId,
    vti: CommodityId,
    // Accounts
    alex_checking: AccountId,
    alex_credit: AccountId,
    alex_brokerage: AccountId,
    alex_eur: AccountId,
    joint_savings: AccountId,
    joint_brokerage: AccountId,
    mortgage: AccountId,
    // Counterparties
    grocer: CounterpartyId,
    store: CounterpartyId,
    lender: CounterpartyId,
    employer: CounterpartyId,
    fund_manager: CounterpartyId,
    opening: CounterpartyId,
    // Categories
    groceries: CategoryId,
}

impl Household {
    fn new() -> Self {
        let mut book = Book::default();

        let mut party = |name: &str, kind| {
            let p = Party {
                id: PartyId::new(),
                name: name.to_owned(),
                kind,
            };
            let id = p.id;
            book.add_party(p);
            id
        };
        let household = party("Household", PartyKind::Household);
        let alex = party("Alex", PartyKind::Person);
        party("Sam", PartyKind::Person);
        party("Studio", PartyKind::Business);

        let mut commodity = |code: &str, kind, precision| {
            let c = Commodity::new(CommodityId::new(), code, kind, precision).unwrap();
            let id = c.id;
            book.add_commodity(c);
            id
        };
        let usd = commodity("USD", CommodityKind::Fiat, 2);
        let eur = commodity("EUR", CommodityKind::Fiat, 2);
        let acme = commodity("ACME", CommodityKind::Security, 0);
        let vti = commodity("VTI", CommodityKind::Fund, 3);

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
        let alex_eur = account("Alex EUR Account", AccountKind::Checking, alex, true);
        let joint_savings = account("Joint Savings", AccountKind::Savings, household, true);
        let joint_brokerage = account("Joint Brokerage", AccountKind::Brokerage, household, false);
        let mortgage = account("Mortgage", AccountKind::Mortgage, household, false);

        let mut counterparty = |name: &str, system| {
            let c = Counterparty {
                id: CounterpartyId::new(),
                name: name.to_owned(),
                system,
            };
            let id = c.id;
            book.add_counterparty(c);
            id
        };

        Self {
            grocer: counterparty("Grocer", None),
            store: counterparty("Store", None),
            lender: counterparty("Lender", None),
            employer: counterparty("Employer", None),
            fund_manager: counterparty("Fund manager", None),
            opening: counterparty(
                "Opening balances",
                Some(SystemCounterparty::OpeningBalances),
            ),
            book,
            household,
            alex,
            usd,
            eur,
            acme,
            vti,
            alex_checking,
            alex_credit,
            alex_brokerage,
            alex_eur,
            joint_savings,
            joint_brokerage,
            mortgage,
            groceries: CategoryId::new(),
        }
    }

    fn classify(&self, split: &Split) -> Result<Classified, SplitError> {
        split.classify(&self.book)
    }
}

fn amt(s: &str) -> Amount {
    s.parse().unwrap()
}

fn acct(id: AccountId) -> Side {
    Side::Account(id)
}

fn cp(id: CounterpartyId) -> Side {
    Side::Counterparty(Some(id))
}

/// A split moving `qty` of one commodity, with no labels.
fn split(from: Side, to: Side, commodity: CommodityId, qty: &str) -> Split {
    conversion(from, (commodity, qty), to, (commodity, qty))
}

fn conversion(from: Side, out: (CommodityId, &str), to: Side, inn: (CommodityId, &str)) -> Split {
    Split {
        from: Leg {
            side: from,
            commodity: out.0,
            quantity: amt(out.1),
        },
        to: Leg {
            side: to,
            commodity: inn.0,
            quantity: amt(inn.1),
        },
        purpose: None,
        category: None,
        bill: None,
        description: None,
        settles: None,
    }
}

fn kind(c: Result<Classified, SplitError>) -> Kind {
    c.unwrap().kind
}

#[test]
fn examples_from_the_design() {
    let h = Household::new();
    let (usd, eur, acme, vti) = (h.usd, h.eur, h.acme, h.vti);

    // Groceries, labelled for the household.
    let groceries = Split {
        purpose: Some(h.household),
        category: Some(h.groceries),
        ..split(acct(h.alex_credit), cp(h.grocer), usd, "80")
    };
    assert_eq!(kind(h.classify(&groceries)), Kind::Expense);

    // Refund: income, same category.
    let refund = Split {
        category: Some(h.groceries),
        ..split(cp(h.store), acct(h.alex_credit), usd, "30")
    };
    assert_eq!(kind(h.classify(&refund)), Kind::Income);

    // Outside the household, unknown who: still an expense or income.
    let unknown = Side::Counterparty(None);
    let spent = Split {
        purpose: Some(h.household),
        category: Some(h.groceries),
        ..split(acct(h.alex_credit), unknown, usd, "12")
    };
    assert_eq!(kind(h.classify(&spent)), Kind::Expense);
    let received = split(unknown, acct(h.alex_checking), usd, "12");
    assert_eq!(kind(h.classify(&received)), Kind::Income);

    // Card payment.
    let card = split(acct(h.alex_checking), acct(h.alex_credit), usd, "500");
    let c = h.classify(&card).unwrap();
    assert_eq!(
        (c.kind, c.crossing, c.implied_price),
        (Kind::Transfer, false, None)
    );

    // Mortgage payment: principal and interest are both expenses, the
    // principal into the (non-liquid) mortgage.
    let principal = split(acct(h.joint_savings), acct(h.mortgage), usd, "1200");
    let interest = split(acct(h.joint_savings), cp(h.lender), usd, "800");
    assert_eq!(kind(h.classify(&principal)), Kind::Expense);
    assert_eq!(kind(h.classify(&interest)), Kind::Expense);

    // Stock vest: income in shares, into a non-liquid account.
    let vest = split(cp(h.employer), acct(h.alex_brokerage), acme, "10");
    assert_eq!(kind(h.classify(&vest)), Kind::Income);

    // Sell stock inside one account: a conversion, allowed.
    let sell = conversion(
        acct(h.alex_brokerage),
        (acme, "10"),
        acct(h.alex_brokerage),
        (usd, "1500"),
    );
    let c = h.classify(&sell).unwrap();
    assert_eq!(c.kind, Kind::Conversion);
    assert_eq!(
        c.implied_price,
        Some(ImpliedPrice {
            base: usd,
            quote: acme,
            rate: "0.00666666666666666667".parse().unwrap(),
        })
    );

    // Buy a fund.
    let buy = conversion(
        acct(h.joint_brokerage),
        (usd, "1000"),
        acct(h.joint_brokerage),
        (vti, "4.125"),
    );
    assert_eq!(kind(h.classify(&buy)), Kind::Conversion);

    // Dividend.
    let dividend = split(cp(h.fund_manager), acct(h.joint_brokerage), usd, "25");
    assert_eq!(kind(h.classify(&dividend)), Kind::Income);

    // FX exchange between two accounts: 1 EUR = 1.1 USD.
    let fx = conversion(
        acct(h.alex_checking),
        (usd, "1100"),
        acct(h.alex_eur),
        (eur, "1000"),
    );
    let c = h.classify(&fx).unwrap();
    assert_eq!(c.kind, Kind::Conversion);
    assert_eq!(c.implied_price.unwrap().rate.to_string(), "1.1");
}

#[test]
fn between_liquid_and_not() {
    let h = Household::new();
    // Alex Brokerage → Joint Savings: income, which may carry a purpose and
    // a category.
    let sold = Split {
        purpose: Some(h.household),
        category: Some(h.groceries),
        ..split(
            acct(h.alex_brokerage),
            acct(h.joint_savings),
            h.usd,
            "10000",
        )
    };
    let c = h.classify(&sold).unwrap();
    assert_eq!((c.kind, c.crossing), (Kind::Income, true));

    // The reverse (investing, paying down a loan) is an expense.
    let invest = split(
        acct(h.joint_savings),
        acct(h.joint_brokerage),
        h.usd,
        "5000",
    );
    let c = h.classify(&invest).unwrap();
    assert_eq!((c.kind, c.crossing), (Kind::Expense, true));

    // Between two non-liquid accounts: a transfer.
    let moved = split(
        acct(h.alex_brokerage),
        acct(h.joint_brokerage),
        h.usd,
        "5000",
    );
    let c = h.classify(&moved).unwrap();
    assert_eq!((c.kind, c.crossing), (Kind::Transfer, false));
}

#[test]
fn rejected_splits() {
    let h = Household::new();
    let usd = h.usd;

    let external = split(cp(h.employer), cp(h.store), usd, "10");
    assert_eq!(h.classify(&external), Err(SplitError::BothExternal));
    let unknown = split(Side::Counterparty(None), cp(h.store), usd, "10");
    assert_eq!(h.classify(&unknown), Err(SplitError::BothExternal));

    let same = split(acct(h.alex_checking), acct(h.alex_checking), usd, "10");
    assert_eq!(h.classify(&same), Err(SplitError::SameAccount));

    let mut unequal = split(acct(h.alex_checking), acct(h.alex_credit), usd, "10");
    unequal.to.quantity = amt("9.99");
    assert!(matches!(
        h.classify(&unequal),
        Err(SplitError::UnequalQuantities { .. })
    ));

    for bad in ["0", "-5"] {
        let s = split(acct(h.alex_credit), cp(h.grocer), usd, bad);
        assert!(
            matches!(h.classify(&s), Err(SplitError::NotPositive(_))),
            "{bad}"
        );
    }

    let cents = split(acct(h.alex_credit), cp(h.grocer), usd, "80.005");
    assert!(matches!(h.classify(&cents), Err(SplitError::Precision(_))));

    let ghost = split(acct(AccountId::new()), cp(h.grocer), usd, "1");
    assert!(matches!(
        h.classify(&ghost),
        Err(SplitError::UnknownAccount(_))
    ));
}

#[test]
fn opening_balances_are_adjustments_not_income_or_spending() {
    let h = Household::new();
    let usd = h.usd;
    // A debt's opening balance leaves the account; an asset's enters it.
    let owed = split(acct(h.mortgage), cp(h.opening), usd, "250000");
    let held = split(cp(h.opening), acct(h.joint_savings), usd, "9000");
    assert_eq!(kind(h.classify(&owed)), Kind::Adjustment);
    assert_eq!(kind(h.classify(&held)), Kind::Adjustment);
    // Neither is anyone's spending or anything's category.
    assert_eq!(
        h.classify(&Split {
            purpose: Some(h.household),
            ..owed.clone()
        }),
        Err(SplitError::PurposeNotAllowed(Kind::Adjustment))
    );
    assert_eq!(
        h.classify(&Split {
            category: Some(h.groceries),
            ..owed
        }),
        Err(SplitError::CategoryNotAllowed(Kind::Adjustment))
    );
}

#[test]
fn purpose_and_category_rules() {
    let h = Household::new();
    let usd = h.usd;

    // Transfers carry neither purpose nor category.
    let card = split(acct(h.alex_checking), acct(h.alex_credit), usd, "500");
    let with_purpose = Split {
        purpose: Some(h.alex),
        ..card.clone()
    };
    assert_eq!(
        h.classify(&with_purpose),
        Err(SplitError::PurposeNotAllowed(Kind::Transfer))
    );
    let with_category = Split {
        category: Some(h.groceries),
        ..card
    };
    assert_eq!(
        h.classify(&with_category),
        Err(SplitError::CategoryNotAllowed(Kind::Transfer))
    );

    // Income inside an investment account has a category but no purpose.
    let dividend = split(cp(h.fund_manager), acct(h.joint_brokerage), usd, "25");
    let categorized = Split {
        category: Some(h.groceries),
        ..dividend.clone()
    };
    assert!(h.classify(&categorized).is_ok());
    let with_purpose = Split {
        purpose: Some(h.household),
        ..dividend
    };
    assert_eq!(
        h.classify(&with_purpose),
        Err(SplitError::PurposeNotAllowed(Kind::Income))
    );

    // The purpose must be a known party.
    let unknown = Split {
        purpose: Some(PartyId::new()),
        ..split(acct(h.alex_credit), cp(h.grocer), usd, "80")
    };
    assert!(matches!(
        h.classify(&unknown),
        Err(SplitError::UnknownParty(_))
    ));
}

#[test]
fn transactions_report_the_failing_split() {
    let h = Household::new();
    let tx = |splits| Transaction {
        id: TransactionId::new(),
        date: time::Date::from_calendar_date(2026, time::Month::March, 14).unwrap(),
        description: "Warehouse store".to_owned(),
        notes: None,
        tags: vec![],
        splits,
    };
    assert_eq!(
        tx(vec![]).classify(&h.book),
        Err(TransactionError::NoSplits)
    );

    let ok = split(acct(h.alex_credit), cp(h.store), h.usd, "120");
    let bad = split(acct(h.alex_credit), cp(h.store), h.usd, "45.001");
    assert_eq!(
        tx(vec![ok.clone(), ok.clone()])
            .classify(&h.book)
            .unwrap()
            .len(),
        2
    );
    assert!(matches!(
        tx(vec![ok, bad]).classify(&h.book),
        Err(TransactionError::Split { index: 1, .. })
    ));
}

proptest! {
    /// The kind depends only on which sides are yours and whether the
    /// commodities match.
    #[test]
    fn kind_follows_the_table(
        from_mine in any::<bool>(),
        to_mine in any::<bool>(),
        same_commodity in any::<bool>(),
        cents in 1u32..10_000_000,
    ) {
        let h = Household::new();
        let qty = Amount::from_decimal(rust_decimal::Decimal::new(cents.into(), 2)).to_string();
        let from = if from_mine { acct(h.alex_checking) } else { cp(h.store) };
        let to = if to_mine { acct(h.joint_savings) } else { cp(h.grocer) };
        let target = if same_commodity { h.usd } else { h.eur };
        let s = conversion(from, (h.usd, &qty), to, (target, &qty));
        let expected = match (from_mine, to_mine, same_commodity) {
            (false, false, _) => Err(SplitError::BothExternal),
            (true, false, _) => Ok(Kind::Expense),
            (false, true, _) => Ok(Kind::Income),
            (true, true, true) => Ok(Kind::Transfer),
            (true, true, false) => Ok(Kind::Conversion),
        };
        prop_assert_eq!(h.classify(&s).map(|c| c.kind), expected);
    }
}
