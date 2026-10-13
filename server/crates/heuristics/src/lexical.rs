//! `lexical-knn`, version 1: suggestions from the labeled transactions
//! whose bank text looks most like the row's (design §8, docs/ai.md).
//!
//! - Texts are compared by their [`normalize::key`]: character trigrams and
//!   words, weighted by TF-IDF, cosine similarity.
//! - **Nearest neighbors** vote on the whole combination of labels, each
//!   weighted by its similarity, then by the row's context: the same account
//!   (else the same owner), the same direction, the same or a similar
//!   amount, and recency. A label's score is the weight agreeing with the
//!   winning combination, out of all of it plus a prior, so a lone neighbor
//!   never scores high.
//! - **Settled labels** (facts): only examples that agree with them vote
//!   (all do if none agrees); with a settled counterparty, all of its
//!   examples are neighbors, however their text reads.
//! - **Alias memory:** rows with exactly this text, on this account if it
//!   has any: how many of the latest, in a row, had the same value.
//! - **The account** of a statement: the accounts its rows' neighbors are
//!   in, each row voting once.
//! - **Calibration:** on training, the model suggests the latest labeled
//!   transactions from the ones before each, and the account of each of the
//!   latest months' statements, and fits each score to how often it was
//!   right ([`Curve`]). A confidence of 0.9 then means about 9 in 10 right
//!   on this ledger's own data.

use std::collections::{BTreeMap, HashMap};

use rust_decimal::Decimal;
use suggest::{
    Account, Facts, Guess, History, Identity, Kind, Label, Labels, Method, Proposal, Row,
    Statement, StatementKind, Suggester, Value,
};
use time::Date;
use uuid::Uuid;

use crate::calibrate::Curve;
use crate::normalize;

pub struct LexicalKnn;

impl Method for LexicalKnn {
    fn name(&self) -> &'static str {
        "lexical-knn"
    }

    fn version(&self) -> &'static str {
        "1"
    }

    fn train(&self, history: &History) -> Box<dyn Suggester> {
        Box::new(Model::train(self, history))
    }
}

/// Neighbors that vote.
const K: usize = 15;
/// Less similar texts don't count as neighbors.
const MIN_SIMILARITY: f32 = 0.3;
/// Added to the votes' total weight: what a lone neighbor can't outweigh.
const PRIOR: f32 = 0.5;
/// The latest rows with the same text that alias memory looks at.
const ALIAS: usize = 5;
/// Rows of a statement that vote on its account.
const STATEMENT_ROWS: usize = 40;
/// Calibration: the latest labeled transactions suggested again.
const CALIBRATION_ROWS: usize = 2000;
/// Calibration: the latest statements (an account's month) suggested an
/// account, and how many of each one's rows vote.
const CALIBRATION_STATEMENTS: usize = 150;
const CALIBRATION_STATEMENT_ROWS: usize = 20;

/// A labeled transaction, ready to compare.
struct Ex {
    transaction: Uuid,
    account: Uuid,
    owner: Option<Uuid>,
    date: Date,
    amount: Decimal,
    key: String,
    /// The normalized text, for reasons.
    shown: String,
    labels: Labels,
}

struct Model {
    identity: Identity,
    accounts: HashMap<Uuid, Account>,
    ex: Vec<Ex>,
    /// Feature (a trigram or a word) → its index.
    features: HashMap<String, usize>,
    idf: Vec<f32>,
    /// Feature → the examples that have it, with its weight in each.
    postings: Vec<Vec<(u32, f32)>>,
    /// Key → its examples, oldest first.
    alias: HashMap<String, Vec<u32>>,
    /// Counterparty → its examples.
    by_counterparty: HashMap<Uuid, Vec<u32>>,
    neighbors_curve: BTreeMap<Label, Curve>,
    alias_curve: BTreeMap<Label, Curve>,
    account_curve: Curve,
}

/// A row as the model compares it.
struct Query<'a> {
    account: Option<Uuid>,
    owner: Option<Uuid>,
    date: Date,
    amount: Decimal,
    key: &'a str,
    facts: Facts,
    /// Only examples dated before this (calibration and backtests).
    before: Option<Date>,
}

