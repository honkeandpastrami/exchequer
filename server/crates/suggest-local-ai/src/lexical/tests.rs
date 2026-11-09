use suggest::{CONFIDENT, Example};
use time::macros::date;

use super::*;

const HOUSEHOLD: Uuid = Uuid::from_u128(100);
const CHECKING: Uuid = Uuid::from_u128(1);
const CARD: Uuid = Uuid::from_u128(2);
const CAFE: Uuid = Uuid::from_u128(10);
const GROCER: Uuid = Uuid::from_u128(11);
const EMPLOYER: Uuid = Uuid::from_u128(12);
const DINING: Uuid = Uuid::from_u128(20);
const GROCERIES: Uuid = Uuid::from_u128(21);
const SALARY: Uuid = Uuid::from_u128(22);
const ALEX: Uuid = Uuid::from_u128(30);

fn labels(cp: Uuid, cat: Uuid, purpose: Option<Uuid>) -> Labels {
    Labels {
        counterparty: Value::Id(cp),
        category: Value::Id(cat),
        purpose: purpose.into(),
    }
}

/// A transfer's side: the other account, nothing else.
fn transfer(to: Uuid) -> Labels {
    Labels {
        counterparty: Value::Account(to),
        ..Labels::default()
    }
}

/// Two years of a fictional household: coffee and groceries on the
/// card, pay into checking, the card paid from checking each month.
fn history() -> History {
    let mut examples = Vec::new();
    let mut day = date!(2024 - 01 - 01);
    let mut n = 0u128;
    while day < date!(2026 - 01 - 01) {
        n += 1;
        let mut add = |account, amount: i64, description: String, l| {
            n += 1;
            examples.push(Example {
                transaction: Uuid::from_u128(1000 + n),
                account,
                date: day,
                amount: Decimal::new(amount, 2),
                description,
                labels: l,
            });
        };
        add(
            CARD,
            -450 - i64::from(day.day()),
            format!(
                "SQ *CORNER CAFE #04{} {:02}/{:02}",
                day.day() % 7,
                day.month() as u8,
                day.day()
            ),
            labels(CAFE, DINING, Some(ALEX)),
        );
        if day.day() % 3 == 0 {
            add(
                CARD,
                -8000,
                format!("GREEN GROCER {}", day.to_julian_day()),
                labels(GROCER, GROCERIES, None),
            );
        }
        if day.day() == 15 {
            add(
                CHECKING,
                250_000,
                "ACME PAYROLL DIRECT DEP".to_owned(),
                labels(EMPLOYER, SALARY, None),
            );
        }
        if day.day() == 8 {
            let paid = -40_000 - i64::from(u8::from(day.month())) * 1000;
            add(
                CHECKING,
                paid,
                format!("CARD AUTOPAY {}", day.to_julian_day()),
                transfer(CARD),
            );
            add(
                CARD,
                -paid,
                format!("CARD AUTOPAY {}", day.to_julian_day()),
                transfer(CHECKING),
            );
        }
        day = day.next_day().unwrap();
    }
    History {
        accounts: vec![
            Account {
                id: CHECKING,
                kind: "checking".into(),
                owner: HOUSEHOLD,
                liquid: true,
                archived: false,
            },
            Account {
                id: CARD,
                kind: "credit_card".into(),
                owner: HOUSEHOLD,
                liquid: true,
                archived: false,
            },
        ],
        examples,
    }
}

fn row(account: Option<Uuid>, amount: i64, description: &str) -> Row {
    Row {
        account,
        date: date!(2026 - 01 - 10),
        amount: Decimal::new(amount, 2),
        description: description.into(),
        raw: serde_json::Value::Null,
        facts: Facts::default(),
        transfer_candidates: Vec::new(),
        duplicates: None,
    }
}

#[test]
fn a_familiar_row_is_labeled_confidently() {
    let m = LexicalKnn.train(&history());
    assert_eq!(m.identity().name, "lexical-knn");
    let p = &m.labels(&[row(Some(CARD), -463, "SQ *CORNER CAFE #0412 01/10")])[0];
    let cp = p.counterparty.as_ref().unwrap();
    assert_eq!(cp.value, Value::Id(CAFE));
    assert!(cp.confidence >= CONFIDENT, "{cp:?}");
    assert_eq!(p.purpose.as_ref().unwrap().value, Value::Id(ALEX));
    // "None" is a value like any other: groceries are the owner's.
    let p = &m.labels(&[row(Some(CARD), -8000, "GREEN GROCER 999999")])[0];
    let purpose = p.purpose.as_ref().unwrap();
    assert_eq!(
        (purpose.value, purpose.confidence >= CONFIDENT),
        (Value::None, true)
    );
}

