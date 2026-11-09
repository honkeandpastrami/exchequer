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
mod tests;
