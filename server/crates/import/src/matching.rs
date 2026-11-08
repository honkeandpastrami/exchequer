//! Matching staged rows (design §6.3-§6.4): to transactions already in the
//! ledger (duplicates), and to each other (transfer pairs). Each match is
//! one-to-one.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use rust_decimal::Decimal;
use suggest::TRANSFER_DAYS;
use time::Date;
use uuid::Uuid;

use crate::params::{NEAR_DAY, NEAR_LIKENESS, NEAR_MOST, TRANSFER_DAY, UNMEASURED};

/// How sure matching is, per mille, that two sides of a transfer `days`
/// apart are one transfer (a transfer pair, or a recorded transfer).
#[must_use]
pub fn transfer_confidence(days: Option<i32>) -> i32 {
    days.map_or(UNMEASURED, |d| (1000 - TRANSFER_DAY * d).max(0))
}

/// How sure matching is, per mille, that a possible duplicate by amount is the
/// transaction it's like: never sure ([`NEAR_MOST`]).
#[must_use]
pub fn near_confidence(days: i32, similarity: i32) -> i32 {
    (similarity * NEAR_LIKENESS / 100 - NEAR_DAY * days).clamp(0, NEAR_MOST)
}

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

/// What a staged row's other side is, as you chose it. A suggestion doesn't
/// count: it was made without seeing the other side, and a pair settles it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Not chosen.
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
    /// Whether it may be matched: a new row (not a duplicate), whatever
    /// its choice, so choosing never changes what it is.
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

/// Whether `a`'s other side, as you chose it, lets `b` be its transfer's
/// other side: not chosen, or chosen as a transfer to `b`'s account.
fn allows<K>(a: &Staged<K>, b: &Staged<K>) -> bool {
    match a.side {
        Side::Open => true,
        Side::Transfer(to) => to == b.account,
        Side::Outside => false,
    }
}

/// Eligible rows that fit each other, each pair once, the lower key first.
fn fitting<K: Ord>(rows: &[Staged<K>]) -> Vec<(&Staged<K>, &Staged<K>)> {
    let eligible: Vec<&Staged<K>> = rows.iter().filter(|r| r.eligible).collect();
    let mut out = Vec::new();
    for (i, a) in eligible.iter().enumerate() {
        for b in &eligible[i + 1..] {
            if fits(a, b) {
                out.push(if a.key < b.key { (*a, *b) } else { (*b, *a) });
            }
        }
    }
    out
}

fn days<K>(a: &Staged<K>, b: &Staged<K>) -> i64 {
    (a.date - b.date).whole_days().abs()
}

/// Takes pairs in order, each row at most once; each key maps to the
/// other's.
fn greedy<K: Copy + Ord + Hash, R: Ord>(mut pairs: Vec<(R, K, K)>) -> HashMap<K, K> {
    pairs.sort();
    let mut out = HashMap::new();
    for (_, x, y) in pairs {
        if out.contains_key(&x) || out.contains_key(&y) {
            continue;
        }
        out.insert(x, y);
        out.insert(y, x);
    }
    out
}

/// Transfer matches among staged rows: pairs of rows that are the two sides
/// of one transfer, each key mapped to the other's. Two rows that fit each
/// other pair unless your choice says otherwise: a counterparty (or none)
/// you chose for either, or a transfer to another account. Suggestions
/// don't count; the pair settles them. Rows you chose as transfers to each
/// other go first, then the closest dates.
#[must_use]
pub fn transfer_matches<K: Copy + Ord + Hash>(rows: &[Staged<K>]) -> HashMap<K, K> {
    let pairs = fitting(rows)
        .into_iter()
        .filter(|(a, b)| allows(a, b) && allows(b, a))
        .map(|(a, b)| {
            let chosen = [a, b]
                .iter()
                .filter(|r| matches!(r.side, Side::Transfer(_)))
                .count();
            ((Reverse(chosen), days(a, b)), a.key, b.key)
        })
        .collect();
    greedy(pairs)
}