#[test]
fn a_duplicate_doesnt_echo_the_transaction_it_duplicates() {
    // The ledger has the row already, unlabeled: the row may replace it.
    let mut h = history();
    let old = Uuid::from_u128(9999);
    h.examples.push(Example {
        transaction: old,
        account: CARD,
        date: date!(2026 - 01 - 10),
        amount: Decimal::new(-463, 2),
        description: "SQ *CORNER CAFE #0412 01/10".into(),
        labels: Labels::default(),
    });
    let m = LexicalKnn.train(&h);
    let mut r = row(Some(CARD), -463, "SQ *CORNER CAFE #0412 01/10");
    let echoed = m.labels(std::slice::from_ref(&r))[0].counterparty.clone();
    assert_eq!(
        echoed.map(|g| g.value),
        Some(Value::None),
        "the old one wins"
    );
    r.duplicates = Some(old);
    let p = &m.labels(std::slice::from_ref(&r))[0];
    let cp = p.counterparty.as_ref().unwrap();
    assert_eq!(cp.value, Value::Id(CAFE));
    assert!(cp.confidence >= CONFIDENT, "{cp:?}");
    assert!(!m.similar(&r, 5).contains(&old));
}

#[test]
fn similar_transactions_are_evidence() {
    let h = history();
    let m = LexicalKnn.train(&h);
    let near = m.similar(&row(Some(CARD), -463, "SQ *CORNER CAFE #0412"), 3);
    assert_eq!(near.len(), 3);
    for t in near {
        let e = h.examples.iter().find(|e| e.transaction == t).unwrap();
        assert_eq!(e.labels.counterparty, Value::Id(CAFE));
    }
    assert!(m.similar(&row(None, -1, "UNHEARD OF"), 3).is_empty());
}

#[test]
fn a_card_payment_is_a_transfer_to_the_card() {
    let m = LexicalKnn.train(&history());
    let mut r = row(Some(CHECKING), -45_000, "CARD AUTOPAY 2461050");
    let p = &m.labels(std::slice::from_ref(&r))[0];
    let cp = p.counterparty.as_ref().unwrap();
    assert_eq!(cp.value, Value::Account(CARD));
    // The card's side, staged too, is a potential transfer match: more
    // sure still.
    r.transfer_candidates = vec![suggest::Candidate {
        account: CARD,
        date: r.date,
    }];
    let boosted = m.labels(std::slice::from_ref(&r))[0]
        .counterparty
        .clone()
        .unwrap();
    assert_eq!(boosted.value, Value::Account(CARD));
    assert!(boosted.confidence >= cp.confidence, "{boosted:?} {cp:?}");
}

#[test]
fn a_stranger_gets_nothing_confident() {
    let m = LexicalKnn.train(&history());
    let p = &m.labels(&[row(Some(CARD), -1999, "UNHEARD OF HARDWARE")])[0];
    for l in Label::ALL {
        assert!(
            p.get(l).is_none_or(|g| g.confidence < CONFIDENT),
            "{l:?}: {p:?}"
        );
    }
}

#[test]
fn facts_are_left_out_and_steer_the_rest() {
    let m = LexicalKnn.train(&history());
    let mut r = row(Some(CARD), -463, "SQ *CORNER CAFE");
    r.facts.set(Label::Counterparty, Value::Id(GROCER));
    let p = &m.labels(&[r])[0];
    assert!(p.counterparty.is_none());
    assert_eq!(p.category.as_ref().unwrap().value, Value::Id(GROCERIES));
}

#[test]
fn a_statement_is_matched_to_its_account() {
    let m = LexicalKnn.train(&history());
    let s = Statement {
        file_name: None,
        number: None,
        kind: None,
        rows: vec![
            row(None, -455, "SQ *CORNER CAFE #0401"),
            row(None, -8000, "GREEN GROCER 123456"),
            row(None, -470, "SQ *CORNER CAFE #0402"),
        ],
    };
    let g = m.account(&s).unwrap();
    assert_eq!(g.value, CARD);
    assert!(g.confidence >= CONFIDENT, "{g:?}");
    // A bank statement can't be the card's.
    let g = m.account(&Statement {
        kind: Some(StatementKind::Bank),
        ..s
    });
    assert!(g.is_none_or(|g| g.value == CHECKING && g.confidence < CONFIDENT));
}

#[test]
fn nothing_learned_suggests_nothing() {
    let m = LexicalKnn.train(&History::default());
    assert_eq!(
        m.labels(&[row(None, -100, "CAFE")]),
        vec![Proposal::default()]
    );
    assert!(
        m.account(&Statement {
            file_name: None,
            number: None,
            kind: None,
            rows: vec![row(None, -100, "CAFE")]
        })
        .is_none()
    );
}

#[test]
fn the_snapshot_names_the_data() {
    let h = history();
    let a = LexicalKnn.train(&h);
    let b = LexicalKnn.train(&h.before(date!(2025 - 06 - 01)));
    assert_ne!(a.identity(), b.identity());
    assert_eq!(a.identity(), LexicalKnn.train(&h).identity());
}

#[test]
fn a_backtest_measures_the_method() {
    let r = suggest::backtest::run(
        &LexicalKnn,
        &history(),
        date!(2025 - 12 - 01),
        date!(2026 - 01 - 01),
    );
    let cp = &r.labels[&Label::Counterparty];
    assert!(cp.asked > 30 && cp.correct == cp.asked, "{cp:?}");
    assert_eq!(r.accounts.asked, 2);
    assert_eq!(r.accounts.correct, 2);
}
