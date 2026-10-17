//! What the decision model is shown (docs/ai.md): computed from the
//! ledger, never hand-written, so it doesn't go stale, and worded by the
//! templates in `resources/` ([`crate::prompts`]).

use std::collections::{BTreeMap, HashMap};

use heuristics::normalize;
use outbound::decision::Question;
use rust_decimal::Decimal;
use serde_json::{Value as Json, json};
use suggest::{Candidate, Example, History, Label, Row, Statement, Value};
use time::{Date, Duration};
use uuid::Uuid;

use crate::prompts::{LABELS, render};

/// Similar past transactions shown per row.
pub const SIMILAR: usize = 10;
/// Counterparty candidates offered per row.
const CANDIDATES: usize = 10;
/// A statement's rows shown, spread over it.
const STATEMENT_ROWS: usize = 8;
/// What "usual" covers: the year before the ledger's latest transaction.
const YEAR: Duration = Duration::days(365);
/// An account's usage: its last six months.
const HALF_YEAR: Duration = Duration::days(183);
/// Bank texts' length shown.
const TEXT: usize = 100;

/// One of your accounts, as remote AI may describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
    pub id: Uuid,
    pub name: String,
    /// `checking`, `credit_card`, …
    pub kind: String,
    pub owner: Uuid,
    /// The last four of each number it's known by.
    pub numbers: Vec<String>,
    pub archived: bool,
}

/// A party: an account owner, or whom a transaction is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyInfo {
    pub id: Uuid,
    pub name: String,
    /// `household`, `person` or `business`.
    pub kind: String,
}

/// The ledger's names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    pub accounts: Vec<AccountInfo>,
    /// Ordinary counterparties (not system ones such as opening balances).
    pub counterparties: Vec<(Uuid, String)>,
    pub categories: Vec<(Uuid, String)>,
    pub parties: Vec<PartyInfo>,
}

/// A question and the value each option stands for; options standing for
/// no answer ("cannot tell") have none.
pub struct Options<T> {
    pub question: Question,
    pub values: Vec<(String, T)>,
}

/// How often each value occurred.
#[derive(Default)]
struct Tally(HashMap<Value, u32>);

impl Tally {
    fn add(&mut self, v: Value) {
        *self.0.entry(v).or_default() += 1;
    }