/// Transfer conflicts among the rows [`transfer_matches`] left unpaired:
/// a row you chose as a transfer to one account, and a row in another of
/// your accounts that fits it and would take it (not chosen, or chosen as
/// the transfer back). They're a transfer pair whose default keeps your
/// choice. One-to-one, the closest dates first.
#[must_use]
pub fn transfer_conflicts<K: Copy + Ord + Hash>(
    rows: &[Staged<K>],
    pairs: &HashMap<K, K>,
) -> HashMap<K, K> {
    let free = |r: &Staged<K>| !pairs.contains_key(&r.key);
    let clash = |a: &Staged<K>, b: &Staged<K>| {
        matches!(a.side, Side::Transfer(to) if to != b.account) && allows(b, a)
    };
    let conflicts = fitting(rows)
        .into_iter()
        .filter(|(a, b)| free(a) && free(b) && (clash(a, b) || clash(b, a)))
        .map(|(a, b)| (days(a, b), a.key, b.key))
        .collect();
    greedy(conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKING: Uuid = Uuid::from_u128(1);
    const CARD: Uuid = Uuid::from_u128(2);
    const SAVINGS: Uuid = Uuid::from_u128(3);
    const USD: Uuid = Uuid::from_u128(9);

    #[test]
    fn how_sure_matching_is() {
        // A transfer: surer the closer its sides are dated.
        let transfer: Vec<i32> = [Some(0), Some(1), Some(3), Some(5), None]
            .into_iter()
            .map(transfer_confidence)
            .collect();
        assert_eq!(transfer, [1000, 970, 910, 850, 500]);
        // A possible duplicate never reaches confident, however alike.
        assert_eq!(near_confidence(0, 1000), 890);
        assert_eq!(near_confidence(1, 1000), 830);
        assert_eq!(near_confidence(2, 300), 147);
        assert_eq!(near_confidence(3, 0), 0);
    }

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
    fn rows_that_fit_pair_unless_you_chose_otherwise() {
        let rows = vec![
            // A card payment from checking and its arrival on the card:
            // neither side chosen, still a pair.
            staged(1, CHECKING, 13, -64_218, Side::Open),
            staged(2, CARD, 10, 64_218, Side::Open),
            // A counterparty you chose: never paired.
            staged(3, CHECKING, 9, -5000, Side::Open),
            staged(4, SAVINGS, 9, 5000, Side::Outside),
            // A transfer you chose to another account: not paired either.
            staged(5, CHECKING, 20, -700, Side::Transfer(CARD)),
            staged(6, SAVINGS, 20, 700, Side::Open),
            // A day past the window: too far.
            staged(7, CHECKING, 1, -900, Side::Open),
            staged(
                8,
                CARD,
                2 + u8::try_from(TRANSFER_DAYS).unwrap(),
                900,
                Side::Open,
            ),
        ];
        assert_eq!(transfer_matches(&rows), HashMap::from([(1, 2), (2, 1)]));
    }

    #[test]
    fn pairs_are_one_to_one_chosen_first_then_closest() {
        let rows = vec![
            staged(1, CHECKING, 10, -10_000, Side::Transfer(SAVINGS)),
            // Closer, but not chosen.
            staged(2, SAVINGS, 10, 10_000, Side::Open),
            // Chosen as the transfer back: pairs chosen on both sides go
            // first, the closest of them (with 4) before the other (with 1).
            staged(3, SAVINGS, 13, 10_000, Side::Transfer(CHECKING)),
            staged(4, CHECKING, 11, -10_000, Side::Transfer(SAVINGS)),
            // Too far apart, the wrong sign: never.
            staged(5, SAVINGS, 20, 10_000, Side::Open),
            staged(6, SAVINGS, 10, -10_000, Side::Open),
        ];
        let got = transfer_matches(&rows);
        // 1 takes the one left.
        assert_eq!(got, HashMap::from([(3, 4), (4, 3), (1, 2), (2, 1)]));
        // A row that can't be matched (a duplicate, or split) isn't.
        let mut rows = rows;
        rows[2].eligible = false;
        rows[3].eligible = false;
        assert_eq!(transfer_matches(&rows), HashMap::from([(1, 2), (2, 1)]));
    }

    #[test]
    fn a_transfer_chosen_elsewhere_is_a_conflict() {
        let rows = vec![
            // You chose savings, but the card holds the other side.
            staged(1, CHECKING, 13, -100_429, Side::Transfer(SAVINGS)),
            staged(2, CARD, 10, 100_429, Side::Open),
            // Another, the same day.
            staged(3, CHECKING, 13, -2000, Side::Transfer(SAVINGS)),
            staged(4, CARD, 13, 2000, Side::Open),
            // Paired with the account you chose: no conflict with the card.
            staged(5, CHECKING, 13, -300, Side::Transfer(SAVINGS)),
            staged(6, SAVINGS, 13, 300, Side::Open),
            staged(7, CARD, 13, 300, Side::Open),
        ];
        let pairs = transfer_matches(&rows);
        assert_eq!(pairs, HashMap::from([(5, 6), (6, 5)]));
        assert_eq!(
            transfer_conflicts(&rows, &pairs),
            HashMap::from([(1, 2), (2, 1), (3, 4), (4, 3)])
        );
    }
}
