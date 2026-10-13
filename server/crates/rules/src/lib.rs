//! Rules (design §8, docs/ai.md): the household's [Rhai](https://rhai.rs)
//! script, the first level of the suggestion stack.
//!
//! The script may define `fn classify(row, s)`, called for each bank row,
//! and `fn account(file, s)`, called for each statement without an account.
//! What it sets is a suggestion at full confidence, naming the script's
//! version and the line that set it.
//!
//! Scripts are sandboxed: no file system, network or process access, no
//! `eval`, limited operations per call. Rhai is built without floating
//! point, and amounts are [`Amount`]s, compared with `amount("…")` literals
//! (design §3.5).
//!
//! Validation ([`Script::compile`]) rejects a script with line-numbered
//! errors: syntax, undefined variables, the entry points' parameters, and
//! every name the script uses (in setters such as `s.category("Dining")`
//! and in comparisons such as `row.account == "Alex · Checking"`), which
//! must be written out so they can be checked.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rhai::{
    AST, ASTNode, Dynamic, Engine, EvalAltResult, Expr, Map, NativeCallContext, Position, Scope,
};
use rust_decimal::Decimal;
use suggest::{Guess, Identity, Kind, Label, Proposal, Row, Statement, Suggester, Value};
use uuid::Uuid;

/// Longest script accepted, in bytes.
pub const MAX_SOURCE: usize = 200_000;
/// Operations one call may take before it's stopped.
const MAX_OPERATIONS: u64 = 100_000;

/// One of your accounts, as scripts name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountName {
    pub id: Uuid,
    pub name: String,
    /// The owner's name.
    pub owner: String,
}

impl AccountName {
    /// `Owner · Name`, as accounts are shown.
    #[must_use]
    pub fn shown(&self) -> String {
        format!("{} · {}", self.owner, self.name)
    }
}

/// Every name a script may use, with what it stands for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    pub accounts: Vec<AccountName>,
    pub counterparties: Vec<(Uuid, String)>,
    pub categories: Vec<(Uuid, String)>,
    /// Purposes: the parties.
    pub parties: Vec<(Uuid, String)>,
}

/// Names to IDs.
#[derive(Debug, Default)]
struct Lookup {
    counterparties: HashMap<String, Uuid>,
    categories: HashMap<String, Uuid>,
    parties: HashMap<String, Uuid>,
    /// By `Owner · Name`, and by name alone when only one account has it
    /// (`None` when several do).
    accounts: HashMap<String, Option<Uuid>>,
    /// Account names alone, for `row.account_name`.
    account_names: HashMap<String, ()>,
    by_id: HashMap<Uuid, AccountName>,
}

impl Lookup {
    fn new(names: &Names) -> Self {
        let mut accounts: HashMap<String, Option<Uuid>> = HashMap::new();
        for a in &names.accounts {
            accounts
                .entry(a.name.clone())
                .and_modify(|v| *v = None)
                .or_insert(Some(a.id));
        }
        for a in &names.accounts {
            accounts.insert(a.shown(), Some(a.id));
        }
        let map = |v: &[(Uuid, String)]| v.iter().map(|(id, n)| (n.clone(), *id)).collect();
        Self {
            counterparties: map(&names.counterparties),
            categories: map(&names.categories),
            parties: map(&names.parties),
            accounts,
            account_names: names
                .accounts
                .iter()
                .map(|a| (a.name.clone(), ()))
                .collect(),
            by_id: names.accounts.iter().map(|a| (a.id, a.clone())).collect(),
        }
    }

    /// What a setter's name stands for, or why it stands for nothing.
    fn resolve(&self, what: Setter, name: &str) -> Result<Uuid, String> {
        let (map, kind) = match what {
            Setter::Label(Label::Counterparty) => (&self.counterparties, "counterparty"),
            Setter::Label(Label::Category) => (&self.categories, "category"),
            Setter::Label(Label::Purpose) => (&self.parties, "party"),
            Setter::Account => {
                return match self.accounts.get(name) {
                    Some(Some(id)) => Ok(*id),
                    Some(None) => Err(format!(
                        "several accounts are named {name:?}: write \"Owner · {name}\""
                    )),
                    None => Err(format!("no account is named {name:?}")),
                };
            }
        };
        map.get(name)
            .copied()
            .ok_or_else(|| format!("no {kind} is named {name:?}"))
    }
}

/// What a setter sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setter {
    Label(Label),
    Account,
}