/// Counts of a key's trigrams (with a space at each end) and words.
fn features(key: &str) -> HashMap<String, u32> {
    let mut out: HashMap<String, u32> = HashMap::new();
    if key.is_empty() {
        return out;
    }
    let chars: Vec<char> = format!(" {key} ").chars().collect();
    for w in chars.windows(3) {
        *out.entry(w.iter().collect()).or_default() += 1;
    }
    for w in key.split(' ') {
        *out.entry(format!("w:{w}")).or_default() += 1;
    }
    out
}

fn tf(count: u32) -> f32 {
    1.0 + (count as f32).ln()
}

/// How many digits `amount`'s whole part has: amounts alike in size.
fn magnitude(amount: Decimal) -> usize {
    amount.abs().trunc().to_string().len()
}

fn first_of_month(d: Date) -> Date {
    d.replace_day(1).unwrap_or(d)
}

/// What a neighbor said about one label.
struct Vote {
    value: Value,
    score: f32,
    /// The heaviest neighbor agreeing.
    like: u32,
    agree: usize,
    of: usize,
}

/// What alias memory says about one label.
struct Alias {
    value: Value,
    /// How many of the latest rows, in a row, had it (1 to [`ALIAS`]).
    streak: usize,
}

impl Model {
    fn train(method: &LexicalKnn, history: &History) -> Self {
        let accounts: HashMap<Uuid, Account> =
            history.accounts.iter().map(|a| (a.id, a.clone())).collect();
        let mut ex: Vec<Ex> = history
            .examples
            .iter()
            .map(|e| Ex {
                transaction: e.transaction,
                account: e.account,
                owner: accounts.get(&e.account).map(|a| a.owner),
                date: e.date,
                amount: e.amount,
                key: normalize::key(&e.description),
                shown: normalize::description(&e.description)
                    .chars()
                    .take(80)
                    .collect(),
                labels: e.labels,
            })
            .collect();
        ex.sort_by(|a, b| a.date.cmp(&b.date).then(a.transaction.cmp(&b.transaction)));

        let counts: Vec<HashMap<String, u32>> = ex.iter().map(|e| features(&e.key)).collect();
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut df: Vec<u32> = Vec::new();
        for c in &counts {
            for f in c.keys() {
                let i = *index.entry(f.clone()).or_insert_with(|| {
                    df.push(0);
                    df.len() - 1
                });
                df[i] += 1;
            }
        }
        let n = ex.len() as f32;
        let idf: Vec<f32> = df
            .iter()
            .map(|d| ((n + 1.0) / (*d as f32 + 1.0)).ln() + 1.0)
            .collect();
        let mut postings: Vec<Vec<(u32, f32)>> = vec![Vec::new(); df.len()];
        for (i, c) in counts.iter().enumerate() {
            let w: Vec<(usize, f32)> = c
                .iter()
                .map(|(f, k)| {
                    let j = index[f];
                    (j, tf(*k) * idf[j])
                })
                .collect();
            let norm = w.iter().map(|x| x.1 * x.1).sum::<f32>().sqrt();
            for (j, x) in w {
                #[expect(clippy::cast_possible_truncation, reason = "fewer than 2^32 examples")]
                postings[j].push((i as u32, x / norm));
            }
        }
        let mut alias: HashMap<String, Vec<u32>> = HashMap::new();
        for (i, e) in ex.iter().enumerate() {
            if !e.key.is_empty() {
                #[expect(clippy::cast_possible_truncation, reason = "fewer than 2^32 examples")]
                alias.entry(e.key.clone()).or_default().push(i as u32);
            }
        }

        let mut by_counterparty: HashMap<Uuid, Vec<u32>> = HashMap::new();
        for (i, e) in ex.iter().enumerate() {
            if let Some(c) = e.labels.counterparty {
                #[expect(clippy::cast_possible_truncation, reason = "fewer than 2^32 examples")]
                by_counterparty.entry(c).or_default().push(i as u32);
            }
        }

        let mut model = Self {
            identity: Identity {
                kind: Kind::Local,
                name: method.name().to_owned(),
                version: method.version().to_owned(),
                details: serde_json::json!({
                    "snapshot": history.snapshot(),
                    "examples": history.examples.len(),
                    "through": history.through().map(|d| d.to_string()),
                }),
            },
            accounts,
            ex,
            features: index,
            idf,
            postings,
            alias,
            by_counterparty,
            neighbors_curve: BTreeMap::new(),
            alias_curve: BTreeMap::new(),
            account_curve: Curve::default(),
        };
        model.calibrate();
        model
    }

