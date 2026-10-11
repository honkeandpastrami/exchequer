//! Parties and the settlement tree (design §4.1).
//!
//! The Household is the root; each person settles with the Household; each
//! business settles with its funder (the Household or a person), which can
//! change from month to month. A [`SettlementTree`] is therefore built for one
//! month, from the parties and that month's funders.

use std::collections::HashMap;

use crate::ids::PartyId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartyKind {
    Household,
    Person,
    Business,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    pub id: PartyId,
    pub name: String,
    pub kind: PartyKind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreeError {
    #[error("there must be exactly one household, found {0}")]
    HouseholdCount(usize),
    #[error("business {0} has no funder")]
    MissingFunder(PartyId),
    #[error("funder {funder} of business {business} must be the household or a person")]
    BadFunder { business: PartyId, funder: PartyId },
    #[error("party {0} is not a business, so it has no funder")]
    NotABusiness(PartyId),
    #[error("unknown party {0}")]
    UnknownParty(PartyId),
}

/// One edge crossed on a path through the tree. Every edge joins a `child`
/// to its parent; `upward` says whether the path goes from child to parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub child: PartyId,
    pub upward: bool,
}

#[derive(Debug, Clone)]
pub struct SettlementTree {
    household: PartyId,
    /// Every party except the household, mapped to its parent.
    parent: HashMap<PartyId, PartyId>,
}

impl SettlementTree {
    /// `funders` maps each business to its funder for the month.
    pub fn new(parties: &[Party], funders: &HashMap<PartyId, PartyId>) -> Result<Self, TreeError> {
        let kinds: HashMap<PartyId, PartyKind> = parties.iter().map(|p| (p.id, p.kind)).collect();
        let households: Vec<PartyId> = parties
            .iter()
            .filter(|p| p.kind == PartyKind::Household)
            .map(|p| p.id)
            .collect();
        let [household] = households[..] else {
            return Err(TreeError::HouseholdCount(households.len()));
        };

        for (&business, &funder) in funders {
            match kinds.get(&business) {
                None => return Err(TreeError::UnknownParty(business)),
                Some(PartyKind::Business) => {}
                Some(_) => return Err(TreeError::NotABusiness(business)),
            }
            match kinds.get(&funder) {
                None => return Err(TreeError::UnknownParty(funder)),
                Some(PartyKind::Household | PartyKind::Person) => {}
                Some(PartyKind::Business) => {
                    return Err(TreeError::BadFunder { business, funder });
                }
            }
        }

        let mut parent = HashMap::new();
        for p in parties {
            match p.kind {
                PartyKind::Household => {}
                PartyKind::Person => {
                    parent.insert(p.id, household);
                }
                PartyKind::Business => {
                    let funder = funders.get(&p.id).ok_or(TreeError::MissingFunder(p.id))?;
                    parent.insert(p.id, *funder);
                }
            }
        }
        Ok(Self { household, parent })
    }

    #[must_use]
    pub const fn household(&self) -> PartyId {
        self.household
    }

    /// The party's parent, or `None` for the household.
    #[must_use]
    pub fn parent(&self, party: PartyId) -> Option<PartyId> {
        self.parent.get(&party).copied()
    }

    /// The edges crossed going from `from` to `to`, in order. A claim between
    /// two parties is added to each of these edges (design §4.2, "Routing").
    /// Empty when `from == to`.
    pub fn path(&self, from: PartyId, to: PartyId) -> Result<Vec<Step>, TreeError> {
        let up_from = self.ancestors(from)?;
        let up_to = self.ancestors(to)?;
        // The lowest common ancestor is the first of `from`'s ancestors that
        // `to` also has; the household is always shared.
        let lca = *up_from
            .iter()
            .find(|p| up_to.contains(p))
            .expect("every party descends from the household");

        let mut steps: Vec<Step> = up_from
            .iter()
            .take_while(|&&p| p != lca)
            .map(|&child| Step {
                child,
                upward: true,
            })
            .collect();
        let down: Vec<Step> = up_to
            .iter()
            .take_while(|&&p| p != lca)
            .map(|&child| Step {
                child,
                upward: false,
            })
            .collect();
        steps.extend(down.into_iter().rev());
        Ok(steps)
    }

