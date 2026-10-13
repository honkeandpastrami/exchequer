//! Dev tool: backtests a local method on a history file, the way
//! `GET /suggesters/backtest` does on the server's own ledger. For trying a
//! method on a copy of real data, read from a local path given at runtime;
//! real data never enters the repo.
//!
//! ```text
//! cargo run --release -p heuristics --example backtest -- history.json 2026-01 2026-08 [method]
//! ```
//!
//! With `BACKTEST_DUMP=out.json`, each row's guesses are written too, for
//! looking into what the method gets wrong.
//!
//! The file: `{"accounts": [{"id", "kind", "owner", "archived"}], "examples":
//! [{"transaction", "account", "date", "amount", "description",
//! "counterparty", "category", "purpose"}]}`, IDs as UUID strings, the
//! amount a signed decimal string, labels `null` for none.
#![allow(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    clippy::cast_precision_loss,
    reason = "percentages for printing"
)]

use std::time::Instant;

use rust_decimal::Decimal;
use serde_json::Value;
use suggest::backtest::{BINS, Score};
use suggest::{Account, Example, History, Label, Labels};
use time::{Date, Month};
use uuid::Uuid;

fn date(s: &str) -> Date {
    let p: Vec<i32> = s.split('-').map(|x| x.parse().expect("a date")).collect();
    let month = Month::try_from(u8::try_from(p[1]).expect("a month")).expect("a month");
    let day = p.get(2).copied().unwrap_or(1);
    Date::from_calendar_date(p[0], month, u8::try_from(day).expect("a day")).expect("a date")
}

fn uuid(v: &Value) -> Option<Uuid> {
    v.as_str().map(|s| s.parse().expect("a UUID"))
}

fn pct(n: u32, of: u32) -> String {
    if of == 0 {
        "-".to_owned()
    } else {
        format!("{:.1}%", f64::from(n) * 100.0 / f64::from(of))
    }
}

fn line(name: &str, s: &Score) {
    println!(
        "  {name:<13} asked {:>5}  guessed {:>6}  right {:>6}  confident {:>6} of asked, right {:>6}",
        s.asked,
        pct(s.guessed, s.asked),
        pct(s.correct, s.guessed),
        pct(s.confident, s.asked),
        pct(s.confident_correct, s.confident),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, path, from, to, rest @ ..] = args.as_slice() else {
        eprintln!("usage: backtest <history.json> <from YYYY-MM> <to YYYY-MM> [method]");
        std::process::exit(2);
    };
    let method = match rest.first() {
        Some(id) => heuristics::method(id).expect("a known method"),
        None => heuristics::METHODS[0],
    };
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("readable")).expect("JSON");
    let history = History {
        accounts: file["accounts"]
            .as_array()
            .expect("accounts")
            .iter()
            .map(|a| Account {
                id: uuid(&a["id"]).expect("an ID"),
                kind: a["kind"].as_str().expect("a kind").to_owned(),
                owner: uuid(&a["owner"]).expect("an owner"),
                archived: a["archived"].as_bool().unwrap_or(false),
            })
            .collect(),
        examples: file["examples"]
            .as_array()
            .expect("examples")
            .iter()
            .map(|e| Example {
                transaction: uuid(&e["transaction"]).expect("an ID"),
                account: uuid(&e["account"]).expect("an account"),
                date: date(e["date"].as_str().expect("a date")),
                amount: e["amount"]
                    .as_str()
                    .expect("an amount")
                    .parse::<Decimal>()
                    .expect("a decimal"),
                description: e["description"].as_str().unwrap_or_default().to_owned(),
                labels: Labels {
                    counterparty: uuid(&e["counterparty"]),
                    category: uuid(&e["category"]),
                    purpose: uuid(&e["purpose"]),
                },
            })
            .collect(),
    };
    println!("{}: {} examples", method.id(), history.examples.len());
    let (mut month, last) = (date(from), date(to));
    let mut totals: Vec<Score> = vec![Score::default(); 4];
    let mut dump: Vec<Value> = Vec::new();
    while month <= last {
        let next = date(&format!(
            "{}-{}",
            month.year() + i32::from(month.month() == Month::December),
            u8::from(month.month().next())
        ));
        let started = Instant::now();
        let r = suggest::backtest::run(method, &history, month, next);
        if std::env::var_os("BACKTEST_DUMP").is_some() {
            dump.extend(guesses(method, &history, month, next));
        }
        println!(
            "{} (trained on {}, {:.1}s)",
            &month.to_string()[..7],
            r.trained_on,
            started.elapsed().as_secs_f64()
        );
        for (i, l) in Label::ALL.into_iter().enumerate() {
            line(l.as_str(), &r.labels[&l]);
            add(&mut totals[i], &r.labels[&l]);
        }
        line("account", &r.accounts);
        add(&mut totals[3], &r.accounts);
        month = next;
    }
    println!("all months");
    for (i, l) in Label::ALL.into_iter().enumerate() {
        line(l.as_str(), &totals[i]);
    }
    line("account", &totals[3]);
    if let Some(path) = std::env::var_os("BACKTEST_DUMP") {
        std::fs::write(path, serde_json::to_string(&dump).expect("JSON")).expect("writable");
    }
    println!("calibration (all labels): confidence bin, guesses, right");
    for b in 0..BINS {
        let (g, r) = totals[..3]
            .iter()
            .fold((0, 0), |(g, r), s| (g + s.bins[b].0, r + s.bins[b].1));
        println!(
            "  {:>3}-{:<4} {g:>6} {:>6}",
            b * 100,
            b * 100 + 99,
            pct(r, g)
        );
    }
}

/// Each transaction in `[start, end)` with the method's guesses, trained on
/// what came before.
fn guesses(method: &dyn suggest::Method, history: &History, start: Date, end: Date) -> Vec<Value> {
    let s = method.train(&history.before(start));
    let tests: Vec<&Example> = history
        .examples
        .iter()
        .filter(|e| e.date >= start && e.date < end)
        .collect();
    let rows: Vec<suggest::Row> = tests
        .iter()
        .map(|e| suggest::Row {
            account: Some(e.account),
            date: e.date,
            amount: e.amount,
            description: e.description.clone(),
            raw: Value::Null,
            facts: suggest::Facts::default(),
        })
        .collect();
    tests
        .iter()
        .zip(s.labels(&rows))
        .map(|(e, p)| {
            let mut out = serde_json::Map::new();
            out.insert("transaction".into(), e.transaction.to_string().into());
            for l in Label::ALL {
                out.insert(
                    l.as_str().into(),
                    p.get(l).map_or(Value::Null, |g| {
                        serde_json::json!({
                            "value": g.value.map(|v| v.to_string()),
                            "confidence": g.confidence,
                            "reason": g.reason,
                        })
                    }),
                );
            }
            Value::Object(out)
        })
        .collect()
}

fn add(total: &mut Score, s: &Score) {
    total.asked += s.asked;
    total.guessed += s.guessed;
    total.correct += s.correct;
    total.confident += s.confident;
    total.confident_correct += s.confident_correct;
    for (t, b) in total.bins.iter_mut().zip(s.bins) {
        t.0 += b.0;
        t.1 += b.1;
    }
}
