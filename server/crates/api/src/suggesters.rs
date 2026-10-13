//! `/api/v1/suggesters` (design §8, docs/ai.md): measuring local AI on the
//! ledger's own past.

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
use crate::suggestions::history;

/// Most months one backtest covers.
const MAX_MONTHS: usize = 24;

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
    /// The local method to measure, as configured (`lexical-knn-v1`); the
    /// server's own if omitted.
    method: Option<String>,
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
}

impl From<backtest::Score> for Score {
    fn from(s: backtest::Score) -> Self {
        Self {
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

/// One month: the method trained on everything before it, then asked about
/// its transactions.
#[derive(Debug, Serialize, ToSchema)]
pub struct BacktestMonth {
    #[schema(value_type = String)]
    month: Month,
    /// What the trained suggester records as its details (e.g. its data's
    /// snapshot).
    #[schema(value_type = Object)]
    suggester: serde_json::Value,
    /// Labeled transactions it learned from.
    trained_on: u64,
    /// The month's transactions, each asked its labels with its account
    /// known.
    labels: Vec<LabelScore>,
    /// Each account's transactions in the month, as one statement asked its
    /// account.
    accounts: Score,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Backtest {
    /// The method measured (`<name>-v<version>`).
    method: String,
    months: Vec<BacktestMonth>,
}

/// Measure a local AI method on the ledger's own past (docs/ai.md). For each
/// month, the method learns from everything before it, then suggests labels
/// for the month's transactions (between one of your accounts and the
/// outside) as if they were being imported, and an account for each
/// account's transactions; its guesses are compared with the ledger. Reads
/// only; nothing is stored.
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
    if to < p.from || months.len() > MAX_MONTHS {
        return Err(Problem::invalid(format!(
            "from..to must be 1-{MAX_MONTHS} months"
        )));
    }
    let mut db = state.pool.acquire().await?;
    let history = history(&mut db).await?;
    drop(db);
    let reports = tokio::task::spawn_blocking(move || {
        months
            .into_iter()
            .map(|m| {
                (
                    m,
                    backtest::run(method, &history, m.first_day(), m.next().first_day()),
                )
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Problem::internal())?;
    Ok(axum::Json(Backtest {
        method: method.id(),
        months: reports
            .into_iter()
            .map(|(month, r)| BacktestMonth {
                month,
                suggester: r.suggester.details,
                trained_on: u64::try_from(r.trained_on).unwrap_or(u64::MAX),
                labels: r
                    .labels
                    .into_iter()
                    .map(|(l, score)| LabelScore {
                        label: Label::from(l),
                        score: score.into(),
                    })
                    .collect(),
                accounts: r.accounts.into(),
            })
            .collect(),
    }))
}
