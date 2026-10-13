//! Suggestions (design §8, docs/ai.md): what a bank row is (its counterparty,
//! category and purpose) and which account a statement is for.
//!
//! Every suggester implements [`Suggester`], whatever it is: rules, a local
//! method or a remote model. A local method ([`Method`]) is trained from the
//! ledger's labeled history into a suggester; methods are interchangeable, so
//! replacing one is a configuration change, and each suggestion names the
//! exact suggester that made it ([`Identity`]). [`stack`] combines the levels;
//! [`backtest`] measures a method on the ledger's own past.
//!
//! Confidences are per mille ([`CONFIDENT`] is 0.9); nothing here is a float.

use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use time::Date;
use uuid::Uuid;

pub mod backtest;
pub mod stack;

/// Confidence, per mille, from which a suggestion counts as made.
pub const CONFIDENT: u16 = 900;

/// What a row is labeled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Label {
    Counterparty,
    Category,
    Purpose,
}

impl Label {
    pub const ALL: [Self; 3] = [Self::Counterparty, Self::Category, Self::Purpose];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Counterparty => "counterparty",
            Self::Category => "category",
            Self::Purpose => "purpose",
        }
    }
}

/// A label's value: an ID, or `None` for "none" (an unknown counterparty, no
/// category, the account's owner as the purpose).
pub type Value = Option<Uuid>;

/// One value per label.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Labels {
    pub counterparty: Value,
    pub category: Value,
    pub purpose: Value,
}

impl Labels {
    #[must_use]
    pub fn get(&self, label: Label) -> Value {
        match label {
            Label::Counterparty => self.counterparty,
            Label::Category => self.category,
            Label::Purpose => self.purpose,
        }
    }
}

/// Labels already settled for a row (by you, or confidently by an earlier
/// level): a suggester proposes the others consistently with them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    pub counterparty: Option<Value>,
    pub category: Option<Value>,
    pub purpose: Option<Value>,
}

impl Facts {
    #[must_use]
    pub fn get(&self, label: Label) -> Option<Value> {
        match label {
            Label::Counterparty => self.counterparty,
            Label::Category => self.category,
            Label::Purpose => self.purpose,
        }
    }

    pub fn set(&mut self, label: Label, value: Value) {
        let slot = match label {
            Label::Counterparty => &mut self.counterparty,
            Label::Category => &mut self.category,
            Label::Purpose => &mut self.purpose,
        };
        *slot = Some(value);
    }

    /// Whether `labels` agree with every fact.
    #[must_use]
    pub fn admit(&self, labels: &Labels) -> bool {
        Label::ALL
            .into_iter()
            .all(|l| self.get(l).is_none_or(|v| v == labels.get(l)))
    }

    #[must_use]
    pub fn all_settled(&self) -> bool {
        Label::ALL.into_iter().all(|l| self.get(l).is_some())
    }
}

/// Which level a suggester belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Rule,
    Local,
    Remote,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// The exact system that made a suggestion: two suggesters with the same
/// identity make the same suggestions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub kind: Kind,
    pub name: String,
    pub version: String,
    /// Whatever else identifies it, e.g. the data a local method learned
    /// from. A JSON object.
    pub details: serde_json::Value,
}

/// One of your accounts, as a suggester sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: Uuid,
    /// `checking`, `credit_card`, …
    pub kind: String,
    pub owner: Uuid,
    pub archived: bool,
}

/// A labeled transaction from the ledger: one of your accounts and the
/// outside, as an imported row becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    pub transaction: Uuid,
    pub account: Uuid,
    pub date: Date,
    /// Signed, as the account's statement shows it: positive into it.
    pub amount: Decimal,
    pub description: String,
    pub labels: Labels,
}

/// What a local method learns from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    pub accounts: Vec<Account>,
    pub examples: Vec<Example>,
}

impl History {
    /// The part of it dated before `date`.
    #[must_use]
    pub fn before(&self, date: Date) -> Self {
        Self {
            accounts: self.accounts.clone(),
            examples: self
                .examples
                .iter()
                .filter(|e| e.date < date)
                .cloned()
                .collect(),
        }
    }