const SETTERS: [(&str, Setter); 4] = [
    ("counterparty", Setter::Label(Label::Counterparty)),
    ("category", Setter::Label(Label::Category)),
    ("purpose", Setter::Label(Label::Purpose)),
    ("account", Setter::Account),
];

/// Row properties that hold a name, and what kind of name.
const NAMED: [(&str, &str); 3] = [
    ("account", "account"),
    ("account_name", "account name"),
    ("owner", "party"),
];

/// A problem with a script, where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

impl ScriptError {
    fn at(pos: Position, message: impl Into<String>) -> Self {
        Self {
            line: pos.line(),
            column: pos.position(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "line {l}, column {c}: {}", self.message),
            (Some(l), None) => write!(f, "line {l}: {}", self.message),
            _ => f.write_str(&self.message),
        }
    }
}

/// An amount in a script: decimal, never a float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount(pub Decimal);

/// What a script set, each value with the line that set it. The last call
/// of a setter wins.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Set {
    pub counterparty: Option<(Value, usize)>,
    pub category: Option<(Value, usize)>,
    pub purpose: Option<(Value, usize)>,
    pub account: Option<(Uuid, usize)>,
}

impl Set {
    #[must_use]
    pub fn get(&self, label: Label) -> Option<(Value, usize)> {
        match label {
            Label::Counterparty => self.counterparty,
            Label::Category => self.category,
            Label::Purpose => self.purpose,
        }
    }
}

/// The `s` a script's entry points get.
#[derive(Debug, Clone)]
struct Sink {
    lookup: Arc<Lookup>,
    set: Arc<Mutex<Set>>,
}

impl Sink {
    fn set(
        &mut self,
        ctx: &NativeCallContext,
        what: Setter,
        name: Option<&str>,
    ) -> Result<(), Box<EvalAltResult>> {
        let line = ctx.call_position().line().unwrap_or(0);
        let value = match name {
            Some(n) => Some(self.lookup.resolve(what, n)?),
            None => None,
        };
        let mut set = self
            .set
            .lock()
            .map_err(|_| "the script's results are unavailable")?;
        match what {
            Setter::Label(Label::Counterparty) => set.counterparty = Some((value, line)),
            Setter::Label(Label::Category) => set.category = Some((value, line)),
            Setter::Label(Label::Purpose) => set.purpose = Some((value, line)),
            Setter::Account => {
                let id = value.ok_or("an account can't be none")?;
                set.account = Some((id, line));
            }
        }
        Ok(())
    }
}

fn engine() -> Engine {
    let mut e = Engine::new();
    e.set_max_operations(MAX_OPERATIONS)
        .set_max_call_levels(32)
        .set_max_expr_depths(64, 32)
        .set_max_string_size(10_000)
        .set_max_array_size(10_000)
        .set_max_map_size(1_000)
        .set_strict_variables(true)
        .set_fail_on_invalid_map_property(true);
    e.disable_symbol("eval");
    e.on_print(|_| {});
    e.on_debug(|_, _, _| {});

    e.register_type_with_name::<Amount>("Amount");
    e.register_fn("amount", |s: &str| -> Result<Amount, Box<EvalAltResult>> {
        s.trim()
            .parse::<Decimal>()
            .map(Amount)
            .map_err(|_| format!("not an amount: {s:?}").into())
    });
    e.register_fn("to_string", |a: &mut Amount| a.0.to_string());
    e.register_fn("to_debug", |a: &mut Amount| format!("amount(\"{}\")", a.0));
    e.register_fn("abs", |a: Amount| Amount(a.0.abs()));
    e.register_fn("-", |a: Amount| Amount(-a.0));
    e.register_fn(
        "+",
        |a: Amount, b: Amount| -> Result<Amount, Box<EvalAltResult>> {
            a.0.checked_add(b.0)
                .map(Amount)
                .ok_or_else(|| "amount overflow".into())
        },
    );
    e.register_fn(
        "-",
        |a: Amount, b: Amount| -> Result<Amount, Box<EvalAltResult>> {
            a.0.checked_sub(b.0)
                .map(Amount)
                .ok_or_else(|| "amount overflow".into())
        },
    );
    macro_rules! compare {
        ($op:literal, $f:expr) => {
            e.register_fn($op, |a: Amount, b: Amount| $f(&a.0, &b.0));
            e.register_fn($op, |a: Amount, b: i64| $f(&a.0, &Decimal::from(b)));
            e.register_fn($op, |a: i64, b: Amount| $f(&Decimal::from(a), &b.0));
        };
    }
    compare!("==", Decimal::eq);
    compare!("!=", Decimal::ne);
    compare!("<", Decimal::lt);
    compare!("<=", Decimal::le);
    compare!(">", Decimal::gt);
    compare!(">=", Decimal::ge);

    e.register_type_with_name::<Sink>("Setter");
    for (name, what) in SETTERS {
        e.register_fn(
            name,
            move |ctx: NativeCallContext, s: &mut Sink, value: &str| s.set(&ctx, what, Some(value)),
        );
        e.register_fn(name, move |ctx: NativeCallContext, s: &mut Sink, _: ()| {
            s.set(&ctx, what, None)
        });
    }
    e
}

