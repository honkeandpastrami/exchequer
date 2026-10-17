//! `/api/v1/suggesters` (design §8, docs/ai.md): measuring local AI, and
//! remote AI after it, on the ledger's own past.
#![allow(
    clippy::disallowed_types,
    reason = "backtest measures are fractions, not money; never stored"
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::State;
use domain::Month;
use serde::{Deserialize, Serialize};
use suggest::backtest::{self, BINS};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::imports::Label;
use crate::problem::{Problem, Query};
use crate::suggestions::{catalog, history};

/// Most months one backtest covers.
const MAX_MONTHS: usize = 24;
/// Most months one backtest with remote AI covers: each row local AI is
/// unsure of is a request.
const MAX_REMOTE_MONTHS: usize = 3;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(backtest))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BacktestParams {
    /// First month, `YYYY-MM`.
    #[param(value_type = String)]
    from: Month,
    /// Last month, `YYYY-MM`; `from` if omitted (at most 24 months in all).
    #[param(value_type = Option<String>)]
    to: Option<Month>,
    /// The local method to measure, as configured (`lexical-knn-v2`); the
    /// server's own if omitted.
    method: Option<String>,
    /// Also ask remote AI, as configured, about what the method leaves
    /// unsure (at most 3 months). Its requests are real ones, billed.
    #[serde(default)]
    remote: bool,
}

/// Guesses by confidence: those from `from` up to the next bin's `from`
/// (the last bin includes 1000).
#[derive(Debug, Serialize, ToSchema)]
pub struct Bin {
    /// Per mille.
    from: u16,
    guesses: u32,
    right: u32,
}

/// The usual measures, as fractions (null without a denominator). A row
/// not guessed counts against recall and accuracy, not precision; each row
/// has one true value, so accuracy is the micro recall. Macro means are
/// over the classes (values, or accounts) either true or guessed, a class
/// never guessed or never true counting 0.
#[derive(Debug, Serialize, ToSchema)]
pub struct Metrics {
    /// Rows (or statements).
    samples: u32,
    /// Guessed, of the samples.
    coverage: Option<f64>,
    /// Right, of the guesses.
    precision: Option<f64>,
    /// Right, of the samples.
    recall: Option<f64>,
    f1: Option<f64>,
    accuracy: Option<f64>,
    macro_precision: Option<f64>,
    macro_recall: Option<f64>,
    macro_f1: Option<f64>,
}

impl From<backtest::Metrics> for Metrics {
    fn from(m: backtest::Metrics) -> Self {
        Self {
            samples: m.samples,
            coverage: m.coverage,
            precision: m.precision,
            recall: m.recall,
            f1: m.f1,
            accuracy: m.accuracy,
            macro_precision: m.macro_precision,
            macro_recall: m.macro_recall,
            macro_f1: m.macro_f1,
        }
    }
}

/// Guesses against what the ledger says.
#[derive(Debug, Serialize, ToSchema)]
pub struct Score {
    /// Rows (or statements) asked about.
    asked: u32,
    /// Of them, those with a guess.
    guessed: u32,
    /// Of them, those guessed right.
    right: u32,
    /// Guesses at 900 or more, which commit would take.
    confident: u32,
    confident_right: u32,
    bins: Vec<Bin>,
    /// Over all guesses.
    all: Metrics,
    /// Over guesses at 900 or more only.
    confident_only: Metrics,
}