    /// Fits the curves from the model's own guesses on its latest data.
    fn calibrate(&mut self) {
        let mut neighbors: BTreeMap<Label, Vec<(f32, bool)>> = BTreeMap::new();
        let mut alias: BTreeMap<Label, Vec<(f32, bool)>> = BTreeMap::new();
        let from = self.ex.len().saturating_sub(CALIBRATION_ROWS);
        for e in &self.ex[from..] {
            let q = Query {
                account: Some(e.account),
                owner: e.owner,
                date: e.date,
                amount: e.amount,
                key: &e.key,
                facts: Facts::default(),
                before: Some(e.date),
            };
            let (votes, aliases) = self.raw(&q);
            for l in Label::ALL {
                let truth = e.labels.get(l);
                if let Some(v) = &votes[&l] {
                    neighbors
                        .entry(l)
                        .or_default()
                        .push((v.score, v.value == truth));
                }
                if let Some(a) = &aliases[&l] {
                    alias
                        .entry(l)
                        .or_default()
                        .push((a.streak as f32, a.value == truth));
                }
            }
        }
        self.neighbors_curve = neighbors
            .into_iter()
            .map(|(l, p)| (l, Curve::fit(p)))
            .collect();
        self.alias_curve = alias.into_iter().map(|(l, p)| (l, Curve::fit(p))).collect();

        // An account's month of rows is one statement.
        let mut months: BTreeMap<(Date, Uuid), Vec<u32>> = BTreeMap::new();
        for (i, e) in self.ex.iter().enumerate() {
            if self.accounts.get(&e.account).is_some_and(|a| !a.archived) {
                #[expect(clippy::cast_possible_truncation, reason = "fewer than 2^32 examples")]
                months
                    .entry((first_of_month(e.date), e.account))
                    .or_default()
                    .push(i as u32);
            }
        }
        let mut points = Vec::new();
        for ((month, account), rows) in months
            .into_iter()
            .rev()
            .filter(|(_, rows)| rows.len() >= 3)
            .take(CALIBRATION_STATEMENTS)
        {
            let rows: Vec<&Ex> = spread(&rows, CALIBRATION_STATEMENT_ROWS)
                .into_iter()
                .map(|i| &self.ex[*i as usize])
                .collect();
            let kind = self.accounts.get(&account).map(|a| statement_kind(&a.kind));
            let queries: Vec<Query> = rows
                .iter()
                .map(|e| Query {
                    account: None,
                    owner: None,
                    date: e.date,
                    amount: e.amount,
                    key: &e.key,
                    facts: Facts::default(),
                    before: Some(month),
                })
                .collect();
            if let Some((a, score, _)) = self.account_raw(kind, &queries) {
                points.push((score, a == account));
            }
        }
        self.account_curve = Curve::fit(points);
    }