    /// The most frequent first, with their percentage, at most `n`.
    fn top(&self, n: usize) -> Vec<(Value, u32)> {
        let total: u32 = self.0.values().sum();
        let mut v: Vec<(Value, u32)> = self.0.iter().map(|(k, c)| (*k, *c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v.into_iter()
            .map(|(k, c)| (k, c * 100 / total.max(1)))
            .collect()
    }
}

/// A label value's use over the past year.
#[derive(Default)]
struct Usage {
    count: u32,
    counterparties: Tally,
    categories: Tally,
    purposes: Tally,
    amounts: Vec<Decimal>,
    /// The latest bank text.
    latest: Option<(Date, String)>,
}

impl Usage {
    fn add(&mut self, e: &Example) {
        self.count += 1;
        self.counterparties.add(e.labels.counterparty);
        self.categories.add(e.labels.category);
        self.purposes.add(e.labels.purpose);
        self.amounts.push(e.amount.abs());
        if self.latest.as_ref().is_none_or(|l| e.date >= l.0) {
            self.latest = Some((e.date, text(&e.description)));
        }
    }

    /// The median amount and the range.
    fn typical(&self) -> Json {
        let mut a = self.amounts.clone();
        a.sort();
        match (a.first(), a.get(a.len() / 2), a.last()) {
            (Some(lo), Some(mid), Some(hi)) => {
                json!({ "median": money(*mid), "low": money(*lo), "high": money(*hi) })
            }
            _ => Json::Null,
        }
    }
}

fn money(d: Decimal) -> String {
    format!("{:.2}", d.abs())
}

/// A bank text as sent: normalized, so no long number is.
fn text(s: &str) -> String {
    normalize::description(s).chars().take(TEXT).collect()
}

fn direction(amount: Decimal) -> &'static str {
    if amount.is_sign_negative() {
        "out of the account"
    } else {
        "into the account"
    }
}

/// A deterministic shuffle: options are ordered at random (order sways the
/// model), but the same row is always asked the same way.
fn shuffle<T>(items: &mut [T], seed: &str) {
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.bytes() {
        x = (x ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    for i in (1..items.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let j = usize::try_from(x % (i as u64 + 1)).unwrap_or(0);
        items.swap(i, j);
    }
}

/// Up to `n` of `items`, evenly spread.
fn spread<T>(items: &[T], n: usize) -> Vec<&T> {
    if items.len() <= n {
        return items.iter().collect();
    }
    (0..n).map(|i| &items[i * items.len() / n]).collect()
}

/// The ledger, summarized for questions.
pub struct Context {
    catalog: Catalog,
    examples: Vec<Example>,
    by_transaction: HashMap<Uuid, usize>,
    /// Examples by normalized text, oldest first.
    by_key: HashMap<String, Vec<usize>>,
    /// By counterparty, or transfer account.
    counterparties: HashMap<Value, Usage>,
    categories: HashMap<Uuid, Usage>,
    /// By party; `None` is the account's owner.
    purposes: HashMap<Value, Usage>,
    /// Each account's last six months.
    accounts: HashMap<Uuid, Usage>,
}

impl Context {
    pub fn new(history: &History, catalog: &Catalog) -> Self {
        let mut examples = history.examples.clone();
        examples.sort_by(|a, b| a.date.cmp(&b.date).then(a.transaction.cmp(&b.transaction)));
        let latest = history.through().unwrap_or(Date::MIN);
        let mut c = Self {
            catalog: catalog.clone(),
            by_transaction: HashMap::new(),
            by_key: HashMap::new(),
            counterparties: HashMap::new(),
            categories: HashMap::new(),
            purposes: HashMap::new(),
            accounts: HashMap::new(),
            examples: Vec::new(),
        };
        for (i, e) in examples.iter().enumerate() {
            c.by_transaction.insert(e.transaction, i);
            c.by_key
                .entry(normalize::key(&e.description))
                .or_default()
                .push(i);
            // A transfer has no category or purpose: it says nothing about
            // them, nor about how an account's spending is labeled.
            let transfer = matches!(e.labels.counterparty, Value::Account(_));
            if e.date + YEAR >= latest {
                if e.labels.counterparty != Value::None {
                    c.counterparties
                        .entry(e.labels.counterparty)
                        .or_default()
                        .add(e);
                }
                if let Some(id) = e.labels.category.id() {
                    c.categories.entry(id).or_default().add(e);
                }
                if !transfer {
                    c.purposes.entry(e.labels.purpose).or_default().add(e);
                }
            }
            if e.date + HALF_YEAR >= latest && !transfer {
                c.accounts.entry(e.account).or_default().add(e);
            }
        }
        c.examples = examples;
        c
    }

    fn counterparty(&self, v: Value) -> String {
        match v {
            Value::Account(id) => format!(
                "{} {}",
                LABELS.your_account,
                self.account(id)
                    .map_or_else(|| id.to_string(), |a| self.account_name(a))
            ),
            v => self.name(
                v,
                &self.catalog.counterparties,
                &LABELS.unknown_counterparty,
            ),
        }
    }

    fn category(&self, v: Value) -> String {
        self.name(v, &self.catalog.categories, &LABELS.no_category)
    }

    fn party(&self, v: Value) -> String {
        match v {
            Value::None => LABELS.owner.clone(),
            Value::Id(id) | Value::Account(id) => self
                .catalog
                .parties
                .iter()
                .find(|p| p.id == id)
                .map_or_else(|| id.to_string(), |p| p.name.clone()),
        }
    }

    fn name(&self, v: Value, names: &[(Uuid, String)], none: &str) -> String {
        match v {
            Value::None => none.to_owned(),
            Value::Id(id) | Value::Account(id) => names
                .iter()
                .find(|n| n.0 == id)
                .map_or_else(|| id.to_string(), |n| n.1.clone()),
        }
    }

    fn account(&self, id: Uuid) -> Option<&AccountInfo> {
        self.catalog.accounts.iter().find(|a| a.id == id)
    }

    fn owner(&self, a: &AccountInfo) -> Option<&PartyInfo> {
        self.catalog.parties.iter().find(|p| p.id == a.owner)
    }

    /// `Owner · Name`: account names are unique per owner only.
    fn account_name(&self, a: &AccountInfo) -> String {
        match self.owner(a) {
            Some(o) => format!("{} · {}", o.name, a.name),
            None => a.name.clone(),
        }
    }

    fn shares(&self, t: &Tally, n: usize, name: impl Fn(Value) -> String) -> Vec<String> {
        t.top(n)
            .into_iter()
            .map(|(v, pct)| format!("{} {pct}%", name(v)))
            .collect()
    }

    /// An account, and how it's been used.
    fn account_json(&self, a: &AccountInfo) -> Json {
        let owner = self.owner(a);
        let mut j = json!({
            "name": self.account_name(a),
            "kind": a.kind,
            "owner": owner.map(|o| &o.name),
            "owner_kind": owner.map(|o| &o.kind),
        });
        if let Some(u) = self.accounts.get(&a.id) {
            j["recent_purposes"] = json!(self.shares(&u.purposes, 4, |v| self.party(v)));
            j["recent_categories"] = json!(self.shares(&u.categories, 6, |v| self.category(v)));
        }
        j
    }

    fn example_json(&self, e: &Example) -> Json {
        json!({
            "date": e.date.to_string(),
            "description": text(&e.description),
            "amount": money(e.amount),
            "direction": direction(e.amount),
            "account": self.account(e.account).map(|a| self.account_name(a)),
            "counterparty": self.counterparty(e.labels.counterparty),
            "category": self.category(e.labels.category),
            "purpose": self.party(e.labels.purpose),
        })
    }

    /// The bank's own fields that say what kind of transaction it is
    /// (`Type`, `Category`, a merchant category code), normalized.
    fn bank_fields(raw: &Json) -> Json {
        let mut out = serde_json::Map::new();
        if let Some(fields) = raw.as_object() {
            for (k, v) in fields {
                let lower = k.to_lowercase();
                if !["type", "category", "mcc", "sic"]
                    .iter()
                    .any(|w| lower.contains(w))
                {
                    continue;
                }
                let v = match v {
                    Json::String(s) => s.clone(),
                    Json::Number(n) => n.to_string(),
                    _ => continue,
                };
                if !v.trim().is_empty() && out.len() < 5 {
                    out.insert(k.chars().take(40).collect(), json!(text(&v)));
                }
            }
        }
        Json::Object(out)
    }

    /// "The same amount, with the same text, in N of the last 6 months."
    fn recurs(&self, row: &Row) -> Option<String> {
        let same = self.by_key.get(&normalize::key(&row.description))?;
        let mut months: Vec<(i32, u8)> = same
            .iter()
            .map(|i| &self.examples[*i])
            .filter(|e| {
                e.date < row.date
                    && e.date + HALF_YEAR >= row.date
                    && e.amount.abs() == row.amount.abs()
                    && row.account.is_none_or(|a| a == e.account)
            })
            .map(|e| (e.date.year(), u8::from(e.date.month())))
            .collect();
        months.sort_unstable();
        months.dedup();
        (months.len() >= 2).then(|| render("state/recurs.j2", &json!({ "months": months.len() })))
    }

    /// A counterparty, or one of your accounts (`row`'s potential transfer
    /// match there, if any).
    fn counterparty_option(&self, v: Value, staged: Option<&Candidate>) -> String {
        let transfer = v.account().map(|id| {
            json!({
                "kind": self.account(id).map(|a| &a.kind),
                "staged": staged.map(|c| c.date.to_string()),
            })
        });
        let usage = self.counterparties.get(&v).map(|u| {
            json!({
                "latest": u.latest.as_ref().map(|l| &l.1),
                "categories": self.shares(&u.categories, 2, |v| self.category(v)),
                "purposes": self.shares(&u.purposes, 2, |v| self.party(v)),
                "amount": u.typical(),
                "count": u.count,
            })
        });
        render(
            "options/counterparty.j2",
            &json!({ "transfer": transfer, "usage": usage }),
        )
    }

    fn category_option(&self, id: Uuid) -> String {
        let usage = self.categories.get(&id).map(|u| {
            json!({
                "counterparties": self.shares(&u.counterparties, 4, |v| self.counterparty(v)),
                "amount": u.typical(),
                "purposes": self.shares(&u.purposes, 2, |v| self.party(v)),
                "count": u.count,
            })
        });
        render("options/category.j2", &json!({ "usage": usage }))
    }

    fn party_option(&self, v: Value) -> String {
        let party = v
            .id()
            .and_then(|id| self.catalog.parties.iter().find(|p| p.id == id))
            .map(|p| json!({ "kind": p.kind }));
        let usage = self.purposes.get(&v).map(|u| {
            json!({
                "categories": self.shares(&u.categories, 4, |v| self.category(v)),
                "counterparties": self.shares(&u.counterparties, 4, |v| self.counterparty(v)),
                "count": u.count,
            })
        });
        render(
            "options/purpose.j2",
            &json!({ "party": party, "usage": usage }),
        )
    }

    /// A question over `options` (each a key, its value and its
    /// description), plus `extra` options standing for no answer.
    fn question<T>(
        name: &str,
        options: Vec<(String, T, String)>,
        extra: &[(&String, &String)],
        seed: &str,
    ) -> Options<T> {
        let mut criteria: Vec<(String, String)> = Vec::new();
        let mut values = Vec::new();
        for (k, v, d) in options {
            if criteria.iter().any(|c| c.0 == k) {
                continue;
            }
            criteria.push((k.clone(), d));
            values.push((k, v));
        }
        for (k, d) in extra {
            criteria.push(((*k).to_owned(), (*d).to_owned()));
        }
        shuffle(&mut criteria, &format!("{seed}|{name}"));
        Options {
            question: Question {
                name: name.to_owned(),
                instructions: render(&format!("questions/{name}.j2"), &Json::Null),
                options: criteria,
            },
            values,
        }
    }

    /// The state and the questions for a row's open labels.
    pub fn row(&self, row: &Row, similar: &[Uuid]) -> (Json, Vec<(Label, Options<Value>)>) {
        let similar: Vec<&Example> = similar
            .iter()
            .filter_map(|t| self.by_transaction.get(t).map(|i| &self.examples[*i]))
            .collect();
        let seed = format!("{}|{}|{}", row.date, row.amount, row.description);
        let mut asked = Vec::new();
        for label in Label::ALL {
            if row.facts.get(label).is_some() {
                continue;
            }
            let o = match label {
                Label::Counterparty => self.counterparty_question(row, &similar, &seed),
                Label::Category => self.category_question(&seed),
                Label::Purpose => self.purpose_question(&seed),
            };
            if let Some(o) = o {
                asked.push((label, o));
            }
        }

        let mut r = json!({
            "description": text(&row.description),
            "date": row.date.to_string(),
            "weekday": row.date.weekday().to_string(),
            "amount": money(row.amount),
            "direction": direction(row.amount),
        });
        let fields = Self::bank_fields(&row.raw);
        if fields.as_object().is_some_and(|f| !f.is_empty()) {
            r["bank_fields"] = fields;
        }
        if let Some(s) = self.recurs(row) {
            r["recurs"] = json!(s);
        }
        let staged: Vec<Json> = row
            .transfer_candidates
            .iter()
            .filter_map(|c| {
                let a = self.account(c.account)?;
                Some(json!({ "account": self.account_name(a), "date": c.date.to_string() }))
            })
            .collect();
        if !staged.is_empty() {
            r["same_amount_the_other_way_in"] = json!(staged);
        }
        let mut state = json!({ "transaction": r });
        if let Some(a) = row.account.and_then(|a| self.account(a)) {
            state["account"] = self.account_json(a);
        }
        let mut settled = BTreeMap::new();
        for l in Label::ALL {
            if let Some(v) = row.facts.get(l) {
                let name = match l {
                    Label::Counterparty => self.counterparty(v),
                    Label::Category => self.category(v),
                    Label::Purpose => self.party(v),
                };
                settled.insert(l.as_str(), name);
            }
        }
        if !settled.is_empty() {
            state["already_known"] = json!(settled);
        }
        if !similar.is_empty() {
            state["similar_past_transactions"] =
                similar.iter().map(|e| self.example_json(e)).collect();
        }
        (state, asked)
    }

    /// Candidates only: the counterparties of similar transactions and of
    /// those with the same text; none, nothing to ask.
    fn counterparty_question(
        &self,
        row: &Row,
        similar: &[&Example],
        seed: &str,
    ) -> Option<Options<Value>> {
        let same: Vec<&Example> = self
            .by_key
            .get(&normalize::key(&row.description))
            .map(|v| v.iter().rev().map(|i| &self.examples[*i]).collect())
            .unwrap_or_default();
        // The accounts of its potential transfer matches first, then what
        // similar rows were.
        let mut candidates: Vec<Value> = Vec::new();
        let staged = row
            .transfer_candidates
            .iter()
            .map(|c| Value::Account(c.account));
        for v in staged.chain(similar.iter().chain(&same).map(|e| e.labels.counterparty)) {
            if candidates.len() < CANDIDATES && !candidates.contains(&v) {
                candidates.push(v);
            }
        }
        if candidates.iter().all(|v| *v == Value::None) {
            return None;
        }
        let options = candidates
            .into_iter()
            .map(|v| {
                let d = match v {
                    Value::None => LABELS.unknown_counterparty_means.clone(),
                    v => self.counterparty_option(
                        v,
                        v.account()
                            .and_then(|a| row.transfer_candidates.iter().find(|c| c.account == a)),
                    ),
                };
                (self.counterparty(v), v, d)
            })
            .collect();
        Some(Self::question(
            "counterparty",
            options,
            &[
                (&LABELS.none_of_these, &LABELS.no_counterparty_listed),
                (&LABELS.cannot_tell, &LABELS.cannot_tell_means),
            ],
            seed,
        ))
    }

    fn category_question(&self, seed: &str) -> Option<Options<Value>> {
        let mut options: Vec<(String, Value, String)> = self
            .catalog
            .categories
            .iter()
            .map(|(id, name)| (name.clone(), Value::Id(*id), self.category_option(*id)))
            .collect();
        options.push((
            LABELS.no_category.clone(),
            Value::None,
            LABELS.no_category_means.clone(),
        ));
        Some(Self::question(
            "category",
            options,
            &[(&LABELS.cannot_tell, &LABELS.cannot_tell_means)],
            seed,
        ))
    }

    fn purpose_question(&self, seed: &str) -> Option<Options<Value>> {
        let mut options: Vec<(String, Value, String)> = self
            .catalog
            .parties
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    Value::Id(p.id),
                    self.party_option(Value::Id(p.id)),
                )
            })
            .collect();
        options.push((
            LABELS.owner.clone(),
            Value::None,
            self.party_option(Value::None),
        ));
        Some(Self::question(
            "purpose",
            options,
            &[(&LABELS.cannot_tell, &LABELS.cannot_tell_means)],
            seed,
        ))
    }

