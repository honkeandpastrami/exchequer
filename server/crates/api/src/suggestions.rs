//! Running the suggestion stack on staged imports (design §8, docs/ai.md).
//! The levels are suggesters ([`suggest::Suggester`]), in order: the active
//! rules script, then the configured local method, trained on the ledger as
//! it is and kept until the ledger changes, then remote AI if configured,
//! shown the local method's evidence.

use std::sync::{Arc, Mutex};

use sqlx::PgConnection;
use store::imports::{AccountSuggestion, RowRow, StatementRow, SuggestionRow};
use suggest::{
    Candidate, Facts, History, Label, Method, Row, Statement, StatementKind, Suggester, Value,
    stack,
};
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
    remote: Option<Arc<remote::Remote>>,
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
            remote: None,
        }
    }

    /// With remote AI, asked after the local method.
    #[must_use]
    pub fn with_remote(self, remote: Option<Arc<remote::Remote>>) -> Self {
        Self { remote, ..self }
    }

    pub(crate) fn local(&self) -> Option<&'static dyn Method> {
        self.local
    }

    pub(crate) fn remote(&self) -> Option<&Arc<remote::Remote>> {
        self.remote.as_ref()
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
        if self.local.is_none() && self.remote.is_none() {
            return Ok(levels);
        }
        let history = Arc::new(history(db).await?);
        let mut local: Option<Arc<dyn Suggester>> = None;
        if let Some(method) = self.local {
            let trained = self.trained.clone();
            let history = history.clone();
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
            levels.push(model.clone());
            local = Some(model);
        }
        if let Some(r) = self.remote.clone() {
            let catalog = catalog(db).await?;
            let level = tokio::task::spawn_blocking(move || r.level(&history, &catalog, local))
                .await
                .map_err(|_| Problem::internal())?;
            levels.push(Arc::new(level));
        }
        Ok(levels)
    }
}

/// Your accounts, as suggesters see them.
pub(crate) async fn accounts(db: &mut PgConnection) -> Result<Vec<suggest::Account>, Problem> {
    Ok(store::history::accounts(&mut *db)
        .await?
        .into_iter()
        .map(|a| suggest::Account {
            id: a.id,
            kind: a.kind,
            owner: a.owner_id,
            liquid: a.liquid,
            archived: a.archived,
        })
        .collect())
}

/// The ledger's labeled history (store::history).
pub(crate) async fn history(db: &mut PgConnection) -> Result<History, Problem> {
    let accounts = accounts(db).await?;
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
                counterparty: e
                    .transfer_account_id
                    .map_or_else(|| e.counterparty_id.into(), Value::Account),
                category: e.category_id.into(),
                purpose: e.purpose_id.into(),
            },
        })
        .collect();
    Ok(History { accounts, examples })
}

/// What remote AI may name: account numbers as their last four only.
pub(crate) async fn catalog(db: &mut PgConnection) -> Result<remote::Catalog, Problem> {
    let c = store::history::catalog(db).await?;
    Ok(remote::Catalog {
        accounts: c
            .accounts
            .into_iter()
            .map(|a| remote::AccountInfo {
                id: a.id,
                name: a.name,
                kind: a.kind,
                owner: a.owner_id,
                numbers: a.numbers.iter().map(|n| n.last4().to_owned()).collect(),
                archived: a.archived,
            })
            .collect(),
        counterparties: c.counterparties,
        categories: c.categories,
        parties: c
            .parties
            .into_iter()
            .map(|(id, name, kind)| remote::PartyInfo { id, name, kind })
            .collect(),
    })
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
            counterparty: r
                .transfer_account_id
                .map_or_else(|| r.counterparty_id.into(), Value::Account),
            category: r.category_id.into(),
            purpose: r.purpose_id.into(),
        },
    )
}

fn row(r: &RowRow, account: Option<Uuid>, transfer_candidates: Vec<Candidate>) -> Row {
    Row {
        account,
        date: r.date,
        amount: r.amount,
        description: r.description.clone(),
        raw: r.raw.clone(),
        facts: facts(r),
        transfer_candidates,
        duplicates: r.match_transaction_id,
    }
}

