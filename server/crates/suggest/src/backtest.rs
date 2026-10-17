//! Measures suggesters on the ledger's own past (design §8): built from
//! everything before a period, they suggest the period's transactions as if
//! they were being imported, and their guesses are compared with what the
//! ledger says. Any local method can be measured, alone or with the levels
//! after it (remote AI), so two can be compared on the same data before one
//! replaces the other.
#![allow(
    clippy::disallowed_types,
    clippy::float_arithmetic,
    reason = "measures are fractions, not money; never stored"
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use time::Date;
use uuid::Uuid;

use crate::{
    CONFIDENT, Example, Facts, Guess, History, Identity, Label, Method, Row, Statement,
    StatementKind, Suggester, Value, labeled, stack,
};

/// How many confidence bins: 0-99, 100-199, …, 900-1000 per mille.
pub const BINS: usize = 10;

/// One class (a label's value, or an account): how often it's the truth,
/// and how often it's guessed and guessed right, among all guesses `[0]`
/// and confident ones `[1]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Class {
    pub actual: u32,
    pub guessed: [u32; 2],
    pub right: [u32; 2],
}

/// The usual measures of a classifier that may not answer, as fractions
/// (none without a denominator). A row not guessed counts against recall
/// and accuracy, not precision. Each row has one true value, so accuracy is
/// the micro recall. Macro means are over the classes either true or
/// guessed, a class never guessed (or never true) counting 0.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Metrics {
    /// Rows (or statements).
    pub samples: u32,
    /// Guessed, of the samples.
    pub coverage: Option<f64>,
    /// Right, of the guesses.
    pub precision: Option<f64>,
    /// Right, of the samples.
    pub recall: Option<f64>,
    pub f1: Option<f64>,
    pub accuracy: Option<f64>,
    pub macro_precision: Option<f64>,
    pub macro_recall: Option<f64>,
    pub macro_f1: Option<f64>,
}
fn ratio(n: f64, d: f64) -> Option<f64> {
    (d > 0.0).then(|| n / d)
}

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
    pub classes: BTreeMap<Value, Class>,
}

impl Score {
    /// `class` names a value as a class.
    fn add<T: PartialEq>(
        &mut self,
        guess: Option<&Guess<T>>,
        truth: &T,
        class: impl Fn(&T) -> Value,
    ) {
        self.asked += 1;
        self.classes.entry(class(truth)).or_default().actual += 1;
        let Some(g) = guess else { return };
        let right = g.value == *truth;
        let confident = g.confidence >= CONFIDENT;
        self.guessed += 1;
        self.correct += u32::from(right);
        if confident {
            self.confident += 1;
            self.confident_correct += u32::from(right);
        }
        let bin = &mut self.bins[usize::from(g.confidence / 100).min(BINS - 1)];
        bin.0 += 1;
        bin.1 += u32::from(right);
        let c = self.classes.entry(class(&g.value)).or_default();
        for i in 0..=usize::from(confident) {
            c.guessed[i] += 1;
            c.right[i] += u32::from(right);
        }
    }

    /// Adds `other`'s counts, e.g. another month's.
    pub fn merge(&mut self, other: &Score) {
        self.asked += other.asked;
        self.guessed += other.guessed;
        self.correct += other.correct;
        self.confident += other.confident;
        self.confident_correct += other.confident_correct;
        for (b, o) in self.bins.iter_mut().zip(other.bins) {
            b.0 += o.0;
            b.1 += o.1;
        }
        for (v, o) in &other.classes {
            let c = self.classes.entry(*v).or_default();
            c.actual += o.actual;
            for i in 0..2 {
                c.guessed[i] += o.guessed[i];
                c.right[i] += o.right[i];
            }
        }
    }

    /// The measures over all guesses, or only confident ones.
    #[must_use]
    pub fn metrics(&self, confident: bool) -> Metrics {
        let (guessed, right) = if confident {
            (self.confident, self.confident_correct)
        } else {
            (self.guessed, self.correct)
        };
        let (asked, guessed, right) = (f64::from(self.asked), f64::from(guessed), f64::from(right));
        let i = usize::from(confident);
        let (mut p, mut r, mut f, mut n) = (0.0, 0.0, 0.0, 0.0);
        for c in self.classes.values() {
            let (actual, g, tp) = (
                f64::from(c.actual),
                f64::from(c.guessed[i]),
                f64::from(c.right[i]),
            );
            if actual + g == 0.0 {
                continue;
            }
            n += 1.0;
            p += ratio(tp, g).unwrap_or(0.0);
            r += ratio(tp, actual).unwrap_or(0.0);
            f += 2.0 * tp / (g + actual);
        }
        Metrics {
            samples: self.asked,
            coverage: ratio(guessed, asked),
            precision: ratio(right, guessed),
            recall: ratio(right, asked),
            f1: ratio(2.0 * right, guessed + asked),
            accuracy: ratio(right, asked),
            macro_precision: ratio(p, n),
            macro_recall: ratio(r, n),
            macro_f1: ratio(f, n),
        }
    }
}

/// What one level's picks scored: of the rows (or statements) asked, those
/// whose final suggestion it made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub suggester: Identity,
    pub labels: BTreeMap<Label, Score>,
    pub accounts: Score,
}

/// One period's results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The period: `start <= date < end`.
    pub start: Date,
    pub end: Date,
    /// How many labeled transactions the levels learned from.
    pub trained_on: usize,
    /// The final suggestions, whichever level made them.
    pub labels: BTreeMap<Label, Score>,
    /// Each account's transactions in the period, as one statement without
    /// an account.
    pub accounts: Score,
    /// Per level, in order.
    pub levels: Vec<Level>,
}

