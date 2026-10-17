use suggest::{CONFIDENT, Candidate, Facts, StatementKind};
use time::macros::date;

use super::*;

const CHECKING: Uuid = Uuid::from_u128(1);
const CARD: Uuid = Uuid::from_u128(2);
const STUDIO_CHECKING: Uuid = Uuid::from_u128(3);
const CAFE: Uuid = Uuid::from_u128(10);
const ACME: Uuid = Uuid::from_u128(11);
const DINING: Uuid = Uuid::from_u128(20);
const SALARY: Uuid = Uuid::from_u128(21);
const HOUSEHOLD: Uuid = Uuid::from_u128(30);

fn names() -> Names {
    let account = |id, name: &str, owner: &str| AccountName {
        id,
        name: name.into(),
        owner: owner.into(),
    };
    Names {
        accounts: vec![
            account(CHECKING, "Checking", "Alex"),
            account(CARD, "Credit", "Alex"),
            account(STUDIO_CHECKING, "Checking", "Studio"),
        ],
        counterparties: vec![(CAFE, "Corner Cafe".into()), (ACME, "Acme".into())],
        categories: vec![(DINING, "Dining".into()), (SALARY, "Salary".into())],
        parties: vec![
            (HOUSEHOLD, "Household".into()),
            (Uuid::from_u128(31), "Studio".into()),
        ],
    }
}

fn row(account: Option<Uuid>, amount: &str, description: &str) -> Row {
    Row {
        account,
        date: date!(2026 - 03 - 02),
        amount: amount.parse().unwrap(),
        description: description.into(),
        raw: serde_json::json!({ "Memo": "via app" }),
        facts: Facts::default(),
        transfer_candidates: Vec::new(),
        duplicates: None,
    }
}

const SCRIPT: &str = r#"
fn classify(row, s) {
    let d = row.description.to_upper();
    if d.contains("CORNER CAFE") {
        s.counterparty("Corner Cafe");
        s.category("Dining");
        if row.account == "Alex · Credit" && row.amount < amount("-20") {
            s.purpose("Household");
        }
    } else if d.starts_with("ACME PAYROLL") && row.amount > 0 && row.weekday <= 5 {
        s.counterparty("Acme");
        s.category("Salary");
        s.purpose(());
    } else if row.raw.Memo == "via app" && row.owner == "Studio" {
        s.category(());
    }
}

fn account(file, s) {
    if file.name != () && file.name.starts_with("card-") {
        s.account("Alex · Credit");
    }
}
"#;

#[test]
fn a_script_sets_labels_with_its_lines() {
    let s = Script::compile(3, SCRIPT, &names()).unwrap();
    let set = s
        .classify(&row(Some(CARD), "-25.00", "Corner Cafe #12"))
        .unwrap();
    assert_eq!(
        (set.counterparty, set.category, set.purpose),
        (
            Some((Value::Id(CAFE), 5)),
            Some((Value::Id(DINING), 6)),
            Some((Value::Id(HOUSEHOLD), 8))
        )
    );
    // Not over 20 on the card: no purpose.
    let set = s
        .classify(&row(Some(CARD), "-4.50", "corner cafe"))
        .unwrap();
    assert_eq!(set.purpose, None);
    // `()` is "none"; integers compare with amounts.
    let set = s
        .classify(&row(Some(CHECKING), "2500", "ACME PAYROLL"))
        .unwrap();
    assert_eq!(set.purpose, Some((Value::None, 13)));
    // Bank fields, and the owner.
    let set = s
        .classify(&row(Some(STUDIO_CHECKING), "-1", "SOMETHING"))
        .unwrap();
    assert_eq!(set.category, Some((Value::None, 15)));
    assert_eq!(
        s.classify(&row(None, "-1", "SOMETHING")).unwrap(),
        Set::default()
    );

    let p = &s.labels(&[row(Some(CARD), "-25.00", "CORNER CAFE")])[0];
    let g = p.counterparty.as_ref().unwrap();
    assert_eq!(
        (g.value, g.confidence, g.reason.as_str()),
        (Value::Id(CAFE), 1000, "rule v3:5")
    );
    assert!(g.confidence >= CONFIDENT);
    assert_eq!(s.identity().kind, Kind::Rule);
}

