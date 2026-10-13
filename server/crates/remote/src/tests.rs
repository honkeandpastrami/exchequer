use std::collections::BTreeMap;

use outbound::decision::Answer;
use rust_decimal::Decimal;
use suggest::{CONFIDENT, Example, Facts, Label, Labels, StatementKind};
use time::macros::date;

use super::*;

const ALEX: Uuid = Uuid::from_u128(1);
const HOUSEHOLD: Uuid = Uuid::from_u128(2);
const CARD: Uuid = Uuid::from_u128(10);
const CHECKING: Uuid = Uuid::from_u128(11);
const CAFE: Uuid = Uuid::from_u128(20);
const GROCER: Uuid = Uuid::from_u128(21);
const DINING: Uuid = Uuid::from_u128(30);
const GROCERIES: Uuid = Uuid::from_u128(31);

/// Answers each question with the option `pick` names (by question and
/// options), at 0.95, and keeps what it was asked.
struct Fake {
    pick: fn(&Question) -> Option<String>,
    fail: Option<fn() -> Error>,
    asked: Mutex<Vec<(serde_json::Value, Vec<Question>)>>,
}

impl Fake {
    fn new(pick: fn(&Question) -> Option<String>) -> Self {
        Self {
            pick,
            fail: None,
            asked: Mutex::default(),
        }
    }
}

impl Decide for Fake {
    fn deployment(&self) -> &str {
        "my-deployment"
    }

    fn decide(&self, state: &serde_json::Value, questions: &[Question]) -> Result<Decision, Error> {
        self.asked
            .lock()
            .unwrap()
            .push((state.clone(), questions.to_vec()));
        if let Some(f) = self.fail {
            return Err(f());
        }
        let mut answers = BTreeMap::new();
        for q in questions {
            let Some(choice) = (self.pick)(q) else {
                continue;
            };
            let probabilities = q
                .options
                .iter()
                .map(|(k, _)| (k.clone(), if *k == choice { 950 } else { 1 }))
                .collect();
            answers.insert(
                q.name.clone(),
                Answer {
                    choice,
                    probabilities,
                },
            );
        }
        Ok(Decision {
            model: "decision-model".into(),
            answers,
        })
    }
}

fn catalog() -> Catalog {
    Catalog {
        accounts: vec![
            AccountInfo {
                id: CARD,
                name: "Credit".into(),
                kind: "credit_card".into(),
                owner: ALEX,
                numbers: vec!["4321".into()],
                archived: false,
            },
            AccountInfo {
                id: CHECKING,
                name: "Checking".into(),
                kind: "checking".into(),
                owner: ALEX,
                numbers: vec![],
                archived: false,
            },
        ],
        counterparties: vec![
            (CAFE, "Corner Cafe".into()),
            (GROCER, "Green Grocer".into()),
        ],
        categories: vec![(DINING, "Dining".into()), (GROCERIES, "Groceries".into())],
        parties: vec![
            PartyInfo {
                id: ALEX,
                name: "Alex".into(),
                kind: "person".into(),
            },
            PartyInfo {
                id: HOUSEHOLD,
                name: "Household".into(),
                kind: "household".into(),
            },
        ],
    }
}

