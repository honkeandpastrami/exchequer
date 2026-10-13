//! Running the suggestion stack on staged imports (design §8, docs/ai.md).
//! The levels are suggesters ([`suggest::Suggester`]), in order: the active
//! rules script, then the configured local method, trained on the ledger as
//! it is and kept until the ledger changes.

use std::sync::{Arc, Mutex};

use sqlx::PgConnection;
use store::imports::{AccountSuggestion, RowRow, StatementRow, SuggestionRow};
use suggest::{Facts, History, Label, Method, Row, Statement, StatementKind, Suggester, stack};
use uuid::Uuid;

use crate::problem::Problem;

/// A trained suggester, and what it was trained on (the method's ID and
/// the history's snapshot).
type Trained = Option<(String, Arc<dyn Suggester>)>;

/// The suggestion stack's configuration, and the trained local method.
#[derive(Clone)]
pub struct Suggestions {
    local: Option<&'static dyn Method>,
    /// The local method trained on a snapshot of the history.
    trained: Arc<Mutex<Trained>>,
}

impl Default for Suggestions {
    /// The default local method.
    fn default() -> Self {
        Self::new(heuristics::METHODS.first().copied())
    }
}

impl Suggestions {
    /// `local`: the local AI method, or none to suggest nothing locally.
    #[must_use]
    pub fn new(local: Option<&'static dyn Method>) -> Self {
        Self {
            local,
            trained: Arc::default(),
        }
    }

    pub(crate) fn local(&self) -> Option<&'static dyn Method> {
        self.local
    }

    /// The levels, in order, trained on the ledger as it is now.
    pub(crate) async fn levels(
        &self,
        db: &mut PgConnection,
    ) -> Result<Vec<Arc<dyn Suggester>>, Problem> {
        let mut levels: Vec<Arc<dyn Suggester>> = Vec::new();
        if let Some(r) = store::rules::active(&mut *db).await?
            && !r.source.trim().is_empty()
        {
            // Validated when uploaded; a name deleted since makes it invalid
            // until it's fixed, and rules sit out meanwhile.
            match rules::Script::compile(r.version, &r.source, &names(db).await?) {
                Ok(s) => levels.push(Arc::new(s)),
                Err(e) => tracing::warn!(
                    version = r.version,
                    errors = e.len(),
                    "the rules script no longer validates; rules are skipped"
                ),
            }
        }
        let Some(method) = self.local else {
            return Ok(levels);
        };
        let history = history(db).await?;
        let trained = self.trained.clone();
        let model = tokio::task::spawn_blocking(move || {
            let key = format!("{}/{}", method.id(), history.snapshot());
            if let Ok(t) = trained.lock()
                && let Some((k, m)) = t.as_ref()
                && *k == key
            {
                return m.clone();
            }
            let m: Arc<dyn Suggester> = Arc::from(method.train(&history));
            if let Ok(mut t) = trained.lock() {
                *t = Some((key, m.clone()));
            }
            m
        })
        .await
        .map_err(|_| Problem::internal())?;
        levels.push(model);
        Ok(levels)
    }
}

/// The ledger's labeled history (store::history).
pub(crate) async fn history(db: &mut PgConnection) -> Result<History, Problem> {
    let accounts = store::history::accounts(&mut *db)
        .await?
        .into_iter()
        .map(|a| suggest::Account {
            id: a.id,
            kind: a.kind,
            owner: a.owner_id,
            archived: a.archived,
        })
        .collect();
    let examples = store::history::examples(&mut *db)
        .await?
        .into_iter()
        .map(|e| suggest::Example {
            transaction: e.transaction_id,
            account: e.account_id,
            date: e.date,
            amount: e.amount,
            description: e.description,
            labels: suggest::Labels {
                counterparty: e.counterparty_id,
                category: e.category_id,
                purpose: e.purpose_id,
            },
        })
        .collect();
    Ok(History { accounts, examples })
}

/// Every name a rules script may use.
pub(crate) async fn names(db: &mut PgConnection) -> Result<rules::Names, Problem> {
    let n = store::rules::names(db).await?;
    Ok(rules::Names {
        accounts: n
            .accounts
            .into_iter()
            .map(|(id, name, owner)| rules::AccountName { id, name, owner })
            .collect(),
        counterparties: n.counterparties,
        categories: n.categories,
        parties: n.parties,
    })
}

