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
    let tree = SettlementTree::new(&e.parties, &HashMap::from([(e.studio, e.household)])).unwrap();
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
    let tree = SettlementTree::new(&e.parties, &HashMap::from([(e.studio, e.household)])).unwrap();
    let stranger = PartyId::new();
    assert_eq!(
        tree.path(stranger, e.alex).unwrap_err(),
        TreeError::UnknownParty(stranger)
    );
}