/// A compiled, validated script.
pub struct Script {
    engine: Engine,
    ast: AST,
    lookup: Arc<Lookup>,
    identity: Identity,
    classify: bool,
    account: bool,
}

impl Script {
    /// Compiles and validates `source` as version `version`; `names` are
    /// what it may name.
    pub fn compile(version: i32, source: &str, names: &Names) -> Result<Self, Vec<ScriptError>> {
        if source.len() > MAX_SOURCE {
            return Err(vec![ScriptError {
                line: None,
                column: None,
                message: format!("the script is longer than {MAX_SOURCE} bytes"),
            }]);
        }
        let engine = engine();
        let ast = engine
            .compile(source)
            .map_err(|e| vec![ScriptError::at(e.position(), e.err_type().to_string())])?;
        let lookup = Lookup::new(names);
        let mut errors = Vec::new();
        let (mut classify, mut account) = (false, false);
        for f in ast.iter_functions() {
            let entry = match f.name {
                "classify" => Some((&mut classify, "classify(row, s)")),
                "account" => Some((&mut account, "account(file, s)")),
                _ => None,
            };
            if let Some((found, signature)) = entry {
                if f.params.len() == 2 {
                    *found = true;
                } else {
                    errors.push(ScriptError {
                        line: None,
                        column: None,
                        message: format!("{} must be fn {signature}", f.name),
                    });
                }
            }
        }
        ast.walk(&mut |path: &[ASTNode]| {
            if let Some(ASTNode::Expr(expr)) = path.last() {
                check(expr, &lookup, &mut errors);
            }
            true
        });
        if !errors.is_empty() {
            errors.sort_by_key(|e| (e.line, e.column));
            return Err(errors);
        }
        Ok(Self {
            engine,
            ast,
            lookup: Arc::new(lookup),
            identity: Identity {
                kind: Kind::Rule,
                name: "rules".to_owned(),
                version: version.to_string(),
                details: serde_json::json!({}),
            },
            classify,
            account,
        })
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.identity.version
    }

    fn sink(&self) -> Sink {
        Sink {
            lookup: self.lookup.clone(),
            set: Arc::default(),
        }
    }

    fn run(&self, entry: &str, arg: Map, sink: &Sink) -> Result<Set, String> {
        let _: Dynamic = self
            .engine
            .call_fn(&mut Scope::new(), &self.ast, entry, (arg, sink.clone()))
            .map_err(|e| e.to_string())?;
        sink.set
            .lock()
            .map(|s| s.clone())
            .map_err(|_| "the script's results are unavailable".to_owned())
    }

    /// What `classify` sets for a row; nothing without one.
    pub fn classify(&self, row: &Row) -> Result<Set, String> {
        if !self.classify {
            return Ok(Set::default());
        }
        self.run("classify", self.row(row), &self.sink())
    }

    /// The account `account` sets for a statement, with its line.
    pub fn account_of(&self, s: &Statement) -> Result<Option<(Uuid, usize)>, String> {
        if !self.account {
            return Ok(None);
        }
        let mut file = Map::new();
        file.insert(
            "name".into(),
            s.file_name.clone().map_or(Dynamic::UNIT, Dynamic::from),
        );
        file.insert(
            "kind".into(),
            match s.kind {
                Some(suggest::StatementKind::Bank) => "bank".into(),
                Some(suggest::StatementKind::CreditCard) => "credit_card".into(),
                None => Dynamic::UNIT,
            },
        );
        let rows: rhai::Array = s.rows.iter().map(|r| self.row(r).into()).collect();
        file.insert("rows".into(), rows.into());
        Ok(self.run("account", file, &self.sink())?.account)
    }