    /// The state and the question for a statement's account: the accounts
    /// it may be (not archived, of the file's kind).
    pub fn statement(&self, s: &Statement) -> Option<(Json, Options<Uuid>)> {
        let options: Vec<(String, Uuid, String)> = self
            .catalog
            .accounts
            .iter()
            .filter(|a| !a.archived && s.kind.is_none_or(|k| k.fits(&a.kind)))
            .map(|a| {
                let usage = self.accounts.get(&a.id).map(|u| {
                    let texts: Vec<String> = self
                        .examples
                        .iter()
                        .rev()
                        .filter(|e| e.account == a.id)
                        .take(4)
                        .map(|e| text(&e.description))
                        .collect();
                    json!({
                        "texts": texts,
                        "amount": u.typical(),
                        "latest": u.latest.as_ref().map(|l| l.0.to_string()),
                    })
                });
                let d = render(
                    "options/account.j2",
                    &json!({
                        "kind": a.kind,
                        "owner": self.owner(a).map(|o| &o.name),
                        "numbers": a.numbers,
                        "usage": usage,
                    }),
                );
                (self.account_name(a), a.id, d)
            })
            .collect();
        if options.is_empty() {
            return None;
        }
        let rows: Vec<Json> = spread(&s.rows, STATEMENT_ROWS)
            .into_iter()
            .map(|r| {
                json!({
                    "date": r.date.to_string(),
                    "description": text(&r.description),
                    "amount": money(r.amount),
                    "direction": direction(r.amount),
                })
            })
            .collect();
        let mut file = json!({
            "rows": s.rows.len(),
            "from": s.rows.iter().map(|r| r.date).min().map(|d| d.to_string()),
            "to": s.rows.iter().map(|r| r.date).max().map(|d| d.to_string()),
            "some_rows": rows,
        });
        if let Some(n) = &s.file_name {
            file["file_name"] = json!(text(n));
        }
        if let Some(n) = &s.number {
            file["account_number_ending"] = json!(n);
        }
        if let Some(k) = s.kind {
            file["kind"] = json!(match k {
                suggest::StatementKind::Bank => "bank account",
                suggest::StatementKind::CreditCard => "credit card",
            });
        }
        let seed = format!("{:?}|{}", s.file_name, s.rows.len());
        Some((
            json!({ "statement": file }),
            Self::question(
                "account",
                options,
                &[
                    (&LABELS.none_of_these, &LABELS.no_account_listed),
                    (&LABELS.cannot_tell, &LABELS.cannot_tell_means),
                ],
                &seed,
            ),
        ))
    }
}
