//! Dev tool (build step 0.3): does `ofx-rs` read a real bank file the same way
//! the importer in use today does?
//!
//! Usage: `ofx-check <statement.ofx|.qfx|.qbo> <reference.csv>`
//!
//! The reference CSV has the columns `transaction_id,date,description,amount`
//! and is produced by the existing tool from the same file. Real bank files are
//! read from wherever they are; nothing is copied, and account numbers are
//! printed as their last four characters only.

use std::collections::HashMap;
use std::error::Error;
use std::process::ExitCode;

use ofx_rs::aggregates::{LedgerBalance, StatementTransaction};
use rust_decimal::Decimal;
use time::{Date, UtcOffset};

/// One transaction, normalized the way the existing importer normalizes it.
struct Normalized {
    id: String,
    /// Posted date converted to UTC, which is what the existing importer stores.
    utc_date: Date,
    /// Posted date in the offset written in the file (the bank's local date).
    local_date: Date,
    description: String,
    amount: Decimal,
}

/// One statement found in the file.
struct Statement {
    kind: &'static str,
    account_last4: String,
    currency: String,
    ledger: Option<(Decimal, Date)>,
    transactions: Vec<Normalized>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let [_, ofx_path, reference_path] = args.as_slice() else {
        eprintln!("usage: ofx-check <statement.ofx> <reference.csv>");
        return ExitCode::from(2);
    };
    match run(ofx_path, reference_path) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Returns `Ok(true)` when every transaction matches the reference.
fn run(ofx_path: &str, reference_path: &str) -> Result<bool, Box<dyn Error>> {
    let bytes = std::fs::read(ofx_path)?;
    // `ofx-rs` parses text, so decoding is our job. Some banks declare a
    // legacy charset in the header but actually write UTF-8; strict UTF-8
    // decoding tells us which case a file is in.
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => return Err(format!("file is not valid UTF-8 ({err})").into()),
    };

    let doc = ofx_rs::parse(&text)?;
    let statements = collect_statements(&doc);
    let reference = read_reference(reference_path)?;

    println!("file: {ofx_path}");
    println!("  OFX header version: {}", doc.header().version());
    let mut ours: Vec<&Normalized> = Vec::new();
    for st in &statements {
        let ledger = match st.ledger {
            Some((amount, date)) => format!("{amount} as of {date}"),
            None => "none".to_owned(),
        };
        println!(
            "  {} statement, account …{}, {}, {} transactions, ledger balance: {ledger}",
            st.kind,
            st.account_last4,
            st.currency,
            st.transactions.len()
        );
        ours.extend(st.transactions.iter());
    }

    Ok(compare(&ours, &reference))
}

fn collect_statements(doc: &ofx_rs::OfxDocument) -> Vec<Statement> {
    let mut out = Vec::new();
    if let Some(banking) = doc.banking() {
        for wrapper in banking.statement_responses() {
            if let Some(resp) = wrapper.response() {
                out.push(Statement {
                    kind: "bank",
                    account_last4: last4(resp.bank_account().account_id().as_str()),
                    currency: resp.currency_default().to_string(),
                    ledger: resp.ledger_balance().map(ledger),
                    transactions: normalize(resp.transaction_list().map(|l| l.transactions())),
                });
            }
        }
    }
    if let Some(cc) = doc.credit_card() {
        for wrapper in cc.statement_responses() {
            if let Some(resp) = wrapper.response() {
                out.push(Statement {
                    kind: "credit card",
                    account_last4: last4(resp.credit_card_account().account_id().as_str()),
                    currency: resp.currency_default().to_string(),
                    ledger: resp.ledger_balance().map(ledger),
                    transactions: normalize(resp.transaction_list().map(|l| l.transactions())),
                });
            }
        }
    }
    out
}

fn ledger(bal: &LedgerBalance) -> (Decimal, Date) {
    (bal.amount().as_decimal(), bal.as_of().as_offset_date_time().date())
}

