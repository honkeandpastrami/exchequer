//! OFX/QFX/QBO parsing: `ofx-rs` plus a hot-patch for two bugs in
//! `ofx-rs` 0.2.0. Delete the patch once
//! upstream fixes them.
//!
//! 1. **Bank-specific tags inside a transaction.** In OFX 1.x (SGML),
//!    `ofx-rs` closes a `<STMTTRN>` at the first tag it doesn't know, so a
//!    later `<FITID>` lands outside the transaction and parsing fails with
//!    "FITID is required". Before parsing, such tags are lifted out of every
//!    transaction and returned separately as [`Extension`]s, keyed by FITID.
//!    (Some banks put useful data there, e.g. a merchant category code.)
//! 2. **Entities in text are dropped.** `ofx-rs` discards entity references
//!    such as `&amp;`, together with the whitespace around them.
//! 3. **Non-ASCII text is mangled in OFX 1.x.** The SGML converter copies text
//!    byte by byte into `char`s, so `É` becomes two garbage characters.
//!
//! Fix for 2 and 3: before parsing, `&`, every non-ASCII character, and the
//! escape marker itself are rewritten as reversible ASCII escapes (`^26;`,
//! `^C9;`, `^5E;`), so `ofx-rs` only ever sees plain ASCII text. Callers pass
//! each string they read from the document through [`restore`], which undoes
//! the escapes and then decodes XML entities once (`&amp;` → `&`,
//! `&amp;amp;` → `&amp;`).

use std::collections::HashMap;
use std::fmt;

pub use ofx_rs::OfxDocument;

/// Marks an escape while `ofx-rs` parses: `^` + hex code point + `;`.
const ESC: char = '^';

/// Tags `ofx-rs` accepts directly inside `<STMTTRN>`: its leaf fields, its
/// child aggregates, and the leaves of those aggregates.
const KNOWN_IN_STMTTRN: &[&str] = &[
    // leaves
    "TRNTYPE",
    "DTPOSTED",
    "DTUSER",
    "DTAVAIL",
    "TRNAMT",
    "FITID",
    "SRVRTID",
    "CHECKNUM",
    "REFNUM",
    "SIC",
    "PAYEEID",
    "NAME",
    "EXTDBINFON",
    "MEMO",
    "CORRECTFITID",
    "CORRECTACTION",
    "INV401KSOURCE",
    // child aggregates
    "PAYEE",
    "BANKACCTTO",
    "CCACCTTO",
    "CURRENCY",
    "ORIGCURRENCY",
    // leaves of those aggregates
    "ADDR1",
    "ADDR2",
    "ADDR3",
    "CITY",
    "STATE",
    "POSTALCODE",
    "COUNTRY",
    "PHONE",
    "BANKID",
    "BRANCHID",
    "ACCTID",
    "ACCTTYPE",
    "ACCTKEY",
    "CURRATE",
    "CURSYM",
];

/// A bank-specific field lifted out of a transaction, e.g. `MCC` = `5411`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    pub tag: String,
    pub value: String,
}

