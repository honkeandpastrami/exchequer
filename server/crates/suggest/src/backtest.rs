//! Measures a local method on the ledger's own past (design §8): trained on
//! everything before a period, it suggests the period's transactions as if
//! they were being imported, and its guesses are compared with what the
//! ledger says. Any method can be measured, so two can be compared on the
//! same data before one replaces the other.

use std::collections::BTreeMap;

use time::Date;
use uuid::Uuid;

use crate::{
    CONFIDENT, Example, Facts, Guess, History, Identity, Label, Method, Row, Statement,
    StatementKind, Value,
};

/// How many confidence bins: 0-99, 100-199, …, 900-1000 per mille.
pub const BINS: usize = 10;

/// Guesses against what the ledger says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Score {
    /// Rows (or statements) asked about.
    pub asked: u32,
    /// Of them, those with a guess.
    pub guessed: u32,
    /// Of them, those guessed right.
    pub correct: u32,
    /// Guesses at [`CONFIDENT`] or more.
    pub confident: u32,
    pub confident_correct: u32,
    /// Guesses by confidence, in [`BINS`] bins, each `(guesses, right)`.
    pub bins: [(u32, u32); BINS],
}

impl Score {
    fn add<T: PartialEq>(&mut self, guess: Option<&Guess<T>>, truth: &T) {
        self.asked += 1;
        let Some(g) = guess else { return };
        let right = g.value == *truth;
        self.guessed += 1;
        self.correct += u32::from(right);
        if g.confidence >= CONFIDENT {
            self.confident += 1;
            self.confident_correct += u32::from(right);
        }
        let bin = &mut self.bins[usize::from(g.confidence / 100).min(BINS - 1)];
        bin.0 += 1;
        bin.1 += u32::from(right);
    }
}

/// One period's results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The period: `start <= date < end`.
    pub start: Date,
    pub end: Date,
    /// The suggester trained on everything before `start`.
    pub suggester: Identity,
    /// How many labeled transactions it learned from.
    pub trained_on: usize,
    pub labels: BTreeMap<Label, Score>,
    /// Each account's transactions in the period, as one statement without
    /// an account.
    pub accounts: Score,
}

/// Trains `method` on `history` before `start`, then suggests labels for
/// each transaction dated in `[start, end)` (its account known, nothing
/// settled) and an account for each account's transactions in the period.
#[must_use]
pub fn run(method: &dyn Method, history: &History, start: Date, end: Date) -> Report {
    let past = history.before(start);
    let s = method.train(&past);
    let tests: Vec<&Example> = history
        .examples
        .iter()
        .filter(|e| e.date >= start && e.date < end)
        .collect();
    let rows: Vec<Row> = tests
        .iter()
        .map(|e| Row {
            account: Some(e.account),
            date: e.date,
            amount: e.amount,
            description: e.description.clone(),
            raw: serde_json::Value::Null,
            facts: Facts::default(),
        })
        .collect();
    let mut labels: BTreeMap<Label, Score> = Label::ALL
        .into_iter()
        .map(|l| (l, Score::default()))
        .collect();
    for (e, p) in tests.iter().zip(s.labels(&rows)) {
        for l in Label::ALL {
            let truth: Value = e.labels.get(l);
            if let Some(score) = labels.get_mut(&l) {
                score.add(p.get(l), &truth);
            }
        }
    }

    let mut by_account: BTreeMap<Uuid, Vec<Row>> = BTreeMap::new();
    for (e, r) in tests.iter().zip(rows) {
        by_account
            .entry(e.account)
            .or_default()
            .push(Row { account: None, ..r });
    }
    let mut accounts = Score::default();
    for (account, rows) in by_account {
        let kind = history.accounts.iter().find(|a| a.id == account).map(|a| {
            if a.kind == "credit_card" {
                StatementKind::CreditCard
            } else {
                StatementKind::Bank
            }
        });
        let guess = s.account(&Statement {
            file_name: None,
            kind,
            rows,
        });
        accounts.add(guess.as_ref(), &account);
    }

    Report {
        start,
        end,
        suggester: s.identity().clone(),
        trained_on: past.examples.len(),
        labels,
        accounts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_count_by_confidence() {
        let mut s = Score::default();
        let g = |value: u8, confidence: u16| Guess {
            value,
            confidence,
            reason: String::new(),
        };
        s.add(Some(&g(1, 950)), &1);
        s.add(Some(&g(1, 1000)), &2);
        s.add(Some(&g(1, 420)), &1);
        s.add(None, &1);
        assert_eq!(
            (
                s.asked,
                s.guessed,
                s.correct,
                s.confident,
                s.confident_correct
            ),
            (4, 3, 2, 2, 1)
        );
        assert_eq!((s.bins[9], s.bins[4]), ((2, 1), (1, 1)));
    }
}
