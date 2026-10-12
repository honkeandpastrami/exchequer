//! The query engine's core (design §7): postings, flows and balances on the
//! fictional household.

mod common;

use std::collections::BTreeMap;

use common::*;
use domain::report::{Bucket, Dimension, Key, KindKey, Posting, Value, balances, flows, postings};
use domain::settlement::SplitRef;
use domain::*;
use time::Date;
use time::macros::date;

fn posted(h: &H, rows: &[(Date, Split, &[&str])]) -> Vec<Posting> {
    rows.iter()
        .enumerate()
        .flat_map(|(i, (date, split, tags))| {
            let c = split.classify(&h.book).unwrap();
            let tags: Vec<String> = tags.iter().map(|t| (*t).to_owned()).collect();
            let source = SplitRef {
                transaction: TransactionId::new(),
                position: i,
            };
            postings(source, *date, &tags, split, &c, &h.book, h.household)
        })
        .collect()
}

fn usd_key(h: &H, bucket: Option<Date>, values: Vec<Value>) -> Key {
    Key {
        bucket,
        values,
        commodity: h.usd,
    }
}

#[test]
fn postings_carry_signs_and_default_purposes() {
    let h = H::new();
    let ps = posted(
        &h,
        &[
            (
                date!(2026 - 06 - 01),
                spend(&h, h.alex_credit, "80", h.household),
                &[],
            ),
            (
                date!(2026 - 06 - 02),
                split(
                    Side::Account(h.alex_credit),
                    Side::Counterparty(Some(h.store)),
                    h.usd,
                    "5",
                ),
                &[],
            ),
            (
                date!(2026 - 06 - 03),
                transfer(&h, h.alex_checking, h.alex_credit, "85"),
                &[],
            ),
            (
                date!(2026 - 06 - 04),
                transfer(&h, h.alex_brokerage, h.joint_savings, "900"),
                &[],
            ),
        ],
    );
    let summary: Vec<(AccountId, String, Option<PartyId>)> = ps
        .iter()
        .map(|p| (p.account, p.quantity.to_string(), p.purpose))
        .collect();
    assert_eq!(
        summary,
        [
            (h.alex_credit, "-80".into(), Some(h.household)),
            // No purpose given: the account's owner.
            (h.alex_credit, "-5".into(), Some(h.alex)),
            // Transfers: two postings, no purpose.
            (h.alex_checking, "-85".into(), None),
            (h.alex_credit, "85".into(), None),
            // Liquidations: the household by default.
            (h.alex_brokerage, "-900".into(), Some(h.household)),
            (h.joint_savings, "900".into(), Some(h.household)),
        ]
    );
    assert_eq!(ps[0].counterparty, Some(h.store));
    assert_eq!(ps[2].counterparty, None);
}

#[test]
fn flows_by_purpose_kind_and_month() {
    let h = H::new();
    let ps = posted(
        &h,
        &[
            (
                date!(2026 - 06 - 01),
                spend(&h, h.alex_credit, "80", h.household),
                &["trip"],
            ),
            (
                date!(2026 - 06 - 15),
                spend(&h, h.alex_credit, "20", h.alex),
                &["trip", "gift"],
            ),
            (
                date!(2026 - 07 - 01),
                spend(&h, h.sam_credit, "40", h.household),
                &[],
            ),
            (
                date!(2026 - 07 - 02),
                transfer(&h, h.alex_checking, h.alex_credit, "100"),
                &[],
            ),
        ],
    );

    let by = flows(
        &ps,
        &[Dimension::Purpose, Dimension::Kind],
        Some(Bucket::Month),
    )
    .unwrap();
    let june = Some(date!(2026 - 06 - 01));
    let july = Some(date!(2026 - 07 - 01));
    let expense = Value::Kind(KindKey::Expense);
    let transfer = Value::Kind(KindKey::Transfer);
    assert_eq!(
        by,
        BTreeMap::from([
            (
                usd_key(
                    &h,
                    june,
                    vec![Value::Party(Some(h.household)), expense.clone()]
                ),
                "-80".parse().unwrap()
            ),
            (
                usd_key(&h, june, vec![Value::Party(Some(h.alex)), expense.clone()]),
                "-20".parse().unwrap()
            ),
            (
                usd_key(&h, july, vec![Value::Party(Some(h.household)), expense]),
                "-40".parse().unwrap()
            ),
            // A transfer within the household nets to zero.
            (
                usd_key(&h, july, vec![Value::Party(None), transfer]),
                Amount::ZERO
            ),
        ])
    );

    // A transaction with two tags counts in each.
    let by_tag = flows(&ps, &[Dimension::Tag], None).unwrap();
    let tag = |t: Option<&str>| usd_key(&h, None, vec![Value::Tag(t.map(str::to_owned))]);
    assert_eq!(by_tag[&tag(Some("trip"))], "-100".parse().unwrap());
    assert_eq!(by_tag[&tag(Some("gift"))], "-20".parse().unwrap());
    assert_eq!(by_tag[&tag(None)], "-40".parse().unwrap());
}

#[test]
fn balances_at_each_bucket_end() {
    let h = H::new();
    let ps = posted(
        &h,
        &[
            (
                date!(2026 - 05 - 31),
                spend(&h, h.alex_credit, "10", h.alex),
                &[],
            ),
            (
                date!(2026 - 06 - 10),
                spend(&h, h.alex_credit, "80", h.household),
                &[],
            ),
            (
                date!(2026 - 08 - 10),
                transfer(&h, h.alex_checking, h.alex_credit, "90"),
                &[],
            ),
            (
                date!(2026 - 09 - 01),
                spend(&h, h.alex_credit, "1", h.alex),
                &[],
            ),
        ],
    );
    let opening = BTreeMap::from([(
        (vec![Value::Account(h.alex_checking)], h.usd),
        "1000".parse().unwrap(),
    )]);
    let b = balances(
        &opening,
        &ps,
        &[Dimension::Account],
        Some(Bucket::Month),
        date!(2026 - 05 - 01),
        date!(2026 - 08 - 31),
    )
    .unwrap();
    let at = |month: Date, account| {
        b[&usd_key(&h, Some(month), vec![Value::Account(account)])].to_string()
    };
    assert_eq!(at(date!(2026 - 05 - 01), h.alex_credit), "-10");
    assert_eq!(at(date!(2026 - 05 - 01), h.alex_checking), "1000");
    assert_eq!(at(date!(2026 - 06 - 01), h.alex_credit), "-90");
    assert_eq!(at(date!(2026 - 07 - 01), h.alex_credit), "-90");
    assert_eq!(at(date!(2026 - 08 - 01), h.alex_credit), "0");
    assert_eq!(at(date!(2026 - 08 - 01), h.alex_checking), "910");
    assert_eq!(
        b.len(),
        8,
        "two accounts, four months; September isn't asked for"
    );

    // Without a bucket: one balance per group, at the end date.
    let b = balances(
        &BTreeMap::new(),
        &ps,
        &[Dimension::Owner],
        None,
        date!(2026 - 01 - 01),
        date!(2026 - 09 - 30),
    )
    .unwrap();
    assert_eq!(
        b,
        BTreeMap::from([(
            usd_key(&h, None, vec![Value::Party(Some(h.alex))]),
            "-91".parse().unwrap()
        )])
    );
}