impl From<backtest::Score> for Score {
    fn from(s: backtest::Score) -> Self {
        Self {
            all: s.metrics(false).into(),
            confident_only: s.metrics(true).into(),
            asked: s.asked,
            guessed: s.guessed,
            right: s.correct,
            confident: s.confident,
            confident_right: s.confident_correct,
            bins: (0u16..)
                .zip(s.bins)
                .take(BINS)
                .map(|(i, (guesses, right))| Bin {
                    from: i * 100,
                    guesses,
                    right,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LabelScore {
    label: Label,
    #[serde(flatten)]
    score: Score,
}

/// One level's share: of the rows (or statements), those whose final
/// suggestion it made, and how right they were.
#[derive(Debug, Serialize, ToSchema)]
pub struct BacktestLevel {
    /// `local` or `remote`.
    kind: String,
    name: String,
    version: String,
    /// What it records as its details (e.g. its data's snapshot).
    #[schema(value_type = Object)]
    details: serde_json::Value,
    labels: Vec<LabelScore>,
    accounts: Score,
}

fn label_scores(s: BTreeMap<suggest::Label, backtest::Score>) -> Vec<LabelScore> {
    s.into_iter()
        .map(|(l, score)| LabelScore {
            label: Label::from(l),
            score: score.into(),
        })
        .collect()
}

/// One month: the levels built from everything before it, then asked about
/// its transactions.
#[derive(Debug, Serialize, ToSchema)]
pub struct BacktestMonth {
    #[schema(value_type = String)]
    month: Month,
    /// Labeled transactions they learned from.
    trained_on: u64,
    /// The month's transactions, each asked its labels with its account
    /// known: the final suggestions, whichever level made them.
    labels: Vec<LabelScore>,
    /// Each account's transactions in the month, as one statement asked its
    /// account.
    accounts: Score,
    /// Per level, in order.
    levels: Vec<BacktestLevel>,
}

/// All months together.
#[derive(Debug, Serialize, ToSchema)]
pub struct BacktestTotal {
    labels: Vec<LabelScore>,
    accounts: Score,
    /// Per level, in order.
    levels: Vec<BacktestLevel>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Backtest {
    /// The method measured (`<name>-v<version>`).
    method: String,
    /// Whether remote AI was asked too.
    remote: bool,
    months: Vec<BacktestMonth>,
    total: BacktestTotal,
}

fn level(l: backtest::Level) -> BacktestLevel {
    BacktestLevel {
        kind: l.suggester.kind.as_str().to_owned(),
        name: l.suggester.name,
        version: l.suggester.version,
        details: l.suggester.details,
        labels: label_scores(l.labels),
        accounts: l.accounts.into(),
    }
}

/// `into`'s scores plus `from`'s.
fn merge(
    into: &mut BTreeMap<suggest::Label, backtest::Score>,
    from: &BTreeMap<suggest::Label, backtest::Score>,
) {
    for (l, s) in from {
        into.entry(*l).or_default().merge(s);
    }
}

/// Measure a local AI method, alone or with remote AI after it, on the
/// ledger's own past (docs/ai.md). For each month, the method learns from
/// everything before it, then suggests labels for the month's transactions
/// (between one of your accounts and the outside) as if they were being
/// imported, and an account for each account's transactions; remote AI, if
/// asked, is shown what's before the month and asked about what the method
/// left unsure. The guesses are compared with the ledger. Reads only;
/// nothing is stored.
#[utoipa::path(get, path = "/suggesters/backtest", tag = "suggesters", security(("bearer" = [])),
    params(BacktestParams),
    responses((status = OK, body = Backtest),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn backtest(
    State(state): State<AppState>,
    Query(p): Query<BacktestParams>,
) -> Result<axum::Json<Backtest>, Problem> {
    let method = match p.method.as_deref() {
        Some(id) => heuristics::method(id).ok_or_else(|| {
            let known: Vec<String> = heuristics::METHODS.iter().map(|m| m.id()).collect();
            Problem::invalid(format!(
                "method: no such method (known: {})",
                known.join(", ")
            ))
        })?,
        None => state
            .suggestions
            .local()
            .ok_or_else(|| Problem::invalid("method: local AI is off; name a method"))?,
    };
    let to = p.to.unwrap_or(p.from);
    let mut months = vec![p.from];
    while months.last().is_some_and(|m| *m < to) && months.len() <= MAX_MONTHS {
        months.push(months[months.len() - 1].next());
    }
    let max = if p.remote {
        MAX_REMOTE_MONTHS
    } else {
        MAX_MONTHS
    };
    if to < p.from || months.len() > max {
        return Err(Problem::invalid(format!("from..to must be 1-{max} months")));
    }
    let remote = match (p.remote, state.suggestions.remote()) {
        (false, _) => None,
        (true, Some(r)) => Some(r.clone()),
        (true, None) => return Err(Problem::invalid("remote: remote AI isn't configured")),
    };
    let mut db = state.pool.acquire().await?;
    let history = history(&mut db).await?;
    let catalog = match remote {
        Some(_) => Some(catalog(&mut db).await?),
        None => None,
    };
    drop(db);
    let reports = tokio::task::spawn_blocking(move || {
        let build = |past: &suggest::History| -> Vec<Arc<dyn suggest::Suggester>> {
            let local: Arc<dyn suggest::Suggester> = Arc::from(method.train(past));
            let mut levels = vec![local.clone()];
            if let (Some(r), Some(c)) = (&remote, &catalog) {
                levels.push(Arc::new(r.level(past, c, Some(local))));
            }
            levels
        };
        months
            .into_iter()
            .map(|m| {
                (
                    m,
                    backtest::run_stack(&build, &history, m.first_day(), m.next().first_day()),
                )
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Problem::internal())?;
    let mut labels = BTreeMap::new();
    let mut accounts = backtest::Score::default();
    let mut levels: Vec<backtest::Level> = Vec::new();
    for (_, r) in &reports {
        merge(&mut labels, &r.labels);
        accounts.merge(&r.accounts);
        for (i, l) in r.levels.iter().enumerate() {
            match levels.get_mut(i) {
                Some(t) => {
                    merge(&mut t.labels, &l.labels);
                    t.accounts.merge(&l.accounts);
                    // The latest month's identity.
                    t.suggester = l.suggester.clone();
                }
                None => levels.push(l.clone()),
            }
        }
    }
    let total = BacktestTotal {
        labels: label_scores(labels),
        accounts: accounts.into(),
        levels: levels.into_iter().map(level).collect(),
    };
    Ok(axum::Json(Backtest {
        method: method.id(),
        remote: p.remote,
        months: reports
            .into_iter()
            .map(|(month, r)| BacktestMonth {
                month,
                trained_on: u64::try_from(r.trained_on).unwrap_or(u64::MAX),
                labels: label_scores(r.labels),
                accounts: r.accounts.into(),
                levels: r.levels.into_iter().map(level).collect(),
            })
            .collect(),
        total,
    }))
}