    /// `party`, then its parent, and so on up to the household.
    fn ancestors(&self, party: PartyId) -> Result<Vec<PartyId>, TreeError> {
        if party != self.household && !self.parent.contains_key(&party) {
            return Err(TreeError::UnknownParty(party));
        }
        let mut chain = vec![party];
        let mut current = party;
        // Depth is at most 3 (household → person → business), so no cycles.
        while let Some(&p) = self.parent.get(&current) {
            chain.push(p);
            current = p;
        }
        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Example {
        household: PartyId,
        alex: PartyId,
        sam: PartyId,
        studio: PartyId,
        parties: Vec<Party>,
    }

    fn example() -> Example {
        let party = |name: &str, kind| Party {
            id: PartyId::new(),
            name: name.to_owned(),
            kind,
        };
        let parties = vec![
            party("Household", PartyKind::Household),
            party("Alex", PartyKind::Person),
            party("Sam", PartyKind::Person),
            party("Studio", PartyKind::Business),
        ];
        Example {
            household: parties[0].id,
            alex: parties[1].id,
            sam: parties[2].id,
            studio: parties[3].id,
            parties,
        }
    }

    fn up(child: PartyId) -> Step {
        Step {
            child,
            upward: true,
        }
    }

    fn down(child: PartyId) -> Step {
        Step {
            child,
            upward: false,
        }
    }

    #[test]
    fn household_funded_business() {
        let e = example();
        let tree =
            SettlementTree::new(&e.parties, &HashMap::from([(e.studio, e.household)])).unwrap();
        assert_eq!(tree.path(e.alex, e.household).unwrap(), vec![up(e.alex)]);
        assert_eq!(tree.path(e.household, e.alex).unwrap(), vec![down(e.alex)]);
        assert_eq!(tree.path(e.alex, e.alex).unwrap(), vec![]);
        // Sam pays a Studio expense: routed Studio → Household → Sam.
        assert_eq!(
            tree.path(e.studio, e.sam).unwrap(),
            vec![up(e.studio), down(e.sam)]
        );
    }

    #[test]
    fn person_funded_business() {
        let e = example();
        let tree = SettlementTree::new(&e.parties, &HashMap::from([(e.studio, e.sam)])).unwrap();
        // Directly Studio → Sam once Sam is the funder.
        assert_eq!(tree.path(e.studio, e.sam).unwrap(), vec![up(e.studio)]);
        assert_eq!(
            tree.path(e.studio, e.alex).unwrap(),
            vec![up(e.studio), up(e.sam), down(e.alex)]
        );
        assert_eq!(
            tree.path(e.studio, e.household).unwrap(),
            vec![up(e.studio), up(e.sam)]
        );
    }

    #[test]
    fn invalid_trees() {
        let e = example();
        assert_eq!(
            SettlementTree::new(&e.parties, &HashMap::new()).unwrap_err(),
            TreeError::MissingFunder(e.studio)
        );
        assert_eq!(
            SettlementTree::new(&e.parties, &HashMap::from([(e.studio, e.studio)])).unwrap_err(),
            TreeError::BadFunder {
                business: e.studio,
                funder: e.studio
            }
        );
        assert_eq!(
            SettlementTree::new(
                &e.parties,
                &HashMap::from([(e.studio, e.household), (e.alex, e.household)])
            )
            .unwrap_err(),
            TreeError::NotABusiness(e.alex)
        );
        assert_eq!(
            SettlementTree::new(&e.parties[1..], &HashMap::new()).unwrap_err(),
            TreeError::HouseholdCount(0)
        );
        let tree =
            SettlementTree::new(&e.parties, &HashMap::from([(e.studio, e.household)])).unwrap();
        let stranger = PartyId::new();
        assert_eq!(
            tree.path(stranger, e.alex).unwrap_err(),
            TreeError::UnknownParty(stranger)
        );
    }
}