    /// Identifies the data, for a suggester's identity: equal histories
    /// give equal snapshots.
    #[must_use]
    pub fn snapshot(&self) -> String {
        let mut examples: Vec<&Example> = self.examples.iter().collect();
        examples.sort_by_key(|e| e.transaction);
        let mut accounts: Vec<&Account> = self.accounts.iter().collect();
        accounts.sort_by_key(|a| a.id);
        let mut h = Sha256::new();
        let id = |v: Value| v.map(|u| u.to_string()).unwrap_or_default();
        for a in accounts {
            h.update(format!(
                "a|{}|{}|{}|{}\n",
                a.id, a.kind, a.owner, a.archived
            ));
        }
        for e in examples {
            h.update(format!(
                "e|{}|{}|{}|{}|{}|{}|{}|{}\n",
                e.transaction,
                e.account,
                e.date,
                e.amount.normalize(),
                e.description,
                id(e.labels.counterparty),
                id(e.labels.category),
                id(e.labels.purpose),
            ));
        }
        h.finalize()
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// The latest date in it.
    #[must_use]
    pub fn through(&self) -> Option<Date> {
        self.examples.iter().map(|e| e.date).max()
    }
}

/// A bank row to suggest labels for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The statement's account, when known.
    pub account: Option<Uuid>,
    pub date: Date,
    /// Signed: positive into the account.
    pub amount: Decimal,
    pub description: String,
    /// What the bank file said about it, field by field (a JSON object), if
    /// it came from one.
    pub raw: serde_json::Value,
    pub facts: Facts,
}

/// What kind of account a bank file says a statement is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    Bank,
    CreditCard,
}

impl StatementKind {
    /// Whether an account of this kind (`checking`, `credit_card`, …) fits.
    #[must_use]
    pub fn fits(self, account_kind: &str) -> bool {
        (account_kind == "credit_card") == (self == Self::CreditCard)
    }
}

/// A statement without an account, to suggest one for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// The bank file's name, if it was given.
    pub file_name: Option<String>,
    /// The last four of the account number the file names, if any (no
    /// account has it yet).
    pub number: Option<String>,
    pub kind: Option<StatementKind>,
    pub rows: Vec<Row>,
}

/// A suggested value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guess<T> {
    pub value: T,
    /// Per mille.
    pub confidence: u16,
    /// Short and readable: where it comes from.
    pub reason: String,
}

/// A suggester's proposal for one row: at most one guess per label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proposal {
    pub counterparty: Option<Guess<Value>>,
    pub category: Option<Guess<Value>>,
    pub purpose: Option<Guess<Value>>,
}

impl Proposal {
    #[must_use]
    pub fn get(&self, label: Label) -> Option<&Guess<Value>> {
        match label {
            Label::Counterparty => self.counterparty.as_ref(),
            Label::Category => self.category.as_ref(),
            Label::Purpose => self.purpose.as_ref(),
        }
    }

    pub fn set(&mut self, label: Label, guess: Option<Guess<Value>>) {
        match label {
            Label::Counterparty => self.counterparty = guess,
            Label::Category => self.category = guess,
            Label::Purpose => self.purpose = guess,
        }
    }
}

/// Anything that makes suggestions.
pub trait Suggester: Send + Sync {
    fn identity(&self) -> &Identity;

    /// One proposal per row, in order. Labels in a row's facts are left out.
    fn labels(&self, rows: &[Row]) -> Vec<Proposal>;

    /// The account a statement is for, if it can tell.
    fn account(&self, statement: &Statement) -> Option<Guess<Uuid>>;

    /// The labeled transactions most like a row (agreeing with its facts),
    /// most alike first, at most `n`: evidence a later level can be shown.
    /// None by default.
    fn similar(&self, _row: &Row, _n: usize) -> Vec<Uuid> {
        Vec::new()
    }
}

/// A local method: learns from the ledger's history, in process.
pub trait Method: Send + Sync {
    /// E.g. `lexical-knn`.
    fn name(&self) -> &'static str;
    /// Changes whenever what it suggests from the same history may change.
    fn version(&self) -> &'static str;
    fn train(&self, history: &History) -> Box<dyn Suggester>;

    /// How configuration names it: `<name>-v<version>`.
    fn id(&self) -> String {
        format!("{}-v{}", self.name(), self.version())
    }
}