fn scores() -> BTreeMap<Label, Score> {
    Label::ALL
        .into_iter()
        .map(|l| (l, Score::default()))
        .collect()
}

/// Trains `method` on `history` before `start`, then suggests labels for
/// each transaction dated in `[start, end)` (its account known, nothing
/// settled) and an account for each account's transactions in the period.
#[must_use]
pub fn run(method: &dyn Method, history: &History, start: Date, end: Date) -> Report {
    run_stack(
        &|past: &History| vec![Arc::from(method.train(past))],
        history,
        start,
        end,
    )
}

/// As [`run`], for a stack of levels `build` makes from the history before
/// `start`; each later level is asked what the earlier ones left unsure.
#[must_use]
pub fn run_stack(
    build: &dyn Fn(&History) -> Vec<Arc<dyn Suggester>>,
    history: &History,
    start: Date,
    end: Date,
) -> Report {
    let past = history.before(start);
    let levels = build(&past);
    let refs: Vec<&dyn Suggester> = levels.iter().map(AsRef::as_ref).collect();
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
            // As if each period's rows were staged together.
            transfer_candidates: history.transfer_candidates(e),
            duplicates: None,
        })
        .collect();
    let mut labels = scores();
    let mut by_level: Vec<Level> = levels
        .iter()
        .map(|s| Level {
            suggester: s.identity().clone(),
            labels: scores(),
            accounts: Score::default(),
        })
        .collect();
    for (e, p) in tests
        .iter()
        .zip(stack::labels(&refs, &rows, &history.accounts))
    {
        // A transfer's category and purpose don't apply: not asked.
        let transfer = !labeled(
            &history.accounts,
            Some(e.account),
            e.amount,
            e.labels.counterparty,
        );
        for l in Label::ALL {
            if transfer && l != Label::Counterparty {
                continue;
            }
            let truth: Value = e.labels.get(l);
            let pick = p.get(l);
            if let Some(score) = labels.get_mut(&l) {
                score.add(pick.map(|p| &p.guess), &truth, |v| *v);
            }
            for (i, level) in by_level.iter_mut().enumerate() {
                if let Some(score) = level.labels.get_mut(&l) {
                    score.add(
                        pick.filter(|p| p.level == i).map(|p| &p.guess),
                        &truth,
                        |v| *v,
                    );
                }
            }
        }
    }

    let mut by_account: BTreeMap<Uuid, Vec<Row>> = BTreeMap::new();
    for (e, r) in tests.iter().zip(rows) {
        by_account.entry(e.account).or_default().push(Row {
            account: None,
            transfer_candidates: Vec::new(),
            duplicates: None,
            ..r
        });
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
        let pick = stack::account(
            &refs,
            &Statement {
                file_name: None,
                number: None,
                kind,
                rows,
            },
        );
        accounts.add(pick.as_ref().map(|p| &p.guess), &account, |a| {
            Value::Account(*a)
        });
        for (i, level) in by_level.iter_mut().enumerate() {
            level.accounts.add(
                pick.as_ref().filter(|p| p.level == i).map(|p| &p.guess),
                &account,
                |a| Value::Account(*a),
            );
        }
    }

    // A level's identity may be known only once it has answered.
    for (level, s) in by_level.iter_mut().zip(&levels) {
        level.suggester = s.identity().clone();
    }
    Report {
        start,
        end,
        trained_on: past.examples.len(),
        labels,
        accounts,
        levels: by_level,
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
        let class = |v: &u8| Value::Id(Uuid::from_u128(u128::from(*v)));
        s.add(Some(&g(1, 950)), &1, class);
        s.add(Some(&g(1, 1000)), &2, class);
        s.add(Some(&g(1, 420)), &1, class);
        s.add(None, &1, class);
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

    #[test]
    fn metrics_are_the_usual_ones() {
        // Truth a a a b, guesses a (sure) b (sure) a (unsure) and none.
        let mut s = Score::default();
        let g = |value: u8, confidence: u16| Guess {
            value,
            confidence,
            reason: String::new(),
        };
        let class = |v: &u8| Value::Id(Uuid::from_u128(u128::from(*v)));
        s.add(Some(&g(1, 950)), &1, class);
        s.add(Some(&g(2, 950)), &1, class);
        s.add(Some(&g(1, 500)), &1, class);
        s.add(None, &2, class);
        let pm = |x: Option<f64>| x.map(|x| (x * 1000.0).round() as i64);
        let all = s.metrics(false);
        // Micro: 2 right of 3 guesses, of 4 rows; F1 = 2·2 / (3 + 4).
        assert_eq!(
            [
                all.coverage,
                all.precision,
                all.recall,
                all.accuracy,
                all.f1
            ]
            .map(pm),
            [Some(750), Some(667), Some(500), Some(500), Some(571)]
        );
        // a: P 2/2, R 2/3, F1 0.8; b: P 0/1, R 0/1, F1 0.
        assert_eq!(
            [all.macro_precision, all.macro_recall, all.macro_f1].map(pm),
            [Some(500), Some(333), Some(400)]
        );
        // 1 right of 2 confident guesses.
        let sure = s.metrics(true);
        assert_eq!(
            [sure.coverage, sure.precision, sure.recall].map(pm),
            [Some(500), Some(500), Some(250)]
        );
        let mut twice = s.clone();
        twice.merge(&s);
        assert_eq!(twice.metrics(false).f1, all.f1);
        assert_eq!(twice.asked, 8);
    }
}
