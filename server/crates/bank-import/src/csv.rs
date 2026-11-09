//! CSV statements: a header row naming a date, a description and an amount
//! (under any of several common names), then one row per transaction. Lines
//! before the header, such as a summary block, are ignored. A port of the
//! preimporter's `csv_processor`, so the same file reads the same way.
//!
//! A negative amount is money leaving the account. Some card statements sign
//! amounts the other way and add a debit/credit column; when a row says `D`
//! or `C` there, that sets the sign instead.

use ledger::Amount;
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use time::{Date, Month};

use crate::{Format, Row, Skipped, Statement};

/// Header names for each column, in order of preference (compared trimmed
/// and lowercased).
const DATE: &[&str] = &[
    "date",
    "posted date",
    "posted_date",
    "posteddate",
    "posting date",
    "transaction date",
    "transaction_date",
    "transactiondate",
    "trans. date",
];
const DESCRIPTION: &[&str] = &["description", "payee", "memo"];
const AMOUNT: &[&str] = &["amount", "transaction amount"];
const REFERENCE: &[&str] = &[
    "transaction id",
    "transaction_id",
    "reference number",
    "reference",
    "reference_number",
    "reference id",
];
const DIRECTION: &[&str] = &["transaction type"];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the CSV file isn't valid UTF-8")]
    Encoding,
    #[error("no header row names a date, a description and an amount")]
    NoHeader,
    #[error("CSV line {line}: {message}")]
    Malformed { line: u64, message: String },
}

struct Columns {
    names: Vec<String>,
    date: usize,
    description: usize,
    amount: usize,
    reference: Option<usize>,
    direction: Option<usize>,
}

fn find(header: &[String], aliases: &[&str]) -> Option<usize> {
    aliases
        .iter()
        .find_map(|alias| header.iter().position(|h| h == alias))
}

fn columns(record: &::csv::StringRecord) -> Option<Columns> {
    let header: Vec<String> = record.iter().map(|c| c.trim().to_lowercase()).collect();
    Some(Columns {
        date: find(&header, DATE)?,
        description: find(&header, DESCRIPTION)?,
        amount: find(&header, AMOUNT)?,
        reference: find(&header, REFERENCE),
        direction: find(&header, DIRECTION),
        names: record.iter().map(|c| c.trim().to_owned()).collect(),
    })
}

fn reader(text: &str) -> ::csv::Reader<&[u8]> {
    ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes())
}

fn text(bytes: &[u8]) -> Option<&str> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    std::str::from_utf8(bytes).ok()
}

/// Whether the bytes are a CSV with a recognizable header row near the top.
#[must_use]
pub fn looks_like(bytes: &[u8]) -> bool {
    text(bytes).is_some_and(|t| {
        reader(t)
            .records()
            .take(30)
            .map_while(Result::ok)
            .any(|r| columns(&r).is_some())
    })
}

/// `01/31/2024`, `01/31/24` or `2024-01-31`. Two-digit years are 1969-2068,
/// as in Python's `strptime`.
fn date(s: &str) -> Option<Date> {
    let s = s.trim();
    let num = |p: &str| p.parse::<u32>().ok();
    let (y, m, d) = if let [y, m, d] = s.split('-').collect::<Vec<_>>()[..] {
        (y.len() == 4).then_some(())?;
        (num(y)?, num(m)?, num(d)?)
    } else if let [m, d, y] = s.split('/').collect::<Vec<_>>()[..] {
        let year = match y.len() {
            4 => num(y)?,
            2 => num(y).map(|y| if y < 69 { 2000 + y } else { 1900 + y })?,
            _ => return None,
        };
        (year, num(m)?, num(d)?)
    } else {
        return None;
    };
    let month = Month::try_from(u8::try_from(m).ok()?).ok()?;
    Date::from_calendar_date(i32::try_from(y).ok()?, month, u8::try_from(d).ok()?).ok()
}

/// `-1,234.50`, `$1,234.50`, `(1,234.50)` (negative). Never through a float.
fn amount(s: &str) -> Option<Amount> {
    let mut v: String = s
        .chars()
        .filter(|c| *c != ',' && !c.is_whitespace())
        .collect();
    let mut negative = false;
    if let Some(inner) = v.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        negative = true;
        v = inner.to_owned();
    }
    if let Some(rest) = v.strip_prefix('-') {
        negative = !negative;
        v = rest.to_owned();
    } else if let Some(rest) = v.strip_prefix('+') {
        v = rest.to_owned();
    }
    let v = v.strip_prefix('$').unwrap_or(&v);
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    let valid = match v.split_once('.') {
        Some((i, f)) => digits(i) && digits(f),
        None => digits(v),
    };
    if !valid {
        return None;
    }
    let d = Decimal::from_str_exact(v).ok()?;
    Some(Amount::from_decimal(if negative { -d } else { d }))
}

/// Reads the statement. Rows missing a date, description or amount (such as
/// a "beginning balance" line) are passed over; rows with one that can't be
/// read are listed in `skipped`.
pub fn parse(bytes: &[u8]) -> Result<Statement, Error> {
    let text = text(bytes).ok_or(Error::Encoding)?;
    let mut cols: Option<Columns> = None;
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for record in reader(text).records() {
        let record = record.map_err(|e| Error::Malformed {
            line: e.position().map_or(0, ::csv::Position::line),
            message: e.to_string(),
        })?;
        let line = record.position().map_or(0, ::csv::Position::line);
        if record.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let Some(c) = &cols else {
            cols = columns(&record);
            continue;
        };
        let cell = |i: usize| record.get(i).unwrap_or_default().trim();
        let (d, desc, amt) = (cell(c.date), cell(c.description), cell(c.amount));
        if d.is_empty() || desc.is_empty() || amt.is_empty() {
            continue;
        }
        let (Some(date), Some(amount)) = (date(d), amount(amt)) else {
            let what = if date(d).is_none() { "date" } else { "amount" };
            skipped.push(Skipped {
                line,
                reason: format!("unreadable {what}"),
            });
            continue;
        };
        let size = Amount::from_decimal(amount.as_decimal().abs());
        let amount = match c.direction.map(cell) {
            Some("D" | "d") => -size,
            Some("C" | "c") => size,
            _ => amount,
        };
        let raw: Map<String, Value> = c
            .names
            .iter()
            .enumerate()
            .filter(|(_, name)| !name.is_empty())
            .map(|(i, name)| (name.clone(), Value::String(cell(i).to_owned())))
            .collect();
        rows.push(Row {
            date,
            amount,
            description: desc.to_owned(),
            native_id: c
                .reference
                .map(cell)
                .filter(|r| !r.is_empty())
                .map(str::to_owned),
            raw,
        });
    }
    if cols.is_none() {
        return Err(Error::NoHeader);
    }
    Ok(Statement {
        format: Format::Csv,
        account: None,
        kind: None,
        currency: None,
        ledger: None,
        rows,
        skipped,
    })
}

#[cfg(test)]
mod tests;