    /// The examples most like `q`, with their weights, heaviest first.
    fn neighbors(&self, q: &Query, admit: impl Fn(&Ex) -> bool) -> Vec<(u32, f32)> {
        let counts = features(q.key);
        if counts.is_empty() {
            return Vec::new();
        }
        // Features no example has still count in the row's norm: a text
        // with unseen words is less like any example.
        let unseen = (self.ex.len() as f32 + 1.0).ln() + 1.0;
        let mut norm = 0.0;
        let mut query = Vec::new();
        for (f, k) in &counts {
            match self.features.get(f) {
                Some(&j) => {
                    let w = tf(*k) * self.idf[j];
                    norm += w * w;
                    query.push((j, w));
                }
                None => norm += (tf(*k) * unseen).powi(2),
            }
        }
        let norm = norm.sqrt();
        let mut sims = vec![0.0_f32; self.ex.len()];
        let mut touched: Vec<u32> = Vec::new();
        for (j, w) in query {
            for (i, x) in &self.postings[j] {
                let s = &mut sims[*i as usize];
                if *s == 0.0 {
                    touched.push(*i);
                }
                *s += w / norm * x;
            }
        }
        // Rows of a counterparty settled as a fact are neighbors however
        // their text reads.
        if let Some(Some(c)) = q.facts.counterparty
            && let Some(of) = self.by_counterparty.get(&c)
        {
            for i in of {
                let s = &mut sims[*i as usize];
                if *s < MIN_SIMILARITY {
                    if *s == 0.0 {
                        touched.push(*i);
                    }
                    *s = MIN_SIMILARITY;
                }
            }
        }
        let mut out: Vec<(u32, f32)> = touched
            .into_iter()
            .map(|i| (i, sims[i as usize]))
            .filter(|(i, s)| {
                let e = &self.ex[*i as usize];
                *s >= MIN_SIMILARITY && q.before.is_none_or(|b| e.date < b) && admit(e)
            })
            .map(|(i, s)| (i, s * s * self.context(&self.ex[i as usize], q)))
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out.truncate(K);
        out
    }

    /// How much an example's circumstances are the row's.
    fn context(&self, e: &Ex, q: &Query) -> f32 {
        let account = match q.account {
            None => 1.0,
            Some(a) if a == e.account => 1.0,
            Some(_) if q.owner.is_some() && q.owner == e.owner => 0.7,
            Some(_) => 0.5,
        };
        let direction = if q.amount.is_sign_positive() == e.amount.is_sign_positive() {
            1.0
        } else {
            0.15
        };
        let amount = if q.amount.abs() == e.amount.abs() {
            1.5
        } else if magnitude(q.amount) == magnitude(e.amount) {
            1.15
        } else {
            1.0
        };
        let days = (q.date - e.date).whole_days().max(0) as f32;
        let recency = 1.0 / (1.0 + days / 730.0);
        account * direction * amount * recency
    }

    /// The neighbors' vote and alias memory, per label not in the facts.
    fn raw(
        &self,
        q: &Query,
    ) -> (
        BTreeMap<Label, Option<Vote>>,
        BTreeMap<Label, Option<Alias>>,
    ) {
        // Examples that agree with the facts; if none does (a label you set
        // that nothing has had), all of them.
        let mut near = self.neighbors(q, |e| q.facts.admit(&e.labels));
        if near.is_empty() {
            near = self.neighbors(q, |_| true);
        }
        let total: f32 = near.iter().map(|n| n.1).sum();
        // The winning combination, the first to reach the most weight.
        let mut combos: Vec<(Labels, f32)> = Vec::new();
        for (i, w) in &near {
            let l = self.ex[*i as usize].labels;
            match combos.iter_mut().find(|c| c.0 == l) {
                Some(c) => c.1 += w,
                None => combos.push((l, *w)),
            }
        }
        let winner = combos
            .iter()
            .fold(None::<&(Labels, f32)>, |best, c| match best {
                Some(b) if b.1 >= c.1 => Some(b),
                _ => Some(c),
            })
            .map(|c| c.0);

        // Alias memory: the same text, on this account if it has any.
        let same: Vec<&Ex> = self
            .alias
            .get(q.key)
            .map(|v| {
                v.iter()
                    .map(|i| &self.ex[*i as usize])
                    .filter(|e| q.before.is_none_or(|b| e.date < b))
                    .collect()
            })
            .unwrap_or_default();
        let agreeing: Vec<&Ex> = same
            .iter()
            .copied()
            .filter(|e| q.facts.admit(&e.labels))
            .collect();
        let same = if agreeing.is_empty() { same } else { agreeing };
        let mine: Vec<&Ex> = same
            .iter()
            .copied()
            .filter(|e| Some(e.account) == q.account)
            .collect();
        let latest: Vec<&Ex> = if mine.is_empty() { same } else { mine }
            .into_iter()
            .rev()
            .take(ALIAS)
            .collect();

        let mut votes = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        for l in Label::ALL {
            if q.facts.get(l).is_some() {
                votes.insert(l, None);
                aliases.insert(l, None);
                continue;
            }
            let vote = winner.and_then(|w| {
                let value = w.get(l);
                let agreeing: Vec<&(u32, f32)> = near
                    .iter()
                    .filter(|(i, _)| self.ex[*i as usize].labels.get(l) == value)
                    .collect();
                Some(Vote {
                    value,
                    score: agreeing.iter().map(|a| a.1).sum::<f32>() / (total + PRIOR),
                    like: agreeing.first()?.0,
                    agree: agreeing.len(),
                    of: near.len(),
                })
            });
            votes.insert(l, vote);
            let alias = latest.first().map(|first| {
                let value = first.labels.get(l);
                Alias {
                    value,
                    streak: latest
                        .iter()
                        .take_while(|e| e.labels.get(l) == value)
                        .count(),
                }
            });
            aliases.insert(l, alias);
        }
        (votes, aliases)
    }

