//! Remote AI (design §8, docs/ai.md): a decision model, asked
//! multiple-choice questions whose options are the ledger's own values, as
//! a [`suggest::Suggester`]. It runs after rules and local AI, so it's asked
//! only about what they left unsure, with what they settled as facts.
//!
//! What it's shown is computed from the ledger on each request
//! ([`context`]): the row, its account and how that's used, the most similar
//! past transactions (from the level before, [`Suggester::similar`]), and
//! each option described by how it's been used. Never an identifier: bank
//! texts go normalized (runs of 5+ digits replaced), account numbers as
//! their last four only.
//!
//! The questions are versioned ([`QUESTIONS`]): a change of wording,
//! options or state is a new version, recorded with every suggestion and
//! measured by the backtest.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use outbound::decision::{Decide, Decision, Error, Question};
use rust_decimal::Decimal;
use suggest::{Guess, History, Identity, Kind, Proposal, Row, Statement, Suggester, Value};
use uuid::Uuid;

mod context;
mod prompts;
#[cfg(test)]
mod tests;

pub use context::{AccountInfo, Catalog, PartyInfo};
use context::{Context, Options};

/// The version of the questions: their wording, options and state.
pub const QUESTIONS: u32 = 2;

/// The configured service.
pub struct Remote {
    decide: Arc<dyn Decide>,
    concurrency: usize,
}

impl Remote {
    /// `concurrency`: requests in flight at once.
    #[must_use]
    pub fn new(decide: Arc<dyn Decide>, concurrency: usize) -> Self {
        Self {
            decide,
            concurrency: concurrency.max(1),
        }
    }

    /// A suggester for the ledger as it is: `history` and `catalog` describe
    /// it, `evidence` (the level before, usually local AI) finds each row's
    /// similar past transactions.
    #[must_use]
    pub fn level(
        &self,
        history: &History,
        catalog: &Catalog,
        evidence: Option<Arc<dyn Suggester>>,
    ) -> Level {
        let deployment = self.decide.deployment().to_owned();
        Level {
            decide: self.decide.clone(),
            concurrency: self.concurrency,
            context: Context::new(history, catalog),
            evidence,
            fallback: identity(&deployment, &deployment),
            identity: OnceLock::new(),
            stopped: AtomicBool::new(false),
        }
    }
}

fn identity(deployment: &str, model: &str) -> Identity {
    Identity {
        kind: Kind::Remote,
        name: model.to_owned(),
        version: format!("q{QUESTIONS}"),
        details: serde_json::json!({
            "provider": "azure-foundry",
            "deployment": deployment,
            "model": model,
            "questions": QUESTIONS,
        }),
    }
}

/// The remote level, for one state of the ledger.
pub struct Level {
    decide: Arc<dyn Decide>,
    concurrency: usize,
    context: Context,
    evidence: Option<Arc<dyn Suggester>>,
    /// Until the service has named its model.
    fallback: Identity,
    identity: OnceLock<Identity>,
    /// Set when the service can't be used at all (a bad key, a wrong
    /// endpoint, unreachable): nothing more is asked.
    stopped: AtomicBool,
}

/// What became of one request.
enum Outcome {
    Answered(Decision),
    /// Nothing to ask.
    Skipped,
    /// The service refused this one (a filter); it steps aside for it.
    Refused,
    /// The service failed; nothing more is asked.
    Failed,
}

impl Level {
    fn ask(&self, state: &serde_json::Value, questions: &[Question]) -> Outcome {
        if questions.is_empty() || self.stopped.load(Ordering::Relaxed) {
            return Outcome::Skipped;
        }
        match self.decide.decide(state, questions) {
            Ok(d) => {
                let model = if d.model.is_empty() {
                    self.decide.deployment()
                } else {
                    &d.model
                };
                self.identity
                    .get_or_init(|| identity(self.decide.deployment(), model));
                Outcome::Answered(d)
            }
            Err(e @ Error::Refused { .. }) => {
                tracing::warn!(error = %e, "remote AI: a request was refused; it steps aside for it");
                Outcome::Refused
            }
            Err(e) => {
                if !self.stopped.swap(true, Ordering::Relaxed) {
                    tracing::warn!(error = %e, "remote AI failed; it's skipped for the rest of this request");
                }
                Outcome::Failed
            }
        }
    }

    /// The guess an answer makes, if any: an option that stands for a value,
    /// with its probability.
    fn guess<T: Clone>(
        &self,
        d: &Decision,
        q: &Question,
        values: &[(String, T)],
    ) -> Option<Guess<T>> {
        let a = d.answers.get(&q.name)?;
        let value = values.iter().find(|(k, _)| *k == a.choice)?.1.clone();
        let confidence = a.confidence();
        Some(Guess {
            value,
            confidence,
            reason: format!(
                "{}: {:.2}",
                self.identity().name,
                Decimal::new(i64::from(confidence), 3)
            ),
        })
    }

    fn row(&self, row: &Row) -> (Proposal, Outcome) {
        let similar = self
            .evidence
            .as_ref()
            .map(|e| e.similar(row, context::SIMILAR))
            .unwrap_or_default();
        let (state, asked) = self.context.row(row, &similar);
        let questions: Vec<Question> = asked.iter().map(|(_, o)| o.question.clone()).collect();
        let outcome = self.ask(&state, &questions);
        let mut p = Proposal::default();
        if let Outcome::Answered(d) = &outcome {
            for (label, o) in &asked {
                p.set(*label, self.guess::<Value>(d, &o.question, &o.values));
            }
        }
        (p, outcome)
    }
}

impl Suggester for Level {
    fn identity(&self) -> &Identity {
        self.identity.get().unwrap_or(&self.fallback)
    }

    fn labels(&self, rows: &[Row]) -> Vec<Proposal> {
        let next = AtomicUsize::new(0);
        let done: Mutex<Vec<(usize, Proposal, Outcome)>> = Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..self.concurrency.min(rows.len()) {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(r) = rows.get(i) else { break };
                        let (p, o) = self.row(r);
                        if let Ok(mut d) = done.lock() {
                            d.push((i, p, o));
                        }
                    }
                });
            }
        });
        let mut done = done.into_inner().unwrap_or_default();
        done.sort_by_key(|d| d.0);
        let count = |f: fn(&Outcome) -> bool| done.iter().filter(|d| f(&d.2)).count();
        let (answered, refused, failed) = (
            count(|o| matches!(o, Outcome::Answered(_))),
            count(|o| matches!(o, Outcome::Refused)),
            count(|o| matches!(o, Outcome::Failed)),
        );
        if answered + refused + failed > 0 {
            tracing::info!(rows = rows.len(), answered, refused, failed, "remote AI");
        }
        let mut out = vec![Proposal::default(); rows.len()];
        for (i, p, _) in done {
            out[i] = p;
        }
        out
    }

    fn account(&self, statement: &Statement) -> Option<Guess<Uuid>> {
        let (state, Options { question, values }) = self.context.statement(statement)?;
        match self.ask(&state, std::slice::from_ref(&question)) {
            Outcome::Answered(d) => self.guess(&d, &question, &values),
            _ => None,
        }
    }
}