    /// A row as scripts see it.
    fn row(&self, r: &Row) -> Map {
        let mut m = Map::new();
        let account = r.account.and_then(|a| self.lookup.by_id.get(&a));
        let name = |v: Option<String>| v.map_or(Dynamic::UNIT, Dynamic::from);
        m.insert("description".into(), r.description.clone().into());
        m.insert("amount".into(), Dynamic::from(Amount(r.amount)));
        m.insert("date".into(), r.date.to_string().into());
        m.insert("year".into(), i64::from(r.date.year()).into());
        m.insert("month".into(), i64::from(u8::from(r.date.month())).into());
        m.insert("day".into(), i64::from(r.date.day()).into());
        m.insert(
            "weekday".into(),
            i64::from(r.date.weekday().number_from_monday()).into(),
        );
        m.insert("account".into(), name(account.map(AccountName::shown)));
        m.insert("account_name".into(), name(account.map(|a| a.name.clone())));
        m.insert("owner".into(), name(account.map(|a| a.owner.clone())));
        let mut raw = Map::new();
        if let Some(o) = r.raw.as_object() {
            for (k, v) in o {
                let v = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => continue,
                    other => other.to_string(),
                };
                raw.insert(k.as_str().into(), v.into());
            }
        }
        m.insert("raw".into(), raw.into());
        m
    }
}

/// Checks the names an expression uses.
fn check(expr: &Expr, lookup: &Lookup, errors: &mut Vec<ScriptError>) {
    match expr {
        Expr::MethodCall(call, pos) => {
            let Some((name, what)) = SETTERS.iter().find(|s| s.0 == call.name.as_str()) else {
                return;
            };
            match call.args.first() {
                Some(Expr::StringConstant(value, _)) => {
                    if let Err(e) = lookup.resolve(*what, value) {
                        errors.push(ScriptError::at(*pos, e));
                    }
                }
                Some(Expr::Unit(_)) if *what != Setter::Account => {}
                _ => errors.push(ScriptError::at(
                    *pos,
                    format!(
                        "{name}(…) takes a name written out (\"…\"){}, so it can be checked",
                        if *what == Setter::Account {
                            ""
                        } else {
                            " or ()"
                        }
                    ),
                )),
            }
        }
        Expr::FnCall(call, pos)
            if (call.name == "==" || call.name == "!=") && call.args.len() == 2 =>
        {
            let property = |e: &Expr| match e {
                Expr::Dot(x, ..) => match &x.rhs {
                    Expr::Property(p, ..) => Some(p.2.to_string()),
                    _ => None,
                },
                _ => None,
            };
            let (a, b) = (&call.args[0], &call.args[1]);
            let (prop, value) = match (property(a), b, property(b), a) {
                (Some(p), Expr::StringConstant(v, _), ..)
                | (_, _, Some(p), Expr::StringConstant(v, _)) => (p, v),
                _ => return,
            };
            let Some((_, kind)) = NAMED.iter().find(|n| n.0 == prop) else {
                return;
            };
            let known = match prop.as_str() {
                "account" => lookup.accounts.contains_key(value.as_str()),
                "account_name" => lookup.account_names.contains_key(value.as_str()),
                _ => lookup.parties.contains_key(value.as_str()),
            };
            if !known {
                errors.push(ScriptError::at(
                    *pos,
                    format!("no {kind} is named {:?}", value.as_str()),
                ));
            }
        }
        _ => {}
    }
}

impl Suggester for Script {
    fn identity(&self) -> &Identity {
        &self.identity
    }

    /// A row the script fails on gets no suggestions.
    fn labels(&self, rows: &[Row]) -> Vec<Proposal> {
        rows.iter()
            .map(|r| {
                let mut p = Proposal::default();
                if let Ok(set) = self.classify(r) {
                    for l in Label::ALL {
                        if let Some((value, line)) = set.get(l) {
                            p.set(
                                l,
                                Some(Guess {
                                    value,
                                    confidence: 1000,
                                    reason: format!("rule v{}:{line}", self.version()),
                                }),
                            );
                        }
                    }
                }
                p
            })
            .collect()
    }

    fn account(&self, statement: &Statement) -> Option<Guess<Uuid>> {
        let (account, line) = self.account_of(statement).ok()??;
        Some(Guess {
            value: account,
            confidence: 1000,
            reason: format!("rule v{}:{line}", self.version()),
        })
    }
}

#[cfg(test)]
mod tests;
