use time::macros::date;

use super::*;
use crate::ids::TransactionId;

fn entry(day: Date, q: &str) -> Entry {
    Entry {
        source: SplitRef {
            transaction: TransactionId::new(),
            position: 0,
        },
        date: day,
        quantity: q.parse().unwrap(),
    }
}

fn reasons(c: &[Candidate]) -> Vec<Reason> {
    c.iter().map(|c| c.reason).collect()
}

const D: Date = date!(2026 - 10 - 11);

#[test]
fn duplicate_charge() {
    let e = [
        entry(date!(2026 - 10 - 03), "-47.30"),
        entry(date!(2026 - 10 - 04), "-47.30"),
        entry(date!(2026 - 10 - 05), "-12.00"),
    ];
    // The ledger is 47.30 lower than the bank.
    let c = candidates(&e, D, "-47.30".parse().unwrap(), 2);
    assert_eq!(c[0].reason, Reason::DuplicatePair);
    assert_eq!(c[0].sources.len(), 2);
    assert!(reasons(&c).contains(&Reason::EqualsDifference));
}

#[test]
fn sign_boundary_subset_transposition() {
    let e = [
        entry(date!(2026 - 10 - 01), "-30.00"),
        entry(date!(2026 - 10 - 02), "-45.50"),
        entry(date!(2026 - 10 - 10), "-19.99"),
        entry(date!(2026 - 10 - 12), "-7.25"), // after the check date
        entry(date!(2026 - 10 - 06), "-63.00"),
    ];
    let first = |diff: &str| candidates(&e, D, diff.parse().unwrap(), 2)[0].reason;
    assert_eq!(first("-60.00"), Reason::SignFlipped);
    // The bank already has the 12th's charge.
    assert_eq!(first("7.25"), Reason::DateBoundary);
    assert_eq!(first("-75.50"), Reason::Subset);
    // 63.00 entered, 36.00 charged: off by 27.00.
    assert_eq!(first("-27.00"), Reason::Transposition);
    assert_eq!(first("0.09"), Reason::MultipleOfNine);
    assert!(candidates(&e, D, Amount::ZERO, 2).is_empty());
}

#[test]
fn digit_swaps() {
    assert_eq!(transpositions(6300), vec![3600, 6030]);
    assert_eq!(transpositions(-12), vec![-21]);
    assert_eq!(minor_units("1.5".parse().unwrap(), 2), Some(150));
}
