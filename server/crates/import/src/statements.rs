//! OFX documents as [`Statement`]s: one per bank or credit-card statement in
//! the file.
//!
//! Dates are the server's local dates (design §6.1): an OFX date-time
//! written with a zone (`20260302220000[-5:EST]`) is converted to the
//! server's zone; one written without (`20260302`) is taken as written,
//! being the bank's own local date.

use std::collections::HashMap;

use domain::{AccountNumber, Amount};
use jiff::tz::TimeZone;
use ofx_rs::aggregates::StatementTransaction;
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use time::{Date, Month, OffsetDateTime};

use crate::ofx::{self, Parsed, restore};
use crate::{Format, LedgerBalance, Row, Statement, StatementKind};

/// An OFX date-time as written: `YYYYMMDD[HHMM[SS[.XXX]]][[±H[.H][:NAME]]]`.
struct Written {
    date: Date,
    /// Seconds since the epoch, when the value names its zone.
    instant: Option<i64>,
    /// Seconds since the epoch reading it as UTC, as `ofx-rs` does.
    as_utc: i64,
}

fn written(raw: &str) -> Option<Written> {
    let raw = raw.trim();
    let (main, zone) = match raw.split_once('[') {
        Some((m, z)) => (m, Some(z.trim_end_matches(']'))),
        None => (raw, None),
    };
    let digits = main.split('.').next()?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) || ![8, 12, 14].contains(&digits.len()) {
        return None;
    }
    let n = |a: usize, b: usize| digits.get(a..b).map_or(Some(0), |x| x.parse::<u8>().ok());
    let year: i32 = digits[..4].parse().ok()?;
    let date = Date::from_calendar_date(year, Month::try_from(n(4, 6)?).ok()?, n(6, 8)?).ok()?;
    let time = time::Time::from_hms(n(8, 10)?, n(10, 12)?, n(12, 14)?).ok()?;
    let as_utc = date.with_time(time).assume_utc().unix_timestamp();
    let instant = match zone {
        None => None,
        Some(z) => {
            let hours: Decimal = z.split(':').next()?.trim().parse().ok()?;
            let offset: i64 = (hours * Decimal::from(3600)).round().try_into().ok()?;
            Some(as_utc - offset)
        }
    };
    Some(Written {
        date,
        instant,
        as_utc,
    })
}

/// The server's date for an OFX date-time. `fallback`: what `ofx-rs` read,
/// when the written form isn't at hand.
fn local_date(raw: Option<&str>, fallback: &OffsetDateTime, zone: &TimeZone) -> Date {
    let in_zone = |secs: i64| {
        jiff::Timestamp::from_second(secs).ok().and_then(|t| {
            let d = t.to_zoned(zone.clone()).date();
            Date::from_calendar_date(
                i32::from(d.year()),
                Month::try_from(u8::try_from(d.month()).ok()?).ok()?,
                u8::try_from(d.day()).ok()?,
            )
            .ok()
        })
    };
    match raw.and_then(written) {
        Some(Written {
            instant: Some(secs),
            date,
            ..
        }) => in_zone(secs).unwrap_or(date),
        Some(w) => w.date,
        None => in_zone(fallback.unix_timestamp()).unwrap_or_else(|| fallback.date()),
    }
}

struct Reader<'a> {
    parsed: &'a Parsed,
    zone: &'a TimeZone,
    /// How many transactions with each FITID have been read.
    seen: HashMap<String, usize>,
}