/// A parsed document plus the bank-specific fields of each transaction,
/// keyed by its (restored) FITID.
pub struct Parsed {
    pub document: OfxDocument,
    pub extensions: HashMap<String, Vec<Extension>>,
    /// Each transaction's `DTPOSTED` as written, by FITID in file order:
    /// `ofx-rs` reads a time without a zone as UTC, losing that it had none.
    pub posted: HashMap<String, Vec<String>>,
    /// Every `DTASOF` (balance dates) as written, in file order.
    pub as_of: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    /// The file isn't valid UTF-8.
    Encoding(std::string::FromUtf8Error),
    /// `ofx-rs` rejected the (patched) document.
    Ofx(ofx_rs::OfxError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(e) => write!(f, "file is not valid UTF-8: {e}"),
            Self::Ofx(e) => write!(f, "OFX parse error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Parses raw file bytes. Banks sometimes declare a legacy charset in the
/// header but write UTF-8, so the bytes are decoded as strict UTF-8
/// regardless of the header.
pub fn parse(bytes: Vec<u8>) -> Result<Parsed, Error> {
    let text = String::from_utf8(bytes).map_err(Error::Encoding)?;
    let (stripped, extensions, posted, as_of) = lift_extensions(&text);
    let document = ofx_rs::parse(&escape(&stripped)).map_err(Error::Ofx)?;
    Ok(Parsed {
        document,
        extensions,
        posted,
        as_of,
    })
}

/// Turns a string read from a parsed document back into real text: undoes
/// the escapes and decodes XML entities once. Unknown entities are left as
/// written.
pub fn restore(s: &str) -> String {
    decode_entities(&unescape(s))
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '&' || c == ESC || !c.is_ascii() {
            out.push_str(&format!("{ESC}{:X};", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(ESC) {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        let decoded = tail.find(';').and_then(|end| {
            u32::from_str_radix(&tail[..end], 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| (c, end + 1))
        });
        match decoded {
            Some((c, consumed)) => {
                out.push(c);
                rest = &tail[consumed..];
            }
            // Can't happen for text that went through `escape`; keep it verbatim.
            None => {
                out.push(ESC);
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let decoded = tail.find(';').and_then(|end| {
            let name = &tail[1..end];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .map(|hex| u32::from_str_radix(hex, 16))
                    .or_else(|| name.strip_prefix('#').map(str::parse::<u32>))
                    .and_then(Result::ok)
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, end + 1))
        });
        match decoded {
            Some((c, consumed)) => {
                out.push(c);
                rest = &tail[consumed..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Removes unknown tags (and their values) from inside every `<STMTTRN>`,
/// returning the cleaned text and the removed fields keyed by FITID.
type Lifted = (
    String,
    HashMap<String, Vec<Extension>>,
    HashMap<String, Vec<String>>,
    Vec<String>,
);

fn lift_extensions(text: &str) -> Lifted {
    let mut out = String::with_capacity(text.len());
    let mut extensions: HashMap<String, Vec<Extension>> = HashMap::new();
    let mut posted: HashMap<String, Vec<String>> = HashMap::new();
    let mut dtposted: Option<String> = None;
    let mut as_of: Vec<String> = Vec::new();
    let mut in_trn = false;
    let mut fitid: Option<String> = None;
    let mut pending: Vec<Extension> = Vec::new();
    let mut rest = text;

    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        let Some(close) = tail.find('>') else { break };
        let tag = &tail[1..close];
        let name = tag
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or_default();
        let is_end = tag.starts_with('/');
        // The tag's value runs to the next '<' (SGML leaf) or is empty.
        let after = &tail[close + 1..];
        let value_len = after.find('<').unwrap_or(after.len());

        // A transaction ends at </STMTTRN>, or implicitly at the next
        // <STMTTRN> or the end of the transaction list.
        if in_trn && ((name == "STMTTRN") || (is_end && name == "BANKTRANLIST")) {
            record_posted(&mut posted, fitid.as_ref(), dtposted.take());
            flush(&mut extensions, fitid.take(), &mut pending);
            in_trn = false;
        }
        if name == "STMTTRN" && !is_end {
            in_trn = true;
        }

        if in_trn && !is_end && name == "FITID" {
            fitid = Some(decode_entities(after[..value_len].trim()));
        }
        if !is_end && name == "DTASOF" {
            as_of.push(after[..value_len].trim().to_owned());
        }
        if in_trn && !is_end && name == "DTPOSTED" {
            dtposted = Some(after[..value_len].trim().to_owned());
        }
        if in_trn && !KNOWN_IN_STMTTRN.contains(&name) && name != "STMTTRN" {
            if !is_end {
                pending.push(Extension {
                    tag: name.to_owned(),
                    value: decode_entities(after[..value_len].trim()),
                });
            }
            // Drop the tag and its value; keep the whitespace layout simple.
            rest = &after[value_len..];
            continue;
        }

        out.push_str(&tail[..=close]);
        rest = after;
    }
    out.push_str(rest);
    if in_trn {
        record_posted(&mut posted, fitid.as_ref(), dtposted);
        flush(&mut extensions, fitid, &mut pending);
    }
    (out, extensions, posted, as_of)
}

fn record_posted(
    map: &mut HashMap<String, Vec<String>>,
    fitid: Option<&String>,
    value: Option<String>,
) {
    if let (Some(id), Some(v)) = (fitid, value) {
        map.entry(id.clone()).or_default().push(v);
    }
}

fn flush(
    map: &mut HashMap<String, Vec<Extension>>,
    fitid: Option<String>,
    pending: &mut Vec<Extension>,
) {
    let fields = std::mem::take(pending);
    if let Some(id) = fitid
        && !fields.is_empty()
    {
        map.entry(id).or_default().extend(fields);
    }
}

#[cfg(test)]
mod tests;
