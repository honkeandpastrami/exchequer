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