/// Mirrors the existing importer: description is NAME, else MEMO, else
/// "Transaction"; the ID is the FITID, with `-2`, `-3`… appended when the same
/// ID repeats within one file.
fn normalize(txns: Option<&[StatementTransaction]>) -> Vec<Normalized> {
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut out = Vec::new();
    for t in txns.unwrap_or_default() {
        let posted = t.date_posted().as_offset_date_time();
        let description = [t.name(), t.memo()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|s| !s.is_empty())
            .unwrap_or("Transaction")
            .to_owned();

        let base = t.fit_id().as_str().to_owned();
        let count = seen.entry(base.clone()).or_insert(0);
        *count += 1;
        let id = if *count == 1 { base } else { format!("{base}-{count}") };

        out.push(Normalized {
            id,
            utc_date: posted.to_offset(UtcOffset::UTC).date(),
            local_date: posted.date(),
            description,
            amount: t.amount().as_decimal(),
        });
    }
    out
}

struct Reference {
    date: String,
    description: String,
    amount: Decimal,
}

fn read_reference(path: &str) -> Result<HashMap<String, Reference>, Box<dyn Error>> {
    let mut map = HashMap::new();
    for record in csv::Reader::from_path(path)?.records() {
        let r = record?;
        let field = |i: usize| r.get(i).unwrap_or_default().to_owned();
        // Parsed straight from text into an exact decimal; never a float.
        let amount: Decimal = field(3).parse()?;
        map.insert(field(0), Reference { date: field(1), description: field(2), amount });
    }
    Ok(map)
}

/// Prints every difference and returns whether there were none.
fn compare(ours: &[&Normalized], reference: &HashMap<String, Reference>) -> bool {
    let mut problems = 0;
    let mut local_date_differs = 0;
    for n in ours {
        if n.local_date != n.utc_date {
            local_date_differs += 1;
        }
        let Some(r) = reference.get(&n.id) else {
            problems += 1;
            println!("  MISSING in reference: id {}", short(&n.id));
            continue;
        };
        let mut diffs = Vec::new();
        if r.date != n.utc_date.to_string() {
            diffs.push(format!("date {} vs {}", n.utc_date, r.date));
        }
        if r.description != n.description {
            diffs.push(format!("description: {}", first_difference(&n.description, &r.description)));
        }
        // Decimal equality ignores scale, so -80 equals -80.00. The reference
        // rounds to cents; a difference here means the bank sent more decimals.
        if r.amount != n.amount {
            diffs.push(format!("amount {} vs {}", n.amount, r.amount));
        }
        if !diffs.is_empty() {
            problems += 1;
            println!("  DIFF id {}: {}", short(&n.id), diffs.join(", "));
        }
    }
    let ours_ids: std::collections::HashSet<&str> = ours.iter().map(|n| n.id.as_str()).collect();
    for id in reference.keys().filter(|id| !ours_ids.contains(id.as_str())) {
        problems += 1;
        println!("  MISSING in ofx-rs output: id {}", short(id));
    }

    println!(
        "  result: {} transactions, {} reference rows, {problems} differences",
        ours.len(),
        reference.len()
    );
    if local_date_differs > 0 {
        println!(
            "  note: {local_date_differs} transactions have a different date in the bank's own \
             offset than in UTC (the reference uses UTC)"
        );
    }
    problems == 0
}

/// Describes where two descriptions diverge without printing them: the
/// character position and the Unicode code points on each side.
fn first_difference(ours: &str, reference: &str) -> String {
    let (a, b): (Vec<char>, Vec<char>) = (ours.chars().collect(), reference.chars().collect());
    let at = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let show = |c: Option<&char>| c.map_or("end".to_owned(), |c| format!("U+{:04X}", u32::from(*c)));
    format!(
        "at char {at}: ofx-rs {} vs reference {} (lengths {} vs {})",
        show(a.get(at)),
        show(b.get(at)),
        a.len(),
        b.len()
    )
}

fn last4(account: &str) -> String {
    let chars: Vec<char> = account.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
}

/// IDs can be long (some banks embed the amount and payee); keep output short.
fn short(id: &str) -> String {
    if id.chars().count() > 16 { format!("{}…", id.chars().take(16).collect::<String>()) } else { id.to_owned() }
}