    /// The account the rows' neighbors are in, its score (the rows' mean
    /// share of it) and how many rows it's the first choice of.
    fn account_raw(
        &self,
        kind: Option<StatementKind>,
        rows: &[Query],
    ) -> Option<(Uuid, f32, usize)> {
        let fits = |e: &Ex| {
            self.accounts
                .get(&e.account)
                .is_some_and(|a| !a.archived && kind.is_none_or(|k| k.fits(&a.kind)))
        };
        let mut share: HashMap<Uuid, f32> = HashMap::new();
        let mut first: HashMap<Uuid, usize> = HashMap::new();
        for q in rows {
            let near = self.neighbors(q, fits);
            let total: f32 = near.iter().map(|n| n.1).sum();
            if total <= 0.0 {
                continue;
            }
            let mut mine: Vec<(Uuid, f32)> = Vec::new();
            for (i, w) in &near {
                let a = self.ex[*i as usize].account;
                match mine.iter_mut().find(|m| m.0 == a) {
                    Some(m) => m.1 += w,
                    None => mine.push((a, *w)),
                }
            }
            for (a, w) in &mine {
                *share.entry(*a).or_default() += w / total / rows.len() as f32;
            }
            if let Some(top) = mine
                .iter()
                .fold(None::<&(Uuid, f32)>, |best, m| match best {
                    Some(b) if b.1 >= m.1 => Some(b),
                    _ => Some(m),
                })
            {
                *first.entry(top.0).or_default() += 1;
            }
        }
        let (account, score) = share
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))?;
        Some((account, score, first.get(&account).copied().unwrap_or(0)))
    }

    fn query<'a>(&self, r: &Row, key: &'a str) -> Query<'a> {
        Query {
            account: r.account,
            owner: r
                .account
                .and_then(|a| self.accounts.get(&a))
                .map(|a| a.owner),
            date: r.date,
            amount: r.amount,
            key,
            facts: r.facts,
            before: None,
        }
    }
}

/// Up to `n` of `items`, evenly spread.
fn spread<T>(items: &[T], n: usize) -> Vec<&T> {
    if items.len() <= n {
        return items.iter().collect();
    }
    (0..n).map(|i| &items[i * items.len() / n]).collect()
}

fn statement_kind(account_kind: &str) -> StatementKind {
    if account_kind == "credit_card" {
        StatementKind::CreditCard
    } else {
        StatementKind::Bank
    }
}

impl Suggester for Model {
    fn identity(&self) -> &Identity {
        &self.identity
    }

