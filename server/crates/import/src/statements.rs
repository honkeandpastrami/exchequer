//! OFX documents as [`Statement`]s: one per bank or credit-card statement in
//! the file.

use domain::{AccountNumber, Amount};
use ofx_rs::aggregates::StatementTransaction;
use serde_json::{Map, Value};

use crate::ofx::{self, Extension, restore};
use crate::{Format, LedgerBalance, Row, Statement, StatementKind};

/// Parses an OFX, QFX or QBO file.
pub fn parse_ofx(bytes: Vec<u8>) -> Result<Vec<Statement>, ofx::Error> {
    let parsed = ofx::parse(bytes)?;
    let doc = &parsed.document;
    let rows = |list: Option<&[StatementTransaction]>| -> Vec<Row> {
        list.unwrap_or_default()
            .iter()
            .map(|t| row(t, &parsed.extensions))
            .collect()
    };
    let ledger = |b: Option<&ofx_rs::aggregates::LedgerBalance>| {
        b.map(|b| LedgerBalance {
            amount: Amount::from_decimal(b.amount().as_decimal()),
            date: b.as_of().as_offset_date_time().date(),
        })
    };
    let mut out = Vec::new();
    if let Some(banking) = doc.banking() {
        for wrapper in banking.statement_responses() {
            if let Some(r) = wrapper.response() {
                out.push(Statement {
                    format: Format::Ofx,
                    account: AccountNumber::new(&restore(r.bank_account().account_id().as_str())),
                    kind: Some(StatementKind::Bank),
                    currency: Some(r.currency_default().as_str().to_owned()),
                    ledger: ledger(r.ledger_balance()),
                    rows: rows(r.transaction_list().map(|l| l.transactions())),
                    skipped: Vec::new(),
                });
            }
        }
    }
    if let Some(cards) = doc.credit_card() {
        for wrapper in cards.statement_responses() {
            if let Some(r) = wrapper.response() {
                out.push(Statement {
                    format: Format::Ofx,
                    account: AccountNumber::new(&restore(
                        r.credit_card_account().account_id().as_str(),
                    )),
                    kind: Some(StatementKind::CreditCard),
                    currency: Some(r.currency_default().as_str().to_owned()),
                    ledger: ledger(r.ledger_balance()),
                    rows: rows(r.transaction_list().map(|l| l.transactions())),
                    skipped: Vec::new(),
                });
            }
        }
    }
    Ok(out)
}

/// The description is the payee name, else the memo, else "Transaction",
/// as in the preimporter.
fn row(
    t: &StatementTransaction,
    extensions: &std::collections::HashMap<String, Vec<Extension>>,
) -> Row {
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

    let mut raw = Map::new();
    let mut put = |k: &str, v: String| {
        raw.insert(k.to_owned(), Value::String(v));
    };
    put("TRNTYPE", t.transaction_type().to_string());
    put("DTPOSTED", posted.to_string());
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
    for e in extensions.get(&fit_id).into_iter().flatten() {
        put(&e.tag, e.value.clone());
    }

    Row {
        date: posted.date(),
        amount: Amount::from_decimal(t.amount().as_decimal()),
        description,
        native_id: Some(fit_id),
        raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

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
    fn a_credit_card_statement() {
        let ss = parse_ofx(SGML.as_bytes().to_vec()).unwrap();
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
        let ss = crate::parse(SGML.as_bytes().to_vec()).unwrap();
        assert_eq!(ss[0].rows.len(), 2);
        assert!(matches!(
            crate::parse(b"  \n".to_vec()),
            Err(crate::Error::Empty)
        ));
        assert!(matches!(
            crate::parse(b"hello".to_vec()),
            Err(crate::Error::UnknownFormat)
        ));
    }
}
