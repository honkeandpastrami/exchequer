//! Bank file import (design §6.1): find out what a file is from its content
//! and parse it into statements of normalized rows. No database and no
//! network: the server decides what the rows mean.

pub mod csv;
pub mod fingerprint;
pub mod ofx;
mod statements;

use domain::{AccountNumber, Amount};
pub use jiff::tz::TimeZone;
use time::Date;

pub use statements::parse_ofx;

/// What a file is, judged by its content (never its name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// OFX 1.x (SGML) or 2.x (XML); QFX and QBO are OFX too.
    Ofx,
    /// A CSV with a header row naming a date, a description and an amount.
    Csv,
}

/// What kind of account an OFX statement says it's for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    Bank,
    CreditCard,
}

/// The balance the bank reported at the end of the statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerBalance {
    pub amount: Amount,
    pub date: Date,
}

/// One account's rows from a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub format: Format,
    /// The account number in the file, if it has one (CSVs don't).
    pub account: Option<AccountNumber>,
    pub kind: Option<StatementKind>,
    /// The currency the file declares, e.g. `USD`.
    pub currency: Option<String>,
    pub ledger: Option<LedgerBalance>,
    pub rows: Vec<Row>,
    /// Lines that look like data but couldn't be read, so nothing is dropped
    /// silently.
    pub skipped: Vec<Skipped>,
}

/// One transaction as the bank wrote it, normalized.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The date in the server's time zone (see [`parse_ofx`]); CSV dates
    /// are taken as written.
    pub date: Date,
    /// Signed from the account's side: negative is money leaving it.
    pub amount: Amount,
    pub description: String,
    /// The bank's own ID for the row (OFX `FITID`, a CSV reference column).
    pub native_id: Option<String>,
    /// The row's fields as read, kept as the transaction's provenance.
    pub raw: serde_json::Map<String, serde_json::Value>,
}

/// A line that wasn't read, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// 1-based line (CSV) or transaction (OFX) number.
    pub line: u64,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the file is empty")]
    Empty,
    #[error("not a format Exchequer reads (OFX, QFX, QBO or CSV with a header row)")]
    UnknownFormat,
    #[error(transparent)]
    Ofx(#[from] ofx::Error),
    #[error(transparent)]
    Csv(#[from] csv::Error),
    #[error("the file has no statements (no activity in the period?)")]
    NoStatements,
}

/// The file's format, from its first bytes.
#[must_use]
pub fn detect(bytes: &[u8]) -> Option<Format> {
    let start = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let head = &start[..start.len().min(4096)];
    let upper: Vec<u8> = head.to_ascii_uppercase();
    let has = |needle: &[u8]| upper.windows(needle.len()).any(|w| w == needle);
    if has(b"OFXHEADER") || has(b"<OFX>") || has(b"<?OFX") {
        return Some(Format::Ofx);
    }
    csv::looks_like(start).then_some(Format::Csv)
}

/// Parses a whole file into its statements, with dates local to `zone`
/// (the server's time zone).
pub fn parse(bytes: Vec<u8>, zone: &TimeZone) -> Result<Vec<Statement>, Error> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(Error::Empty);
    }
    let statements = match detect(&bytes) {
        Some(Format::Ofx) => parse_ofx(bytes, zone)?,
        Some(Format::Csv) => vec![csv::parse(&bytes)?],
        None => return Err(Error::UnknownFormat),
    };
    if statements.is_empty() {
        return Err(Error::NoStatements);
    }
    Ok(statements)
}