    fn labels(&self, rows: &[Row]) -> Vec<Proposal> {
        rows.iter()
            .map(|r| {
                let key = normalize::key(&r.description);
                let (votes, aliases) = self.raw(&self.query(r, &key));
                let mut p = Proposal::default();
                for l in Label::ALL {
                    let by_vote = votes[&l].as_ref().map(|v| {
                        let e = &self.ex[v.like as usize];
                        Guess {
                            value: v.value,
                            confidence: self.neighbors_curve.get(&l).map_or_else(
                                || Curve::default().apply(v.score),
                                |c| c.apply(v.score),
                            ),
                            reason: format!(
                                "like {} “{}” ({} of {} similar)",
                                e.date, e.shown, v.agree, v.of
                            ),
                        }
                    });
                    let by_alias = aliases[&l].as_ref().map(|a| Guess {
                        value: a.value,
                        confidence: self.alias_curve.get(&l).map_or_else(
                            || Curve::default().apply(0.0),
                            |c| c.apply(a.streak as f32),
                        ),
                        reason: if a.streak == 1 {
                            format!("the last time as “{key}”")
                        } else {
                            format!("the last {} times as “{key}”", a.streak)
                        },
                    });
                    let best = match (by_vote, by_alias) {
                        (Some(v), Some(a)) if a.confidence > v.confidence => Some(a),
                        (Some(v), _) => Some(v),
                        (None, a) => a,
                    };
                    p.set(l, best);
                }
                p
            })
            .collect()
    }

