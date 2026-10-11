//! The deterministic discrepancy finder (design §7.1, step 1).
//!
//! When a balance check doesn't match, the difference `ledger − bank` is
//! explained by searching the account's postings in the window since its
//! last verified date. Every candidate says why it was suggested; nothing is
//! changed. The AI explanation (step 6) builds on these candidates.

use time::{Date, Duration};

use crate::amount::Amount;
use crate::settlement::SplitRef;

/// One posting on the checked account and commodity, in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub source: SplitRef,
    pub date: Date,
    /// Positive into the account.
    pub quantity: Amount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Ordered most likely first.
pub enum Reason {
    /// Two postings of the same amount a few days apart, one of them equal
    /// to the difference.
    DuplicatePair,
    /// One posting equal to the difference: recorded twice, or not yet in
    /// the bank.
    EqualsDifference,
    /// Dated just around the check date, and equal to the difference (or its
    /// opposite): the bank may have dated it on the other side.
    DateBoundary,
    /// Recorded with the wrong sign: the difference is twice its amount.
    SignFlipped,
    /// Two or three postings that together equal the difference.
    Subset,
    /// Two adjacent digits swapped in one posting would explain it.
    Transposition,
    /// Two postings of the same amount a few days apart.
    NearDuplicate,
    /// The difference is a multiple of 9 (in the smallest unit), the mark of
    /// a transposition typo; no single posting accounts for it.
    MultipleOfNine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub reason: Reason,
    /// The postings involved (none for `MultipleOfNine`).
    pub sources: Vec<SplitRef>,
}

/// Days around a date that count as "near" for duplicates and boundaries.
pub const NEAR_DAYS: i64 = 3;
/// Larger windows skip the subset search (it grows with the cube).
pub const SUBSET_LIMIT: usize = 400;
/// The most candidates returned.
pub const MAX_CANDIDATES: usize = 25;

/// Candidates for `difference = ledger − bank` at `date`, most likely first.
/// `entries` are the window's postings (dated after the last verified date)
/// plus those up to `NEAR_DAYS` after `date`; `scale` is the commodity's
/// precision.
#[must_use]
pub fn candidates(entries: &[Entry], date: Date, difference: Amount, scale: u32) -> Vec<Candidate> {
    if difference.is_zero() {
        return Vec::new();
    }
    let near = Duration::days(NEAR_DAYS);
    let (inside, after): (Vec<&Entry>, Vec<&Entry>) = entries.iter().partition(|e| e.date <= date);
    let mut out: Vec<Candidate> = Vec::new();
    let one = |reason, e: &Entry| Candidate {
        reason,
        sources: vec![e.source],
    };

    // A posting counted in the ledger that the bank doesn't have yet (or
    // twice in the ledger).
    let equal: Vec<&&Entry> = inside.iter().filter(|e| e.quantity == difference).collect();
    for e in &equal {
        // Same amount nearby: most likely a duplicate.
        if let Some(twin) = inside.iter().find(|o| {
            o.source != e.source && o.quantity == e.quantity && (o.date - e.date).abs() <= near
        }) {
            out.push(Candidate {
                reason: Reason::DuplicatePair,
                sources: vec![twin.source, e.source],
            });
        }
        out.push(one(Reason::EqualsDifference, e));
    }

    // Around the check date: counted on one side of it by the ledger and on
    // the other by the bank.
    for e in &inside {
        if date - e.date <= near && e.quantity == difference {
            out.push(one(Reason::DateBoundary, e));
        }
    }
    for e in &after {
        if e.date - date <= near && e.quantity == -difference {
            out.push(one(Reason::DateBoundary, e));
        }
    }

    // Wrong sign: the ledger has +q where the bank has −q (difference 2q).
    for e in &inside {
        if e.quantity.checked_add(e.quantity).ok() == Some(difference) {
            out.push(one(Reason::SignFlipped, e));
        }
    }

    // Two or three postings summing to the difference.
    if inside.len() <= SUBSET_LIMIT {
        let n = inside.len();
        'search: for i in 0..n {
            for j in i + 1..n {
                let two = inside[i].quantity.checked_add(inside[j].quantity).ok();
                if two == Some(difference) {
                    out.push(Candidate {
                        reason: Reason::Subset,
                        sources: vec![inside[i].source, inside[j].source],
                    });
                }
                for k in j + 1..n {
                    let three = two.and_then(|t| t.checked_add(inside[k].quantity).ok());
                    if three == Some(difference) {
                        out.push(Candidate {
                            reason: Reason::Subset,
                            sources: vec![inside[i].source, inside[j].source, inside[k].source],
                        });
                    }
                    if out.len() >= MAX_CANDIDATES * 2 {
                        break 'search;
                    }
                }
            }
        }
    }

    // Transposed digits.
    let units = minor_units(difference, scale);
    if units.is_some_and(|u| u % 9 == 0) {
        let mut found = false;
        for e in &inside {
            let Some(q) = minor_units(e.quantity, scale) else {
                continue;
            };
            if transpositions(q)
                .into_iter()
                .any(|t| q - t == units.unwrap_or(0))
            {
                out.push(one(Reason::Transposition, e));
                found = true;
            }
        }
        if !found {
            out.push(Candidate {
                reason: Reason::MultipleOfNine,
                sources: Vec::new(),
            });
        }
    }

    // Same amount twice within a few days, whatever the difference.
    for (i, a) in inside.iter().enumerate() {
        for b in &inside[i + 1..] {
            if a.quantity == b.quantity
                && (a.date - b.date).abs() <= near
                && a.quantity != difference
            {
                out.push(Candidate {
                    reason: Reason::NearDuplicate,
                    sources: vec![a.source, b.source],
                });
            }
        }
    }

    out.sort_by_key(|c| c.reason);
    let mut seen = Vec::new();
    out.retain(|c| {
        let mut key = c.sources.clone();
        key.sort();
        let new = !seen.contains(&(c.reason, key.clone()));
        seen.push((c.reason, key));
        new
    });
    out.truncate(MAX_CANDIDATES);
    out
}

/// `a` in the commodity's smallest unit, e.g. cents (None if it doesn't fit).
fn minor_units(a: Amount, scale: u32) -> Option<i128> {
    let mut d = a.as_decimal();
    d.rescale(scale);
    (d.scale() == scale).then(|| d.mantissa())
}

/// Every value obtained by swapping two adjacent digits of `q` (sign kept).
fn transpositions(q: i128) -> Vec<i128> {
    let negative = q < 0;
    let digits: Vec<u8> = q.unsigned_abs().to_string().bytes().collect();
    let mut out = Vec::new();
    for i in 0..digits.len().saturating_sub(1) {
        if digits[i] == digits[i + 1] {
            continue;
        }
        let mut d = digits.clone();
        d.swap(i, i + 1);
        if let Ok(v) = String::from_utf8_lossy(&d).parse::<i128>() {
            out.push(if negative { -v } else { v });
        }
    }
    out
}

#[cfg(test)]
mod tests {
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
}
