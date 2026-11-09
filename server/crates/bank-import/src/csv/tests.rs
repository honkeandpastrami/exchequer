use super::*;
use time::macros::date;

fn amt(s: &str) -> Option<String> {
    amount(s).map(|a| a.as_decimal().to_string())
}

#[test]
fn dates() {
    assert_eq!(date("01/31/2024"), Some(date!(2024 - 01 - 31)));
    assert_eq!(date("1/5/24"), Some(date!(2024 - 01 - 05)));
    assert_eq!(date("12/31/69"), Some(date!(1969 - 12 - 31)));
    assert_eq!(date("2024-02-29"), Some(date!(2024 - 02 - 29)));
    for bad in [
        "2023-02-29",
        "31/01/2024",
        "2024/01/31",
        "Jan 5",
        "",
        "1/5/2024x",
    ] {
        assert_eq!(date(bad), None, "{bad}");
    }
}

#[test]
fn amounts() {
    assert_eq!(amt("-1,234.50").as_deref(), Some("-1234.50"));
    assert_eq!(amt("$1,234.50").as_deref(), Some("1234.50"));
    assert_eq!(amt("-$5").as_deref(), Some("-5"));
    assert_eq!(amt("(12.30)").as_deref(), Some("-12.30"));
    assert_eq!(amt("+0.1").as_deref(), Some("0.1"));
    for bad in ["1e5", "12.", ".5", "--1", "1.2.3", "abc", "$", "NaN"] {
        assert_eq!(amt(bad), None, "{bad}");
    }
}

#[test]
fn a_statement_with_a_summary_block_and_a_reference_column() {
    let file = "\u{feff}Description,,Summary Amt.\n\
        Beginning balance as of 03/01/2026,,\"1,000.00\"\n\
        \n\
        Posted Date,Reference Number,Payee,Address,Amount\n\
        03/01/2026,,Beginning balance as of 03/01/2026,,\n\
        03/02/2026,R-1,GROCER #12,Main St,-80.00\n\
        03/03/2026,,\"ACME, INC. PAYROLL\",,\"1,500.00\"\n\
        03/03/2026,,CAFÉ ☕,,-4.50\n\
        13/04/2026,,BAD DATE,,-1.00\n\
        2026-03-05,,BAD AMOUNT,,1.2.3\n";
    let s = parse(file.as_bytes()).unwrap();
    let summary: Vec<_> = s
        .rows
        .iter()
        .map(|r| {
            (
                r.date.to_string(),
                r.amount.as_decimal().to_string(),
                r.description.as_str(),
                r.native_id.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                "2026-03-02".into(),
                "-80.00".into(),
                "GROCER #12",
                Some("R-1")
            ),
            (
                "2026-03-03".into(),
                "1500.00".into(),
                "ACME, INC. PAYROLL",
                None
            ),
            ("2026-03-03".into(), "-4.50".into(), "CAFÉ ☕", None),
        ]
    );
    assert_eq!(s.rows[0].raw["Address"], "Main St");
    assert_eq!(
        s.skipped,
        [
            Skipped {
                line: 9,
                reason: "unreadable date".into()
            },
            Skipped {
                line: 10,
                reason: "unreadable amount".into()
            },
        ]
    );
    assert!(s.account.is_none() && s.ledger.is_none());
}

#[test]
fn a_card_statement_signed_by_its_debit_credit_column() {
    let file = "Description,,,,Summary Amt.\n\
        Total credits,,,,\"-50.00\"\n\
        Total debits,,,,\"120.00\"\n\
        \n\
        CardHolder Name,Account/Card Number - last 4 digits,Posting Date,Trans. Date,Reference ID,Description,Amount,MCC,Merchant Category,Transaction Type,Expense Category\n\
        JANE DOE,1234,10/20/2026,10/19/2026,Ref: 111,\"SQ *BAKERY, INC\",\"120.00\",5462,\"BAKERIES\",D,Food\n\
        ACME LLC,5678,10/05/2026,10/03/2026,Ref: 222,\"PAYMENT FROM CHK 9999\",\"-50.00\",0000,\"\",C,\n\
        ACME LLC,5678,10/06/2026,10/06/2026,Ref: 333,\"ADJUSTMENT\",\"-7.00\",0000,\"\",,\n";
    assert!(looks_like(file.as_bytes()));
    let s = parse(file.as_bytes()).unwrap();
    let summary: Vec<_> = s
        .rows
        .iter()
        .map(|r| {
            (
                r.date.to_string(),
                r.amount.as_decimal().to_string(),
                r.description.as_str(),
                r.native_id.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                "2026-10-20".into(),
                "-120.00".into(),
                "SQ *BAKERY, INC",
                Some("Ref: 111")
            ),
            (
                "2026-10-05".into(),
                "50.00".into(),
                "PAYMENT FROM CHK 9999",
                Some("Ref: 222")
            ),
            (
                "2026-10-06".into(),
                "-7.00".into(),
                "ADJUSTMENT",
                Some("Ref: 333")
            ),
        ]
    );
    assert!(s.skipped.is_empty());
}

#[test]
fn needs_a_header() {
    assert!(matches!(
        parse(b"03/02/2026,GROCER,-80.00\n"),
        Err(Error::NoHeader)
    ));
    assert!(matches!(parse(b"date,amount\xff\n"), Err(Error::Encoding)));
    assert!(!looks_like(b"just,some,text\n1,2,3\n"));
    assert!(looks_like(b"Date,Description,Amount\n"));
}