    fn account(&self, statement: &Statement) -> Option<Guess<Uuid>> {
        let rows = spread(&statement.rows, STATEMENT_ROWS);
        let keys: Vec<String> = rows
            .iter()
            .map(|r| normalize::key(&r.description))
            .collect();
        let queries: Vec<Query> = rows
            .iter()
            .zip(&keys)
            .map(|(r, k)| Query {
                account: None,
                owner: None,
                facts: Facts::default(),
                ..self.query(r, k)
            })
            .collect();
        let (account, score, first) = self.account_raw(statement.kind, &queries)?;
        Some(Guess {
            value: account,
            confidence: self.account_curve.apply(score),
            reason: format!(
                "{first} of {} rows look like its past transactions",
                queries.len()
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use suggest::{CONFIDENT, Example};
    use time::macros::date;

    use super::*;

    const HOUSEHOLD: Uuid = Uuid::from_u128(100);
    const CHECKING: Uuid = Uuid::from_u128(1);
    const CARD: Uuid = Uuid::from_u128(2);
    const CAFE: Uuid = Uuid::from_u128(10);
    const GROCER: Uuid = Uuid::from_u128(11);
    const EMPLOYER: Uuid = Uuid::from_u128(12);
    const DINING: Uuid = Uuid::from_u128(20);
    const GROCERIES: Uuid = Uuid::from_u128(21);
    const SALARY: Uuid = Uuid::from_u128(22);
    const ALEX: Uuid = Uuid::from_u128(30);

    fn labels(cp: Uuid, cat: Uuid, purpose: Option<Uuid>) -> Labels {
        Labels {
            counterparty: Some(cp),
            category: Some(cat),
            purpose,
        }
    }

    /// Two years of a fictional household: coffee and groceries on the
    /// card, pay into checking.
    fn history() -> History {
        let mut examples = Vec::new();
        let mut day = date!(2024 - 01 - 01);
        let mut n = 0u128;
        while day < date!(2026 - 01 - 01) {
            n += 1;
            let mut add = |account, amount: i64, description: String, l| {
                n += 1;
                examples.push(Example {
                    transaction: Uuid::from_u128(1000 + n),
                    account,
                    date: day,
                    amount: Decimal::new(amount, 2),
                    description,
                    labels: l,
                });
            };
            add(
                CARD,
                -450 - i64::from(day.day()),
                format!(
                    "SQ *CORNER CAFE #04{} {:02}/{:02}",
                    day.day() % 7,
                    day.month() as u8,
                    day.day()
                ),
                labels(CAFE, DINING, Some(ALEX)),
            );
            if day.day() % 3 == 0 {
                add(
                    CARD,
                    -8000,
                    format!("GREEN GROCER {}", day.to_julian_day()),
                    labels(GROCER, GROCERIES, None),
                );
            }
            if day.day() == 15 {
                add(
                    CHECKING,
                    250_000,
                    "ACME PAYROLL DIRECT DEP".to_owned(),
                    labels(EMPLOYER, SALARY, None),
                );
            }
            day = day.next_day().unwrap();
        }
        History {
            accounts: vec![
                Account {
                    id: CHECKING,
                    kind: "checking".into(),
                    owner: HOUSEHOLD,
                    archived: false,
                },
                Account {
                    id: CARD,
                    kind: "credit_card".into(),
                    owner: HOUSEHOLD,
                    archived: false,
                },
            ],
            examples,
        }
    }

    fn row(account: Option<Uuid>, amount: i64, description: &str) -> Row {
        Row {
            account,
            date: date!(2026 - 01 - 10),
            amount: Decimal::new(amount, 2),
            description: description.into(),
            facts: Facts::default(),
        }
    }

    #[test]
    fn a_familiar_row_is_labeled_confidently() {
        let m = LexicalKnn.train(&history());
        assert_eq!(m.identity().name, "lexical-knn");
        let p = &m.labels(&[row(Some(CARD), -463, "SQ *CORNER CAFE #0412 01/10")])[0];
        let cp = p.counterparty.as_ref().unwrap();
        assert_eq!(cp.value, Some(CAFE));
        assert!(cp.confidence >= CONFIDENT, "{cp:?}");
        assert_eq!(p.purpose.as_ref().unwrap().value, Some(ALEX));
        // "None" is a value like any other: groceries are the owner's.
        let p = &m.labels(&[row(Some(CARD), -8000, "GREEN GROCER 999999")])[0];
        let purpose = p.purpose.as_ref().unwrap();
        assert_eq!(
            (purpose.value, purpose.confidence >= CONFIDENT),
            (None, true)
        );
    }

    #[test]
    fn a_stranger_gets_nothing_confident() {
        let m = LexicalKnn.train(&history());
        let p = &m.labels(&[row(Some(CARD), -1999, "UNHEARD OF HARDWARE")])[0];
        for l in Label::ALL {
            assert!(
                p.get(l).is_none_or(|g| g.confidence < CONFIDENT),
                "{l:?}: {p:?}"
            );
        }
    }

    #[test]
    fn facts_are_left_out_and_steer_the_rest() {
        let m = LexicalKnn.train(&history());
        let mut r = row(Some(CARD), -463, "SQ *CORNER CAFE");
        r.facts.set(Label::Counterparty, Some(GROCER));
        let p = &m.labels(&[r])[0];
        assert!(p.counterparty.is_none());
        assert_eq!(p.category.as_ref().unwrap().value, Some(GROCERIES));
    }

    #[test]
    fn a_statement_is_matched_to_its_account() {
        let m = LexicalKnn.train(&history());
        let s = Statement {
            kind: None,
            rows: vec![
                row(None, -455, "SQ *CORNER CAFE #0401"),
                row(None, -8000, "GREEN GROCER 123456"),
                row(None, -470, "SQ *CORNER CAFE #0402"),
            ],
        };
        let g = m.account(&s).unwrap();
        assert_eq!(g.value, CARD);
        assert!(g.confidence >= CONFIDENT, "{g:?}");
        // A bank statement can't be the card's.
        let g = m.account(&Statement {
            kind: Some(StatementKind::Bank),
            ..s
        });
        assert!(g.is_none_or(|g| g.value == CHECKING && g.confidence < CONFIDENT));
    }

    #[test]
    fn nothing_learned_suggests_nothing() {
        let m = LexicalKnn.train(&History::default());
        assert_eq!(
            m.labels(&[row(None, -100, "CAFE")]),
            vec![Proposal::default()]
        );
        assert!(
            m.account(&Statement {
                kind: None,
                rows: vec![row(None, -100, "CAFE")]
            })
            .is_none()
        );
    }

    #[test]
    fn the_snapshot_names_the_data() {
        let h = history();
        let a = LexicalKnn.train(&h);
        let b = LexicalKnn.train(&h.before(date!(2025 - 06 - 01)));
        assert_ne!(a.identity(), b.identity());
        assert_eq!(a.identity(), LexicalKnn.train(&h).identity());
    }

    #[test]
    fn a_backtest_measures_the_method() {
        let r = suggest::backtest::run(
            &LexicalKnn,
            &history(),
            date!(2025 - 12 - 01),
            date!(2026 - 01 - 01),
        );
        let cp = &r.labels[&Label::Counterparty];
        assert!(cp.asked > 30 && cp.correct == cp.asked, "{cp:?}");
        assert_eq!(r.accounts.asked, 2);
        assert_eq!(r.accounts.correct, 2);
    }
}
