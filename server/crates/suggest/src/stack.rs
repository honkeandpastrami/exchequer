//! The levels together (design §8): tried in order (rules, local AI,
//! remote AI). A level runs while anything is still below [`CONFIDENT`];
//! labels already settled, by you or confidently by an earlier level, are
//! given to the next level as facts. A label nobody is confident about keeps
//! the most confident guess any level made. Once a row's other side is
//! settled as one of your accounts, a transfer, its category and purpose
//! don't apply (design §3.2): they're settled as none.

use uuid::Uuid;

use crate::{Account, CONFIDENT, Facts, Guess, Label, Row, Statement, Suggester, Value, labeled};

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

/// What's settled for a row: its facts, the confident picks so far, and a
/// transfer's category and purpose as none.
fn settled(r: &Row, picks: &Picks, accounts: &[Account]) -> Facts {
    let mut facts = r.facts;
    for l in Label::ALL {
        if let Some(p) = picks.get(l)
            && facts.get(l).is_none()
            && p.guess.confidence >= CONFIDENT
        {
            facts.set(l, p.guess.value);
        }
    }
    if let Some(other) = facts.counterparty
        && !labeled(accounts, r.account, r.amount, other)
    {
        for l in [Label::Category, Label::Purpose] {
            if facts.get(l).is_none() {
                facts.set(l, Value::None);
            }
        }
    }
    facts
}

/// Labels for each row, in order. A row's own facts are never suggested.
/// `accounts` are yours, to tell a transfer from an expense or income
/// between a liquid and a non-liquid account.
#[must_use]
pub fn labels(levels: &[&dyn Suggester], rows: &[Row], accounts: &[Account]) -> Vec<Picks> {
    let mut out: Vec<Picks> = vec![Picks::default(); rows.len()];
    for (level, s) in levels.iter().enumerate() {
        // What's settled so far, per row; rows with everything settled sit
        // this level out.
        let mut asked = Vec::new();
        let mut which = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let facts = settled(r, &out[i], accounts);
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
    // A transfer settled late: its category and purpose guesses go.
    for (r, p) in rows.iter().zip(&mut out) {
        let other = r.facts.counterparty.or_else(|| {
            p.counterparty
                .as_ref()
                .filter(|c| c.guess.confidence >= CONFIDENT)
                .map(|c| c.guess.value)
        });
        if other.is_some_and(|v| !labeled(accounts, r.account, r.amount, v)) {
            for l in [Label::Category, Label::Purpose] {
                *p.slot(l) = None;
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
mod tests;