impl Reader<'_> {
    fn rows(&mut self, list: Option<&[StatementTransaction]>) -> Vec<Row> {
        list.unwrap_or_default()
            .iter()
            .map(|t| self.row(t))
            .collect()
    }

    fn ledger(&self, b: Option<&ofx_rs::aggregates::LedgerBalance>) -> Option<LedgerBalance> {
        b.map(|b| {
            let read = b.as_of().as_offset_date_time();
            // The written form whose reading matches what ofx-rs read.
            let raw = self.parsed.as_of.iter().find(|r| {
                written(r).is_some_and(|w| w.instant.unwrap_or(w.as_utc) == read.unix_timestamp())
            });
            LedgerBalance {
                amount: Amount::from_decimal(b.amount().as_decimal()),
                date: local_date(raw.map(String::as_str), read, self.zone),
            }
        })
    }

    fn statement(
        &mut self,
        kind: StatementKind,
        account: &str,
        currency: &str,
        ledger: Option<&ofx_rs::aggregates::LedgerBalance>,
        list: Option<&[StatementTransaction]>,
    ) -> Statement {
        Statement {
            format: Format::Ofx,
            account: AccountNumber::new(&restore(account)),
            kind: Some(kind),
            currency: Some(currency.to_owned()),
            ledger: self.ledger(ledger),
            rows: self.rows(list),
            skipped: Vec::new(),
        }
    }

    /// The description is the payee name, else the memo, else "Transaction",
    /// as in the preimporter.
    fn row(&mut self, t: &StatementTransaction) -> Row {
        let name = t.name().map(restore);
        let memo = t.memo().map(restore);
        let description = [&name, &memo]
            .into_iter()
            .flatten()
            .map(|s| s.trim())
            .find(|s| !s.is_empty())
            .unwrap_or("Transaction")
            .to_owned();
        let fit_id = restore(t.fit_id().as_str());
        let posted = t.date_posted().as_offset_date_time();
        let nth = self.seen.entry(fit_id.clone()).or_insert(0);
        let raw_posted = self
            .parsed
            .posted
            .get(&fit_id)
            .and_then(|v| v.get(*nth))
            .cloned();
        *nth += 1;

        let mut raw = Map::new();
        let mut put = |k: &str, v: String| {
            raw.insert(k.to_owned(), Value::String(v));
        };
        put("TRNTYPE", t.transaction_type().to_string());
        put(
            "DTPOSTED",
            raw_posted.clone().unwrap_or_else(|| posted.to_string()),
        );
        put("TRNAMT", t.amount().as_decimal().to_string());
        put("FITID", fit_id.clone());
        if let Some(n) = name {
            put("NAME", n);
        }
        if let Some(m) = memo {
            put("MEMO", m);
        }
        if let Some(c) = t.check_number() {
            put("CHECKNUM", restore(c.as_str()));
        }
        if let Some(r) = t.reference_number() {
            put("REFNUM", restore(r));
        }
        for e in self.parsed.extensions.get(&fit_id).into_iter().flatten() {
            put(&e.tag, e.value.clone());
        }

        Row {
            date: local_date(raw_posted.as_deref(), posted, self.zone),
            amount: Amount::from_decimal(t.amount().as_decimal()),
            description,
            native_id: Some(fit_id),
            raw,
        }
    }
}

