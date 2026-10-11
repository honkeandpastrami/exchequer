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
    let (stripped, extensions) = lift_extensions(&text);
    let document = ofx_rs::parse(&escape(&stripped)).map_err(Error::Ofx)?;
    Ok(Parsed {
        document,
        extensions,
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
fn lift_extensions(text: &str) -> (String, HashMap<String, Vec<Extension>>) {
    let mut out = String::with_capacity(text.len());
    let mut extensions: HashMap<String, Vec<Extension>> = HashMap::new();
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
            flush(&mut extensions, fitid.take(), &mut pending);
            in_trn = false;
        }
        if name == "STMTTRN" && !is_end {
            in_trn = true;
        }

        if in_trn && !is_end && name == "FITID" {
            fitid = Some(decode_entities(after[..value_len].trim()));
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
        flush(&mut extensions, fitid, &mut pending);
    }
    (out, extensions)
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
mod tests {
    use super::*;

    /// A synthetic OFX 1.02 credit-card statement for the fictional
    /// household: one transaction with bank-specific tags before FITID, one
    /// with escaped ampersands in NAME and FITID.
    const SGML: &str = "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\nSECURITY:NONE\r\n\
ENCODING:USASCII\r\nCHARSET:1252\r\nCOMPRESSION:NONE\r\nOLDFILEUID:NONE\r\nNEWFILEUID:NONE\r\n\r\n\
<OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<DTSERVER>20260930120000<LANGUAGE>ENG</SONRS></SIGNONMSGSRSV1>\
<CREDITCARDMSGSRSV1><CCSTMTTRNRS><TRNUID>1<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<CCSTMTRS><CURDEF>USD<CCACCTFROM><ACCTID>4000123412341234</CCACCTFROM>\
<BANKTRANLIST><DTSTART>20260901<DTEND>20260930\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260905120000\n<TRNAMT>-42.50\n\
<MERCHCAT>BAKERIES\n<MCC>5462\n<CARDNAME>ALEX EXAMPLE\n<FITID>ID-0001\n<NAME>CORNER BAKERY\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260906120000\n<TRNAMT>-19.99\n\
<FITID>ID-0002-A&amp;amp;B\n<NAME>SALT &amp; PEPPER DINER\n<MEMO>A &lt;B&gt; &#38; C ^ D\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260907120000\n<TRNAMT>0.00\n\
<FITID>ID-0003\n<NAME>CRÈME BRÛLÉE CAFÉ\n</STMTTRN>\n\
</BANKTRANLIST><LEDGERBAL><BALAMT>-62.49<DTASOF>20260930</LEDGERBAL>\
</CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>";

    fn transactions(p: &Parsed) -> &[ofx_rs::aggregates::StatementTransaction] {
        let cc = p.document.credit_card().expect("credit card section");
        let resp = cc.statement_responses()[0].response().expect("statement");
        resp.transaction_list()
            .expect("transactions")
            .transactions()
    }

    #[test]
    fn unknown_tags_are_lifted_not_fatal() {
        let p = parse(SGML.as_bytes().to_vec()).expect("parses");
        let t = transactions(&p);
        assert_eq!(t.len(), 3);
        assert_eq!(restore(t[0].fit_id().as_str()), "ID-0001");
        assert_eq!(restore(t[0].name().expect("name")), "CORNER BAKERY");
        let ext = &p.extensions["ID-0001"];
        let tags: Vec<&str> = ext.iter().map(|e| e.tag.as_str()).collect();
        assert_eq!(tags, ["MERCHCAT", "MCC", "CARDNAME"]);
        assert_eq!(ext[1].value, "5462");
    }

    #[test]
    fn entities_are_decoded_once_and_spacing_kept() {
        let p = parse(SGML.as_bytes().to_vec()).expect("parses");
        let t = &transactions(&p)[1];
        assert_eq!(restore(t.name().expect("name")), "SALT & PEPPER DINER");
        assert_eq!(restore(t.memo().expect("memo")), "A <B> & C ^ D");
        // Double-escaped by the bank: decoded once, like other OFX libraries.
        assert_eq!(restore(t.fit_id().as_str()), "ID-0002-A&amp;B");
        assert!(!p.extensions.contains_key("ID-0002-A&amp;B"));
    }

    #[test]
    fn amounts_stay_exact() {
        let p = parse(SGML.as_bytes().to_vec()).expect("parses");
        let t = transactions(&p);
        assert_eq!(t[1].amount().as_decimal().to_string(), "-19.99");
    }

    #[test]
    fn non_ascii_text_survives() {
        let p = parse(SGML.as_bytes().to_vec()).expect("parses");
        assert_eq!(
            restore(transactions(&p)[2].name().expect("name")),
            "CRÈME BRÛLÉE CAFÉ"
        );
    }

    #[test]
    fn escape_round_trips() {
        let s = "a&b^c É ñ 😀 &amp; ^41;";
        assert_eq!(unescape(&escape(s)), s);
        assert!(escape(s).is_ascii());
    }

    #[test]
    fn decode_entities_leaves_unknown_ones() {
        assert_eq!(decode_entities("a &nbsp; b &amp c"), "a &nbsp; b &amp c");
        assert_eq!(decode_entities("&#x41;&#66;"), "AB");
    }
}
