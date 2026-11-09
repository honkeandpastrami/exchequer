use rust_decimal::Decimal;
use time::macros::date;

use super::*;
use crate::{Identity, Kind, Proposal};

/// Proposes `value` for every open label, at `confidence`, and records
/// what facts it was given.
struct Fixed {
    identity: Identity,
    value: Uuid,
    confidence: u16,
    seen: std::sync::Mutex<Vec<Facts>>,
}

impl Fixed {
    fn new(value: u128, confidence: u16) -> Self {
        Self {
            identity: Identity {
                kind: Kind::Local,
                name: "fixed".into(),
                version: "1".into(),
                details: serde_json::json!({}),
            },
            value: Uuid::from_u128(value),
            confidence,
            seen: std::sync::Mutex::default(),
        }
    }
}

impl Suggester for Fixed {
    fn identity(&self) -> &Identity {
        &self.identity
    }
    fn labels(&self, rows: &[Row]) -> Vec<Proposal> {
        rows.iter()
            .map(|r| {
                self.seen.lock().unwrap().push(r.facts);
                let mut p = Proposal::default();
                for l in Label::ALL {
                    if r.facts.get(l).is_none() {
                        p.set(
                            l,
                            Some(Guess {
                                value: Value::Id(self.value),
                                confidence: self.confidence,
                                reason: "fixed".into(),
                            }),
                        );
                    }
                }
                p
            })
            .collect()
    }
    fn account(&self, _: &Statement) -> Option<Guess<Uuid>> {
        Some(Guess {
            value: self.value,
            confidence: self.confidence,
            reason: "fixed".into(),
        })
    }
}

fn row(facts: Facts) -> Row {
    Row {
        account: None,
        date: date!(2026 - 03 - 01),
        amount: Decimal::ONE,
        description: "COFFEE".into(),
        raw: serde_json::Value::Null,
        facts,
        transfer_candidates: Vec::new(),
        duplicates: None,
    }
}

#[test]
fn later_levels_get_settled_labels_as_facts() {
    let mut chosen = Facts::default();
    chosen.set(Label::Purpose, Value::None);
    // The first level is sure only of its counterparty: a second answers
    // the category, knowing the counterparty and your purpose.
    struct OnlyCounterparty(Fixed);
    impl Suggester for OnlyCounterparty {
        fn identity(&self) -> &Identity {
            self.0.identity()
        }
        fn labels(&self, rows: &[Row]) -> Vec<Proposal> {
            self.0
                .labels(rows)
                .into_iter()
                .map(|mut p| {
                    p.category = None;
                    p
                })
                .collect()
        }
        fn account(&self, s: &Statement) -> Option<Guess<Uuid>> {
            self.0.account(s)
        }
    }
    let first = OnlyCounterparty(Fixed::new(1, 950));
    let second = Fixed::new(2, 700);
    let third = Fixed::new(3, 800);
    let picks = labels(&[&first, &second, &third], &[row(chosen)], &[]);
    let p = &picks[0];
    assert_eq!(
        p.counterparty.as_ref().map(|p| (p.level, p.guess.value)),
        Some((0, Value::Id(Uuid::from_u128(1))))
    );
    // The unsure category: the most confident of the later levels.
    assert_eq!(
        p.category.as_ref().map(|p| (p.level, p.guess.value)),
        Some((2, Value::Id(Uuid::from_u128(3))))
    );
    assert!(p.purpose.is_none());
    let seen = second.seen.lock().unwrap();
    assert_eq!(seen[0].counterparty, Some(Value::Id(Uuid::from_u128(1))));
    assert_eq!(seen[0].purpose, Some(Value::None));
    assert_eq!(seen[0].category, None);
}

#[test]
fn a_settled_row_is_not_asked() {
    let first = Fixed::new(1, 1000);
    let second = Fixed::new(2, 1000);
    let picks = labels(&[&first, &second], &[row(Facts::default())], &[]);
    assert!(second.seen.lock().unwrap().is_empty());
    assert_eq!(picks[0].category.as_ref().unwrap().level, 0);
}

#[test]
fn a_transfer_has_no_category_or_purpose() {
    let account = |id: u128, liquid: bool| Account {
        id: Uuid::from_u128(id),
        kind: "checking".into(),
        owner: Uuid::from_u128(99),
        liquid,
        archived: false,
    };
    let accounts = [account(50, true), account(51, true), account(52, false)];
    // The first level is sure the other side is account 51: the second
    // isn't asked about category or purpose, and the first's go.
    struct Transfer(Fixed, Uuid);
    impl Suggester for Transfer {
        fn identity(&self) -> &Identity {
            self.0.identity()
        }
        fn labels(&self, rows: &[Row]) -> Vec<Proposal> {
            self.0
                .labels(rows)
                .into_iter()
                .map(|mut p| {
                    if let Some(c) = &mut p.counterparty {
                        c.value = Value::Account(self.1);
                    }
                    p
                })
                .collect()
        }
        fn account(&self, s: &Statement) -> Option<Guess<Uuid>> {
            self.0.account(s)
        }
    }
    let first = Transfer(Fixed::new(1, 950), Uuid::from_u128(51));
    let second = Fixed::new(2, 950);
    let mut r = row(Facts::default());
    r.account = Some(Uuid::from_u128(50));
    r.amount = -Decimal::ONE;
    let p = &labels(&[&first, &second], &[r.clone()], &accounts)[0];
    assert_eq!(
        p.counterparty.as_ref().map(|p| p.guess.value),
        Some(Value::Account(Uuid::from_u128(51)))
    );
    assert!(p.category.is_none() && p.purpose.is_none(), "{p:?}");
    assert!(second.seen.lock().unwrap().is_empty());
    // Into a liquid account from an investment one: income, which keeps
    // them.
    let first = Transfer(Fixed::new(1, 950), Uuid::from_u128(52));
    r.amount = Decimal::ONE;
    let p = &labels(&[&first], &[r.clone()], &accounts)[0];
    assert!(p.category.is_some() && p.purpose.is_some(), "{p:?}");
    // Out of a liquid account into an investment one: an expense, too.
    r.amount = -Decimal::ONE;
    let p = &labels(&[&first], &[r], &accounts)[0];
    assert!(p.category.is_some() && p.purpose.is_some(), "{p:?}");
}

#[test]
fn an_account_from_the_first_confident_level() {
    let s = Statement {
        file_name: None,
        number: None,
        kind: None,
        rows: vec![row(Facts::default())],
    };
    let (a, b, c) = (Fixed::new(1, 600), Fixed::new(2, 920), Fixed::new(3, 990));
    let p = account(&[&a, &b, &c], &s).unwrap();
    assert_eq!((p.level, p.guess.value), (1, Uuid::from_u128(2)));
    let p = account(&[&a], &s).unwrap();
    assert_eq!(p.guess.confidence, 600);
    assert!(account(&[], &s).is_none());
}
