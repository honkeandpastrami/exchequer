//! Matching staged rows (design §6.3-§6.4): to transactions already in the
//! ledger (duplicates), and to each other (transfer matches). Each match is
//! one-to-one.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use rust_decimal::Decimal;
use suggest::TRANSFER_DAYS;
use time::Date;
use uuid::Uuid;

/// A staged row and an existing transaction that may be the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Near<K> {
    pub row: K,
    pub transaction: Uuid,
    /// Of the descriptions, per mille (0 when not compared).
    pub similarity: i32,
    pub days_apart: i32,
}

/// One-to-one matching: each row and each transaction is used at most once,
/// the most similar descriptions first, then the closest dates. Rows in
/// `done` and transactions in `taken` are already matched.
pub fn one_to_one<K: Copy + Ord + Hash>(
    mut candidates: Vec<Near<K>>,
    done: &HashSet<K>,
    taken: &HashSet<Uuid>,
) -> HashMap<K, Uuid> {
    candidates.sort_by(|a, b| {
        b.similarity
            .cmp(&a.similarity)
            .then(a.days_apart.cmp(&b.days_apart))
            .then(a.row.cmp(&b.row))
            .then(a.transaction.cmp(&b.transaction))
    });
    let mut taken = taken.clone();
    let mut out = HashMap::new();
    for c in candidates {
        if done.contains(&c.row) || out.contains_key(&c.row) {
            continue;
        }
        if taken.insert(c.transaction) {
            out.insert(c.row, c.transaction);
        }
    }
    out
}

/// What a staged row's other side is, as settled so far: by you, or by a
/// confident suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Not settled.
    Open,
    /// One of your accounts: a transfer.
    Transfer(Uuid),
    /// A counterparty, or none: not a transfer.
    Outside,
}

/// A staged row, as transfer matching sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged<K> {
    pub key: K,
    /// The statement's account.
    pub account: Uuid,
    pub commodity: Uuid,
    pub date: Date,
    /// Signed: positive into the account.
    pub amount: Decimal,
    pub side: Side,
    /// Whether it may be matched: a row that will become a new transaction
    /// (not a duplicate, not skipped, not one you made new on purpose).
    pub eligible: bool,
}

/// Whether `b` may be the other side of `a`'s transfer: in another account,
/// in the same commodity, the opposite amount, dated within
/// [`TRANSFER_DAYS`].
#[must_use]
pub fn fits<K>(a: &Staged<K>, b: &Staged<K>) -> bool {
    a.account != b.account
        && a.commodity == b.commodity
        && a.amount == -b.amount
        && (a.date - b.date).whole_days().abs() <= TRANSFER_DAYS
}

/// Transfer matches among staged rows: pairs of rows that are the two sides
/// of one transfer, each key mapped to the other's. A pair needs one row
/// settled as a transfer to the other's account, and the other row open or
/// settled as a transfer back; rows both settled that way go first, then
/// the closest dates.
#[must_use]
pub fn transfer_matches<K: Copy + Ord + Hash>(rows: &[Staged<K>]) -> HashMap<K, K> {
    let mut pairs: Vec<(bool, i64, K, K)> = Vec::new();
    for a in rows.iter().filter(|r| r.eligible) {
        let Side::Transfer(to) = a.side else { continue };
        for b in rows.iter().filter(|r| r.eligible && r.account == to) {
            let back = b.side == Side::Transfer(a.account);
            if !fits(a, b) || !(back || b.side == Side::Open) {
                continue;
            }
            // Each pair once, whichever side found it.
            if back && b.key < a.key {
                continue;
            }
            let days = (a.date - b.date).whole_days().abs();
            let (x, y) = if a.key < b.key {
                (a.key, b.key)
            } else {
                (b.key, a.key)
            };
            pairs.push((!back, days, x, y));
        }
    }
    pairs.sort();
    pairs.dedup();
    let mut out = HashMap::new();
    for (_, _, x, y) in pairs {
        if out.contains_key(&x) || out.contains_key(&y) {
            continue;
        }
        out.insert(x, y);
        out.insert(y, x);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKING: Uuid = Uuid::from_u128(1);
    const CARD: Uuid = Uuid::from_u128(2);
    const SAVINGS: Uuid = Uuid::from_u128(3);
    const USD: Uuid = Uuid::from_u128(9);

    fn near(row: u32, t: u128, similarity: i32, days_apart: i32) -> Near<u32> {
        Near {
            row,
            transaction: Uuid::from_u128(t),
            similarity,
            days_apart,
        }
    }

    #[test]
    fn duplicates_are_one_to_one_best_first() {
        // Two identical coffees and one recorded coffee: one match. The
        // better description wins a transaction; closer dates break ties.
        let got = one_to_one(
            vec![
                near(0, 1, 200, 0),
                near(1, 1, 900, 2),
                near(2, 2, 500, 3),
                near(2, 3, 500, 1),
                near(3, 4, 1000, 0),
            ],
            &HashSet::from([3]),
            &HashSet::from([Uuid::from_u128(2)]),
        );
        assert_eq!(
            got,
            HashMap::from([(1, Uuid::from_u128(1)), (2, Uuid::from_u128(3))])
        );
    }

    fn staged(key: u32, account: Uuid, day: u8, cents: i64, side: Side) -> Staged<u32> {
        Staged {
            key,
            account,
            commodity: USD,
            date: Date::from_calendar_date(2026, time::Month::March, day).unwrap(),
            amount: Decimal::new(cents, 2),
            side,
            eligible: true,
        }
    }

    #[test]
    fn a_settled_transfer_pairs_with_its_other_side() {
        let rows = vec![
            // A card payment from checking, settled as a transfer to the
            // card; the card's side is open.
            staged(1, CHECKING, 13, -64_218, Side::Transfer(CARD)),
            staged(2, CARD, 12, 64_218, Side::Open),
            // Same amount, but a counterparty on purpose: never paired.
            staged(3, CARD, 13, 64_218, Side::Outside),
            // Not settled on either side: no pair, however well it fits.
            staged(4, CHECKING, 9, -5000, Side::Open),
            staged(5, SAVINGS, 9, 5000, Side::Open),
        ];
        let got = transfer_matches(&rows);
        assert_eq!(got, HashMap::from([(1, 2), (2, 1)]));
    }

    #[test]
    fn pairs_are_one_to_one_agreeing_first_then_closest() {
        let rows = vec![
            staged(1, CHECKING, 10, -10_000, Side::Transfer(SAVINGS)),
            // Closer, but only open.
            staged(2, SAVINGS, 10, 10_000, Side::Open),
            // Settled as the transfer back: agreeing pairs go first, the
            // closest of them (with 4) before the other (with 1).
            staged(3, SAVINGS, 13, 10_000, Side::Transfer(CHECKING)),
            staged(4, CHECKING, 11, -10_000, Side::Transfer(SAVINGS)),
            // Too far apart, the wrong sign, another account: never.
            staged(5, SAVINGS, 20, 10_000, Side::Open),
            staged(6, SAVINGS, 10, -10_000, Side::Open),
            staged(7, CARD, 10, 10_000, Side::Open),
        ];
        let got = transfer_matches(&rows);
        // 1 takes the one left.
        assert_eq!(got, HashMap::from([(3, 4), (4, 3), (1, 2), (2, 1)]));
        // A row that can't be matched (a duplicate, or made new on purpose)
        // isn't.
        let mut rows = rows;
        rows[2].eligible = false;
        rows[3].eligible = false;
        assert_eq!(transfer_matches(&rows), HashMap::from([(1, 2), (2, 1)]));
    }
}
