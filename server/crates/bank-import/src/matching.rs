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
mod tests;
