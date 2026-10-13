//! The levels together (design §8): tried in order (rules, local AI,
//! remote AI). A level runs while anything is still below [`CONFIDENT`];
//! labels already settled, by you or confidently by an earlier level, are
//! given to the next level as facts. A label nobody is confident about keeps
//! the most confident guess any level made.

use uuid::Uuid;

use crate::{CONFIDENT, Facts, Guess, Label, Row, Statement, Suggester, Value};

/// A guess and the level (its index in the stack) that made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick<T> {
    pub level: usize,
    pub guess: Guess<T>,
}

/// A row's picks, one per label at most.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picks {
    pub counterparty: Option<Pick<Value>>,
    pub category: Option<Pick<Value>>,
    pub purpose: Option<Pick<Value>>,
}

impl Picks {
    #[must_use]
    pub fn get(&self, label: Label) -> Option<&Pick<Value>> {
        match label {
            Label::Counterparty => self.counterparty.as_ref(),
            Label::Category => self.category.as_ref(),
            Label::Purpose => self.purpose.as_ref(),
        }
    }

    fn slot(&mut self, label: Label) -> &mut Option<Pick<Value>> {
        match label {
            Label::Counterparty => &mut self.counterparty,
            Label::Category => &mut self.category,
            Label::Purpose => &mut self.purpose,
        }
    }
}

/// Labels for each row, in order. A row's own facts are never suggested.
#[must_use]
pub fn labels(levels: &[&dyn Suggester], rows: &[Row]) -> Vec<Picks> {
    let mut out: Vec<Picks> = vec![Picks::default(); rows.len()];
    for (level, s) in levels.iter().enumerate() {
        // What's settled so far, per row; rows with everything settled sit
        // this level out.
        let mut asked = Vec::new();
        let mut which = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let mut facts = r.facts;
            for l in Label::ALL {
                if let Some(p) = out[i].get(l)
                    && facts.get(l).is_none()
                    && p.guess.confidence >= CONFIDENT
                {
                    facts.set(l, p.guess.value);
                }
            }
            if !facts.all_settled() {
                asked.push(Row { facts, ..r.clone() });
                which.push(i);
            }
        }
        if asked.is_empty() {
            break;
        }
        let proposals = s.labels(&asked);
        for ((i, row), p) in which.into_iter().zip(&asked).zip(proposals) {
            for l in Label::ALL {
                if row.facts.get(l).is_some() {
                    continue;
                }
                let Some(g) = p.get(l) else { continue };
                let slot = out[i].slot(l);
                if slot
                    .as_ref()
                    .is_none_or(|old| g.confidence > old.guess.confidence)
                {
                    *slot = Some(Pick {
                        level,
                        guess: g.clone(),
                    });
                }
            }
        }
    }
    out
}

/// A statement's account.
#[must_use]
pub fn account(levels: &[&dyn Suggester], statement: &Statement) -> Option<Pick<Uuid>> {
    let mut best: Option<Pick<Uuid>> = None;
    for (level, s) in levels.iter().enumerate() {
        if best
            .as_ref()
            .is_some_and(|b| b.guess.confidence >= CONFIDENT)
        {
            break;
        }
        if let Some(g) = s.account(statement)
            && best
                .as_ref()
                .is_none_or(|b| g.confidence > b.guess.confidence)
        {
            best = Some(Pick { level, guess: g });
        }
    }
    best
}

/// Facts from the labels you chose: `chosen` names them, `values` holds
/// what each is set to.
#[must_use]
pub fn facts(chosen: &[Label], values: &crate::Labels) -> Facts {
    let mut f = Facts::default();
    for l in chosen {
        f.set(*l, values.get(*l));
    }
    f
}

#[cfg(test)]
mod tests {
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
                                    value: Some(self.value),
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
        }
    }

    #[test]
    fn later_levels_get_settled_labels_as_facts() {
        let mut chosen = Facts::default();
        chosen.set(Label::Purpose, None);
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
        let picks = labels(&[&first, &second, &third], &[row(chosen)]);
        let p = &picks[0];
        assert_eq!(
            p.counterparty.as_ref().map(|p| (p.level, p.guess.value)),
            Some((0, Some(Uuid::from_u128(1))))
        );
        // The unsure category: the most confident of the later levels.
        assert_eq!(
            p.category.as_ref().map(|p| (p.level, p.guess.value)),
            Some((2, Some(Uuid::from_u128(3))))
        );
        assert!(p.purpose.is_none());
        let seen = second.seen.lock().unwrap();
        assert_eq!(seen[0].counterparty, Some(Some(Uuid::from_u128(1))));
        assert_eq!(seen[0].purpose, Some(None));
        assert_eq!(seen[0].category, None);
    }

    #[test]
    fn a_settled_row_is_not_asked() {
        let first = Fixed::new(1, 1000);
        let second = Fixed::new(2, 1000);
        let picks = labels(&[&first, &second], &[row(Facts::default())]);
        assert!(second.seen.lock().unwrap().is_empty());
        assert_eq!(picks[0].category.as_ref().unwrap().level, 0);
    }

    #[test]
    fn an_account_from_the_first_confident_level() {
        let s = Statement {
            file_name: None,
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
}