fn history() -> History {
    let mut examples = Vec::new();
    for (i, (d, cp, cat, text, cents)) in [
        (
            date!(2026 - 01 - 03),
            CAFE,
            DINING,
            "SQ *CORNER CAFE 1234567890",
            -450,
        ),
        (
            date!(2026 - 02 - 03),
            CAFE,
            DINING,
            "SQ *CORNER CAFE 2234567890",
            -450,
        ),
        (
            date!(2026 - 02 - 10),
            GROCER,
            GROCERIES,
            "GREEN GROCER #12",
            -8000,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        examples.push(Example {
            transaction: Uuid::from_u128(100 + i as u128),
            account: CARD,
            date: d,
            amount: Decimal::new(cents, 2),
            description: text.into(),
            labels: Labels {
                counterparty: Some(cp),
                category: Some(cat),
                purpose: (cat == GROCERIES).then_some(HOUSEHOLD),
            },
        });
    }
    History {
        accounts: vec![],
        examples,
    }
}

/// The local level's evidence: every example, latest first.
struct Evidence(Vec<Uuid>);

impl Suggester for Evidence {
    fn identity(&self) -> &Identity {
        unimplemented!()
    }
    fn labels(&self, rows: &[Row]) -> Vec<Proposal> {
        vec![Proposal::default(); rows.len()]
    }
    fn account(&self, _: &Statement) -> Option<Guess<Uuid>> {
        None
    }
    fn similar(&self, _: &Row, n: usize) -> Vec<Uuid> {
        self.0.iter().take(n).copied().collect()
    }
}

fn level(fake: Arc<Fake>) -> Level {
    let evidence = Evidence(vec![Uuid::from_u128(101), Uuid::from_u128(100)]);
    Remote::new(fake, 2).level(&history(), &catalog(), Some(Arc::new(evidence)))
}

fn row(description: &str, facts: Facts) -> Row {
    Row {
        account: Some(CARD),
        date: date!(2026 - 03 - 03),
        amount: Decimal::new(-450, 2),
        description: description.into(),
        raw: serde_json::json!({ "Type": "Sale", "Card Member": "ALEX", "Reference": "998877665544" }),
        facts,
    }
}

fn pick(q: &Question) -> Option<String> {
    Some(
        match q.name.as_str() {
            "counterparty" => "Corner Cafe",
            "category" => "Dining",
            "purpose" => "(the account's owner)",
            _ => "Alex · Credit",
        }
        .into(),
    )
}

#[test]
fn open_labels_are_asked_and_answers_are_ledger_values() {
    let fake = Arc::new(Fake::new(pick));
    let l = level(fake.clone());
    let p = &l.labels(&[row("SQ *CORNER CAFE 3234567890", Facts::default())])[0];
    let cp = p.counterparty.as_ref().unwrap();
    assert_eq!(
        (cp.value, cp.confidence, cp.reason.as_str()),
        (Some(CAFE), 950, "decision-model: 0.95")
    );
    assert!(cp.confidence >= CONFIDENT);
    assert_eq!(p.category.as_ref().unwrap().value, Some(DINING));
    // "None" is a value: the account's owner.
    assert_eq!(p.purpose.as_ref().unwrap().value, None);
    let i = l.identity();
    assert_eq!(
        (i.kind, i.name.as_str(), i.version.as_str()),
        (Kind::Remote, "decision-model", "q1")
    );
    assert_eq!(i.details["deployment"], "my-deployment");

    let asked = fake.asked.lock().unwrap();
    let (state, questions) = &asked[0];
    let names: Vec<&str> = questions.iter().map(|q| q.name.as_str()).collect();
    assert_eq!(names, ["counterparty", "category", "purpose"]);
    // Counterparty candidates come from the evidence; no answer is forced.
    let keys = |q: &Question| -> Vec<String> {
        let mut k: Vec<String> = q.options.iter().map(|o| o.0.clone()).collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&questions[0]),
        ["(cannot tell)", "(none of these)", "Corner Cafe"]
    );
    assert_eq!(
        keys(&questions[2]),
        [
            "(cannot tell)",
            "(the account's owner)",
            "Alex",
            "Household"
        ]
    );
    let t = &state["transaction"];
    assert_eq!(
        (
            &t["description"],
            &t["amount"],
            &t["direction"],
            &t["weekday"]
        ),
        (
            &serde_json::json!("CORNER CAFE #"),
            &serde_json::json!("4.50"),
            &serde_json::json!("out of the account"),
            &serde_json::json!("Tuesday")
        )
    );
    assert_eq!(t["bank_fields"], serde_json::json!({ "Type": "Sale" }));
    assert_eq!(
        t["recurs"],
        "the same amount, with similar text, in 2 of the last 6 months"
    );
    assert_eq!(state["account"]["name"], "Alex · Credit");
    assert_eq!(
        state["similar_past_transactions"][0]["counterparty"],
        "Corner Cafe"
    );
    // No identifier is ever sent: long digit runs are masked everywhere.
    let all = format!("{asked:?}");
    for digits in ["12345", "22345", "998877", "ALEX"] {
        assert!(!all.contains(digits), "{digits} in {all}");
    }
}

#[test]
fn settled_labels_are_facts_not_questions() {
    let fake = Arc::new(Fake::new(pick));
    let l = level(fake.clone());
    let mut facts = Facts::default();
    facts.set(Label::Counterparty, Some(CAFE));
    facts.set(Label::Purpose, Some(HOUSEHOLD));
    let p = &l.labels(&[row("CORNER CAFE", facts)])[0];
    assert!(p.counterparty.is_none() && p.purpose.is_none());
    assert_eq!(p.category.as_ref().unwrap().value, Some(DINING));
    let asked = fake.asked.lock().unwrap();
    assert_eq!(asked[0].1.len(), 1);
    assert_eq!(
        asked[0].0["already_known"],
        serde_json::json!({ "counterparty": "Corner Cafe", "purpose": "Household" })
    );
    // Nothing open: nothing asked.
    drop(asked);
    facts.set(Label::Category, None);
    l.labels(&[row("CORNER CAFE", facts)]);
    assert_eq!(fake.asked.lock().unwrap().len(), 1);
}

