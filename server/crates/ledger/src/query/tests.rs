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

fn posting() -> Posting {
    Posting {
        source: SplitRef {
            transaction: crate::ids::TransactionId::new(),
            position: 0,
        },
        date: date!(2026 - 08 - 19),
        account: AccountId::new(),
        owner: PartyId::new(),
        commodity: CommodityId::new(),
        quantity: Amount::from_decimal(rust_decimal::Decimal::NEGATIVE_ONE),
        kind: Kind::Expense,
        purpose: Some(PartyId::new()),
        category: None,
        counterparty: Some(CounterpartyId::new()),
        tags: vec!["trip".to_owned()],
    }
}

#[test]
fn filters() {
    let p = posting();
    let keeps = |f: Filter| f.keeps(&p);
    assert!(keeps(Filter::default()));
    assert!(keeps(Filter {
        from: Some(p.date),
        to: Some(p.date),
        ..Filter::default()
    }));
    assert!(!keeps(Filter {
        from: Some(date!(2026 - 08 - 20)),
        ..Filter::default()
    }));
    // `None` takes postings with no value there.
    assert!(keeps(Filter {
        categories: vec![None],
        ..Filter::default()
    }));
    assert!(!keeps(Filter {
        categories: vec![Some(CategoryId::new())],
        ..Filter::default()
    }));
    assert!(!keeps(Filter {
        purposes: vec![None],
        ..Filter::default()
    }));
    // Values inside one filter are alternatives; filters all apply.
    assert!(keeps(Filter {
        kinds: vec![Kind::Income, Kind::Expense],
        accounts: vec![AccountId::new(), p.account],
        ..Filter::default()
    }));
    assert!(!keeps(Filter {
        kinds: vec![Kind::Expense],
        commodities: vec![CommodityId::new()],
        ..Filter::default()
    }));
    assert!(keeps(Filter {
        tags: vec!["home".to_owned(), "trip".to_owned()],
        ..Filter::default()
    }));
    assert!(!keeps(Filter {
        tags: vec!["home".to_owned()],
        ..Filter::default()
    }));

    let text = Filter {
        text: Some("RENT".to_owned()),
        ..Filter::default()
    };
    assert!(text.has_text(["Lunch", "June rent"]));
    assert!(!text.has_text(["Lunch"]));
    assert!(Filter::default().has_text([]));
    assert!(!text.by_holding_only());
    assert!(
        Filter {
            accounts: vec![p.account],
            commodities: vec![p.commodity],
            ..Filter::default()
        }
        .by_holding_only()
    );
}