/// Parses an OFX, QFX or QBO file; dates are local to `zone`.
pub fn parse_ofx(bytes: Vec<u8>, zone: &TimeZone) -> Result<Vec<Statement>, ofx::Error> {
    // Banks export a period with no activity as a bare response, sometimes
    // without parts `ofx-rs` requires: that's no statements, not an error.
    let upper = bytes.to_ascii_uppercase();
    let has = |tag: &[u8]| upper.windows(tag.len()).any(|w| w == tag);
    if !has(b"<STMTRS>") && !has(b"<CCSTMTRS>") {
        return Ok(Vec::new());
    }
    let parsed = ofx::parse(bytes)?;
    let doc = &parsed.document;
    let mut r = Reader {
        parsed: &parsed,
        zone,
        seen: HashMap::new(),
    };
    let mut out = Vec::new();
    if let Some(banking) = doc.banking() {
        for w in banking.statement_responses() {
            if let Some(s) = w.response() {
                out.push(r.statement(
                    StatementKind::Bank,
                    s.bank_account().account_id().as_str(),
                    s.currency_default().as_str(),
                    s.ledger_balance(),
                    s.transaction_list().map(|l| l.transactions()),
                ));
            }
        }
    }
    if let Some(cards) = doc.credit_card() {
        for w in cards.statement_responses() {
            if let Some(s) = w.response() {
                out.push(r.statement(
                    StatementKind::CreditCard,
                    s.credit_card_account().account_id().as_str(),
                    s.currency_default().as_str(),
                    s.ledger_balance(),
                    s.transaction_list().map(|l| l.transactions()),
                ));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn zone(name: &str) -> TimeZone {
        TimeZone::get(name).unwrap()
    }

    const SGML: &str = "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\nSECURITY:NONE\r\n\
ENCODING:USASCII\r\nCHARSET:1252\r\nCOMPRESSION:NONE\r\nOLDFILEUID:NONE\r\nNEWFILEUID:NONE\r\n\r\n\
<OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<DTSERVER>20260331120000<LANGUAGE>ENG</SONRS></SIGNONMSGSRSV1>\
<CREDITCARDMSGSRSV1><CCSTMTTRNRS><TRNUID>1<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<CCSTMTRS><CURDEF>USD<CCACCTFROM><ACCTID>4111000011112222</CCACCTFROM>\
<BANKTRANLIST><DTSTART>20260301<DTEND>20260331\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260302220000.000[-5:EST]<TRNAMT>-80.00<FITID>F1\
<NAME>GROCER &amp; CO<MEMO>card 2222<MCC>5411</STMTTRN>\
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260303<TRNAMT>30.00<FITID>F2<MEMO>Refund</STMTTRN>\
</BANKTRANLIST><LEDGERBAL><BALAMT>-50.00<DTASOF>20260331</LEDGERBAL>\
</CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>";

    #[test]
    fn a_response_without_statements_has_none() {
        let empty = "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\n\r\n\
<OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS><LANGUAGE>ENG\
<FI><ORG>Bank<FID>1</FI></SONRS></SIGNONMSGSRSV1><BANKMSGSRSV1></BANKMSGSRSV1></OFX>";
        let ss = parse_ofx(empty.as_bytes().to_vec(), &zone("America/New_York")).unwrap();
        assert!(ss.is_empty());
        assert!(matches!(
            crate::parse(empty.as_bytes().to_vec(), &zone("America/New_York")),
            Err(crate::Error::NoStatements)
        ));
    }

    #[test]
    fn a_credit_card_statement() {
        let ss = parse_ofx(SGML.as_bytes().to_vec(), &zone("America/New_York")).unwrap();
        let [s] = &ss[..] else {
            panic!("one statement")
        };
        assert_eq!(
            (s.kind, s.currency.as_deref()),
            (Some(StatementKind::CreditCard), Some("USD"))
        );
        assert_eq!(s.account.as_ref().map(AccountNumber::last4), Some("2222"));
        assert_eq!(
            s.ledger,
            Some(LedgerBalance {
                amount: "-50.00".parse().unwrap(),
                date: date!(2026 - 03 - 31)
            })
        );
        let r = &s.rows[0];
        assert_eq!(
            (
                r.date,
                r.amount.as_decimal().to_string(),
                r.description.as_str()
            ),
            (date!(2026 - 03 - 02), "-80.00".to_owned(), "GROCER & CO")
        );
        assert_eq!(r.native_id.as_deref(), Some("F1"));
        assert_eq!(
            (&r.raw["MEMO"], &r.raw["MCC"], &r.raw["TRNTYPE"]),
            (
                &Value::from("card 2222"),
                &Value::from("5411"),
                &Value::from("DEBIT")
            )
        );
        // No name: the memo describes it.
        assert_eq!(s.rows[1].description, "Refund");
    }

    #[test]
    fn detected_and_parsed_by_content() {
        assert_eq!(crate::detect(SGML.as_bytes()), Some(Format::Ofx));
        let tokyo = zone("Asia/Tokyo");
        let ss = crate::parse(SGML.as_bytes().to_vec(), &tokyo).unwrap();
        // 22:00 in New York is the next morning in Tokyo; a date written
        // without a zone stays as written.
        assert_eq!(
            ss[0].rows.iter().map(|r| r.date).collect::<Vec<_>>(),
            [date!(2026 - 03 - 03), date!(2026 - 03 - 03)]
        );
        assert!(matches!(
            crate::parse(b"  \n".to_vec(), &tokyo),
            Err(crate::Error::Empty)
        ));
        assert!(matches!(
            crate::parse(b"hello".to_vec(), &tokyo),
            Err(crate::Error::UnknownFormat)
        ));
    }

    #[test]
    fn written_date_times() {
        let ny = zone("America/New_York");
        let at = |raw: &str| local_date(Some(raw), &OffsetDateTime::UNIX_EPOCH, &ny);
        assert_eq!(at("20260302"), date!(2026 - 03 - 02));
        assert_eq!(at("20260302235959.999"), date!(2026 - 03 - 02));
        assert_eq!(at("202603022359"), date!(2026 - 03 - 02));
        assert_eq!(at("20260303030000[0:GMT]"), date!(2026 - 03 - 02));
        assert_eq!(at("20260303030000.000[+0]"), date!(2026 - 03 - 02));
        assert_eq!(at("20260303090000[5.5:IST]"), date!(2026 - 03 - 02));
        assert_eq!(at("20260302220000[-5:EST]"), date!(2026 - 03 - 02));
        for bad in ["2026030", "20261302", "20260302[x:Y]", "2026-03-02"] {
            assert!(written(bad).is_none(), "{bad}");
        }
    }
}