/// Each row's potential transfer matches (design §6.4): rows of every staged
/// import, in your other accounts, in the same commodity, with the opposite
/// amount, dated within a few days, that will become new transactions.
pub(crate) async fn candidates(
    db: &mut PgConnection,
    import_id: Uuid,
    statement: i16,
) -> Result<std::collections::HashMap<i32, Vec<Candidate>>, Problem> {
    let all = store::imports::matchable(&mut *db).await?;
    let Some(mine) = all
        .iter()
        .find(|r| r.import_id == import_id && r.statement == statement)
    else {
        return Ok(std::collections::HashMap::new());
    };
    let (Some(account), commodity) = (mine.account_id, mine.commodity_id) else {
        return Ok(std::collections::HashMap::new());
    };
    let others: Vec<(Uuid, time::Date, rust_decimal::Decimal)> = all
        .iter()
        .filter(|r| {
            r.commodity_id == commodity
                && matches!(r.status.as_str(), "new" | "transfer_match")
                && r.decision.as_deref() != Some("skip")
        })
        .filter_map(|r| Some((r.account_id?, r.date, r.amount)))
        .collect();
    Ok(all
        .iter()
        .filter(|r| r.import_id == import_id && r.statement == statement)
        .map(|r| {
            (
                r.position,
                suggest::candidates(account, r.date, r.amount, others.iter().copied()),
            )
        })
        .collect())
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
        number: s.account_number.as_ref().map(|n| n.last4().to_owned()),
        kind: s.kind.as_deref().map(|k| {
            if k == "credit_card" {
                StatementKind::CreditCard
            } else {
                StatementKind::Bank
            }
        }),
        rows: rows.iter().map(|r| row(r, None, Vec::new())).collect(),
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
/// before; labels you chose are facts, never suggested. With `keep`, a
/// confident suggestion stays too, and counts as a fact for the rest. True
/// if a suggestion changed.
pub(crate) async fn labels(
    db: &mut PgConnection,
    levels: &[Arc<dyn Suggester>],
    import_id: Uuid,
    statement: i16,
    keep: bool,
) -> Result<bool, Problem> {
    let (_, s) = store::imports::statements(&mut *db, &[import_id])
        .await?
        .into_iter()
        .find(|(_, s)| s.position == statement)
        .ok_or_else(Problem::internal)?;
    let account = s.effective_account();
    let staged =
        store::imports::rows(&mut *db, import_id, Some(statement), None, None, i64::MAX).await?;
    let keys: Vec<(i16, i32)> = staged.iter().map(|r| (statement, r.position)).collect();
    let before = store::imports::suggestions(&mut *db, import_id, &keys).await?;
    let kept: Vec<&SuggestionRow> = before
        .iter()
        .filter(|s| keep && s.confidence >= store::imports::CONFIDENT)
        .collect();
    let mut candidates = candidates(db, import_id, statement).await?;
    let rows: Vec<Row> = staged
        .iter()
        .map(|r| {
            let c = candidates.remove(&r.position).unwrap_or_default();
            let mut row = row(r, account, c);
            for k in kept.iter().filter(|k| k.position == r.position) {
                if let Some(l) = Label::ALL.into_iter().find(|l| l.as_str() == k.label) {
                    row.facts.set(l, kept_value(k, l));
                }
            }
            row
        })
        .collect();
    let accounts = accounts(db).await?;
    let owned = levels.to_vec();
    let picks = tokio::task::spawn_blocking(move || {
        let refs: Vec<&dyn Suggester> = owned.iter().map(AsRef::as_ref).collect();
        stack::labels(&refs, &rows, &accounts)
    })
    .await
    .map_err(|_| Problem::internal())?;
    let mut ids: Vec<Option<Uuid>> = vec![None; levels.len()];
    let mut items: Vec<SuggestionRow> = kept.into_iter().cloned().collect();
    for (r, p) in staged.iter().zip(picks) {
        for l in Label::ALL {
            let Some(pick) = p.get(l) else { continue };
            // A purpose names someone and a category something; "none" is
            // neither (design §4.1).
            if matches!(l, Label::Purpose | Label::Category) && pick.guess.value.id().is_none() {
                continue;
            }
            if items
                .iter()
                .any(|k| k.position == r.position && k.label == l.as_str())
            {
                continue;
            }
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
                value: pick.guess.value.id(),
                account_id: pick.guess.value.account(),
                suggester_id: id,
                confidence: per_mille(pick.guess.confidence),
                reason: pick.guess.reason.chars().take(500).collect(),
            });
        }
    }
    let sorted = |v: &[SuggestionRow]| {
        let mut v = v.to_vec();
        v.sort_by(|a, b| (a.position, &a.label).cmp(&(b.position, &b.label)));
        v
    };
    let changed = sorted(&before) != sorted(&items);
    if changed {
        store::imports::set_suggestions(db, import_id, statement, &items).await?;
    }
    Ok(changed)
}

/// A stored suggestion's value, as a fact.
fn kept_value(s: &SuggestionRow, label: Label) -> Value {
    match (label, s.account_id) {
        (Label::Counterparty, Some(a)) => Value::Account(a),
        _ => s.value.into(),
    }
}