#[test]
fn no_answer_is_no_guess() {
    let l = level(Arc::new(Fake::new(|q| {
        Some(match q.name.as_str() {
            "counterparty" => "(none of these)".into(),
            "category" => "(cannot tell)".into(),
            _ => "Somebody we never offered".into(),
        })
    })));
    assert_eq!(
        l.labels(&[row("CORNER CAFE", Facts::default())]),
        vec![Proposal::default()]
    );
}

#[test]
fn a_refusal_skips_the_row_and_a_failure_stops_asking() {
    let mut fake = Fake::new(pick);
    fake.fail = Some(|| Error::Refused {
        status: 400,
        code: Some("content_filter".into()),
    });
    let fake = Arc::new(fake);
    let rows = vec![row("CORNER CAFE", Facts::default()); 3];
    let out = level(fake.clone()).labels(&rows);
    assert_eq!(out, vec![Proposal::default(); 3]);
    assert_eq!(fake.asked.lock().unwrap().len(), 3);

    let mut fake = Fake::new(pick);
    fake.fail = Some(|| Error::Status {
        status: 401,
        code: None,
    });
    let fake = Arc::new(fake);
    let l = Remote::new(fake.clone(), 1).level(&history(), &catalog(), None);
    assert_eq!(l.labels(&rows), vec![Proposal::default(); 3]);
    assert_eq!(fake.asked.lock().unwrap().len(), 1);
    // Never answered: the identity is the deployment's.
    assert_eq!(l.identity().name, "my-deployment");
}

#[test]
fn options_are_shuffled_the_same_way_for_the_same_row() {
    let fake = Arc::new(Fake::new(pick));
    let l = level(fake.clone());
    let a = row("CORNER CAFE", Facts::default());
    l.labels(&[a.clone(), a]);
    let asked = fake.asked.lock().unwrap();
    assert_eq!(asked[0].1, asked[1].1);
    let mut items: Vec<u32> = (0..20).collect();
    context_shuffle(&mut items, "x");
    assert_ne!(items, (0..20).collect::<Vec<_>>());
}

fn context_shuffle(items: &mut [u32], seed: &str) {
    let c = Catalog {
        categories: items
            .iter()
            .map(|i| (Uuid::from_u128(u128::from(*i) + 1000), i.to_string()))
            .collect(),
        ..Catalog::default()
    };
    let ctx = context::Context::new(&History::default(), &c);
    let (_, asked) = ctx.row(
        &Row {
            account: None,
            date: date!(2026 - 03 - 03),
            amount: Decimal::ONE,
            description: seed.into(),
            raw: serde_json::Value::Null,
            facts: Facts::default(),
        },
        &[],
    );
    let order: Vec<u32> = asked
        .iter()
        .find(|(l, _)| *l == Label::Category)
        .unwrap()
        .1
        .question
        .options
        .iter()
        .filter_map(|o| o.0.parse().ok())
        .collect();
    items.copy_from_slice(&order);
}

#[test]
fn a_statement_is_asked_its_account() {
    let fake = Arc::new(Fake::new(pick));
    let l = level(fake.clone());
    let s = Statement {
        file_name: Some("activity-12345678.csv".into()),
        number: Some("4321".into()),
        kind: Some(StatementKind::CreditCard),
        rows: vec![row("SQ *CORNER CAFE 3234567890", Facts::default())],
    };
    let g = l.account(&s).unwrap();
    assert_eq!((g.value, g.confidence), (CARD, 950));
    let asked = fake.asked.lock().unwrap();
    let (state, questions) = &asked[0];
    // A credit card statement: only the card is offered.
    let keys: Vec<&str> = questions[0].options.iter().map(|o| o.0.as_str()).collect();
    assert!(keys.contains(&"Alex · Credit") && !keys.contains(&"Alex · Checking"));
    let card = &questions[0]
        .options
        .iter()
        .find(|o| o.0 == "Alex · Credit")
        .unwrap()
        .1;
    assert!(card.contains("numbers ending 4321"), "{card}");
    assert_eq!(state["statement"]["file_name"], "activity-#.csv");
    assert_eq!(state["statement"]["account_number_ending"], "4321");
}