/// The suggester's ID, recorded the first time it's seen.
async fn suggester_id(db: &mut PgConnection, s: &dyn Suggester) -> Result<Uuid, Problem> {
    let i = s.identity();
    Ok(store::imports::suggester(db, i.kind.as_str(), &i.name, &i.version, &i.details).await?)
}

/// The facts a row brings: the labels you chose.
fn facts(r: &RowRow) -> Facts {
    let chosen: Vec<Label> = Label::ALL
        .into_iter()
        .filter(|l| r.chosen.iter().any(|c| c == l.as_str()))
        .collect();
    stack::facts(
        &chosen,
        &suggest::Labels {
            counterparty: r.counterparty_id,
            category: r.category_id,
            purpose: r.purpose_id,
        },
    )
}

fn row(r: &RowRow, account: Option<Uuid>) -> Row {
    Row {
        account,
        date: r.date,
        amount: r.amount,
        description: r.description.clone(),
        raw: r.raw.clone(),
        facts: facts(r),
    }
}

fn per_mille(c: u16) -> i16 {
    i16::try_from(c.min(1000)).unwrap_or(1000)
}

/// Suggests the account of a statement that has none of its own, replacing
/// what was suggested before.
pub(crate) async fn account(
    db: &mut PgConnection,
    levels: &[Arc<dyn Suggester>],
    import_id: Uuid,
    file_name: Option<&str>,
    s: &StatementRow,
) -> Result<(), Problem> {
    if s.account_id.is_some() {
        return Ok(());
    }
    let rows =
        store::imports::rows(&mut *db, import_id, Some(s.position), None, None, i64::MAX).await?;
    let statement = Statement {
        file_name: file_name.map(str::to_owned),
        kind: s.kind.as_deref().map(|k| {
            if k == "credit_card" {
                StatementKind::CreditCard
            } else {
                StatementKind::Bank
            }
        }),
        rows: rows.iter().map(|r| row(r, None)).collect(),
    };
    let owned = levels.to_vec();
    let pick = tokio::task::spawn_blocking(move || {
        let refs: Vec<&dyn Suggester> = owned.iter().map(AsRef::as_ref).collect();
        stack::account(&refs, &statement)
    })
    .await
    .map_err(|_| Problem::internal())?;
    let suggestion = match pick {
        Some(p) => Some(AccountSuggestion {
            account_id: p.guess.value,
            suggester_id: suggester_id(db, levels[p.level].as_ref()).await?,
            confidence: per_mille(p.guess.confidence),
            reason: p.guess.reason,
        }),
        None => None,
    };
    store::imports::set_account_suggestion(&mut *db, import_id, s.position, suggestion.as_ref())
        .await?;
    Ok(())
}

/// Suggests the labels of a statement's rows, replacing what was suggested
/// before; labels you chose are facts, never suggested.
pub(crate) async fn labels(
    db: &mut PgConnection,
    levels: &[Arc<dyn Suggester>],
    import_id: Uuid,
    statement: i16,
) -> Result<(), Problem> {
    let (_, s) = store::imports::statements(&mut *db, &[import_id])
        .await?
        .into_iter()
        .find(|(_, s)| s.position == statement)
        .ok_or_else(Problem::internal)?;
    let account = s.effective_account();
    let staged =
        store::imports::rows(&mut *db, import_id, Some(statement), None, None, i64::MAX).await?;
    let rows: Vec<Row> = staged.iter().map(|r| row(r, account)).collect();
    let owned = levels.to_vec();
    let picks = tokio::task::spawn_blocking(move || {
        let refs: Vec<&dyn Suggester> = owned.iter().map(AsRef::as_ref).collect();
        stack::labels(&refs, &rows)
    })
    .await
    .map_err(|_| Problem::internal())?;
    let mut ids: Vec<Option<Uuid>> = vec![None; levels.len()];
    let mut items = Vec::new();
    for (r, p) in staged.iter().zip(picks) {
        for l in Label::ALL {
            let Some(pick) = p.get(l) else { continue };
            let id = match ids[pick.level] {
                Some(id) => id,
                None => {
                    let id = suggester_id(db, levels[pick.level].as_ref()).await?;
                    ids[pick.level] = Some(id);
                    id
                }
            };
            items.push(SuggestionRow {
                statement,
                position: r.position,
                label: l.as_str().to_owned(),
                value: pick.guess.value,
                suggester_id: id,
                confidence: per_mille(pick.guess.confidence),
                reason: pick.guess.reason.chars().take(500).collect(),
            });
        }
    }
    store::imports::set_suggestions(db, import_id, statement, &items).await?;
    Ok(())
}