#[test]
fn a_transfer_is_set_from_its_potential_matches() {
    let script = r#"
fn classify(row, s) {
    for c in row.transfer_candidates {
        if c.account == "Alex · Credit" && c.days <= 2 && row.description.contains("AUTOPAY") {
            s.transfer("Alex · Credit");
        }
    }
}
"#;
    let s = Script::compile(2, script, &names()).unwrap();
    let mut r = row(Some(CHECKING), "-300", "CARD AUTOPAY");
    assert_eq!(s.classify(&r).unwrap().counterparty, None);
    r.transfer_candidates = vec![Candidate {
        account: CARD,
        date: date!(2026 - 03 - 01),
    }];
    assert_eq!(
        s.classify(&r).unwrap().counterparty,
        Some((Value::Account(CARD), 5))
    );
    // Names are checked, and a transfer needs one.
    assert!(
        errors("fn classify(row, s) { s.transfer(\"Alex · Nope\"); }")[0].contains("no account")
    );
    assert!(errors("fn classify(row, s) { s.transfer(()); }")[0].contains("written out"));
}

#[test]
fn a_script_suggests_a_statements_account() {
    let s = Script::compile(1, SCRIPT, &names()).unwrap();
    let statement = |file_name: Option<&str>| Statement {
        file_name: file_name.map(str::to_owned),
        number: None,
        kind: Some(StatementKind::CreditCard),
        rows: vec![row(None, "-1", "X")],
    };
    let g = s.account(&statement(Some("card-march.csv"))).unwrap();
    assert_eq!((g.value, g.reason.as_str()), (CARD, "rule v1:21"));
    assert!(s.account(&statement(Some("bank.csv"))).is_none());
    assert!(s.account(&statement(None)).is_none());
}

#[test]
fn an_empty_script_sets_nothing() {
    let s = Script::compile(1, "", &names()).unwrap();
    assert_eq!(s.labels(&[row(None, "1", "X")]), vec![Proposal::default()]);
    assert!(
        s.account(&Statement {
            file_name: None,
            number: None,
            kind: None,
            rows: vec![]
        })
        .is_none()
    );
}

fn errors(source: &str) -> Vec<String> {
    match Script::compile(1, source, &names()) {
        Ok(_) => Vec::new(),
        Err(e) => e.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn invalid_scripts_are_refused_with_lines() {
    let e = errors("fn classify(row, s) {\n  if row.amount < amount(\"1\") {\n}");
    assert_eq!(e.len(), 1);
    assert!(e[0].starts_with("line 3"), "{e:?}");
    // Unknown names, by line, wherever they are.
    let e = errors(
        "fn classify(row, s) {\n\
         s.category(\"Dinning\");\n\
         if row.account == \"Alex · Savings\" { s.purpose(\"Alex\"); }\n\
         if \"Nobody\" != row.owner { s.counterparty(\"Corner Cafe\"); }\n\
         }",
    );
    assert_eq!(e.len(), 4, "{e:?}");
    assert!(e[0].starts_with("line 2") && e[0].contains("no category is named \"Dinning\""));
    assert!(e[1].starts_with("line 3") && e[1].contains("no account is named"));
    assert!(e[2].starts_with("line 3") && e[2].contains("no party is named \"Alex\""));
    assert!(e[3].starts_with("line 4") && e[3].contains("no party is named \"Nobody\""));
    // Names must be written out; an account name two accounts share must
    // name the owner.
    let e = errors("fn classify(row, s) { let c = \"Dining\"; s.category(c); }");
    assert!(e[0].contains("written out"), "{e:?}");
    let e = errors("fn account(file, s) { s.account(\"Checking\"); }");
    assert!(e[0].contains("Owner · Checking"), "{e:?}");
    assert!(errors("fn account(file, s) { s.account(\"Credit\"); }").is_empty());
    // Undefined variables, entry points' parameters, floats.
    assert!(errors("fn classify(row, s) { s.category(nope); }")[0].contains("nope"));
    assert!(errors("fn classify(row) {}")[0].contains("fn classify(row, s)"));
    assert!(!errors("fn classify(row, s) { let x = 1.5; }").is_empty());
    assert!(!errors("fn classify(row, s) { eval(\"1\"); }").is_empty());
}

#[test]
fn a_runaway_script_is_stopped() {
    let s = Script::compile(1, "fn classify(row, s) { loop { } }", &names()).unwrap();
    let e = s.classify(&row(None, "1", "X")).unwrap_err();
    assert!(e.to_lowercase().contains("operations"), "{e}");
    assert_eq!(s.labels(&[row(None, "1", "X")]), vec![Proposal::default()]);
}

#[test]
fn a_name_gone_since_is_an_error_when_run() {
    let s = Script::compile(
        1,
        "fn classify(row, s) { s.category(\"Dining\"); }",
        &names(),
    )
    .unwrap();
    let mut fewer = names();
    fewer.categories.clear();
    assert!(Script::compile(1, "fn classify(row, s) { s.category(\"Dining\"); }", &fewer).is_err());
    assert!(s.classify(&row(None, "1", "X")).is_ok());
}
