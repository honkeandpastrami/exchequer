//! Household settlement (design §4.2–§4.5) on the fictional household: Alex
//! and Sam settle with the Household; Studio with its funder.

mod common;

use std::collections::BTreeMap;

use common::*;
use domain::settlement::{Effect, Item, LineKind, Settlement, SettlementError, SplitRef};
use domain::*;
use proptest::prelude::*;
use time::Date;
use time::macros::date;

#[test]
fn effects_follow_the_claims_table() {
    let h = H::new();
    let claim = |debtor, creditor, liquidation| Effect::Claim {
        debtor,
        creditor,
        liquidation,
    };

    // Groceries on Alex Credit for the Household: the Household owes Alex.
    assert_eq!(
        h.effect(&spend(&h, h.alex_credit, "80", h.household))
            .unwrap(),
        Some((claim(h.household, h.alex, false), amt("80")))
    );
    // Salary into Alex Checking for the Household: Alex owes the Household.
    let salary = Split {
        purpose: Some(h.household),
        ..split(
            Side::Counterparty(Some(h.store)),
            Side::Account(h.alex_checking),
            h.usd,
            "3000",
        )
    };
    assert_eq!(
        h.effect(&salary).unwrap(),
        Some((claim(h.alex, h.household, false), amt("3000")))
    );
    // Studio supplies on Sam Credit: Studio owes Sam.
    assert_eq!(
        h.effect(&spend(&h, h.sam_credit, "100", h.studio)).unwrap(),
        Some((claim(h.studio, h.sam, false), amt("100")))
    );
    // Owner = purpose, or no purpose: nothing.
    assert_eq!(
        h.effect(&spend(&h, h.alex_credit, "5", h.alex)).unwrap(),
        None
    );
    let unlabelled = split(
        Side::Account(h.alex_credit),
        Side::Counterparty(Some(h.store)),
        h.usd,
        "5",
    );
    assert_eq!(h.effect(&unlabelled).unwrap(), None);

    // Liquidations default to the Household: into Joint Savings they simply
    // become shared; into Alex Checking, Alex holds them for the Household.
    let into_joint = transfer(&h, h.alex_brokerage, h.joint_savings, "10000");
    assert_eq!(h.effect(&into_joint).unwrap(), None);
    let into_alex = transfer(&h, h.alex_brokerage, h.alex_checking, "10000");
    assert_eq!(
        h.effect(&into_alex).unwrap(),
        Some((claim(h.alex, h.household, true), amt("10000")))
    );
    let kept = Split {
        purpose: Some(h.alex),
        ..into_alex
    };
    assert_eq!(h.effect(&kept).unwrap(), None);

    // Transfers between liquid accounts of different owners are payments,
    // mortgage payments from a personal account included.
    for (from, to, payer, payee) in [
        (h.alex_checking, h.joint_savings, h.alex, h.household),
        (h.joint_savings, h.alex_checking, h.household, h.alex),
        (h.alex_checking, h.mortgage, h.alex, h.household),
    ] {
        assert_eq!(
            h.effect(&transfer(&h, from, to, "50")).unwrap(),
            Some((Effect::Payment { payer, payee }, amt("50")))
        );
    }
    // Within one owner, Household → Household (the mortgage from Joint
    // Savings), and investment activity: nothing.
    for (from, to) in [
        (h.alex_checking, h.alex_credit),
        (h.joint_savings, h.mortgage),
        (h.alex_checking, h.alex_brokerage),
    ] {
        assert_eq!(h.effect(&transfer(&h, from, to, "50")).unwrap(), None);
    }
    let buy = Split {
        to: Leg {
            side: Side::Account(h.alex_brokerage),
            commodity: h.acme,
            quantity: amt("2"),
        },
        ..transfer(&h, h.alex_brokerage, h.alex_brokerage, "300")
    };
    assert_eq!(h.effect(&buy).unwrap(), None);
}

#[test]
fn values_are_in_the_household_currency() {
    let h = H::new();
    // Spent in EUR from a EUR account: no USD value until prices exist.
    let eur = Split {
        purpose: Some(h.household),
        ..split(
            Side::Account(h.alex_eur),
            Side::Counterparty(Some(h.store)),
            h.eur,
            "90",
        )
    };
    assert_eq!(h.effect(&eur).unwrap_err(), SettlementError::Unpriced);
    // A EUR purchase charged in USD: the card's USD leg is the value.
    let charged = Split {
        to: Leg {
            side: Side::Counterparty(Some(h.store)),
            commodity: h.eur,
            quantity: amt("100"),
        },
        ..spend(&h, h.alex_credit, "110.40", h.household)
    };
    assert_eq!(h.effect(&charged).unwrap().unwrap().1, amt("110.40"));
}

#[test]
fn a_settled_month() {
    let h = H::new();
    let items = h.items(&[
        (
            date!(2026 - 06 - 03),
            spend(&h, h.alex_credit, "80", h.household),
        ),
        (
            date!(2026 - 06 - 20),
            spend(&h, h.alex_credit, "20", h.household),
        ),
        (
            date!(2026 - 06 - 25),
            spend(&h, h.sam_credit, "40", h.household),
        ),
        // Paid early in July, so it settles June.
        (
            date!(2026 - 07 - 02),
            transfer(&h, h.joint_savings, h.alex_checking, "600"),
        ),
    ]);
    let params = BTreeMap::from([(
        month("2026-06"),
        params(&[(h.alex, "500"), (h.sam, "0")], &[]),
    )]);
    let s = Settlement::compute(&h.parties, &params, &items, month("2026-12")).unwrap();

    let june = s.statement(month("2026-06")).unwrap();
    assert_eq!(june.params_from, Some(month("2026-06")));
    let alex = june.edges.iter().find(|e| e.child == h.alex).unwrap();
    assert_eq!(alex.parent, h.household);
    assert_eq!(alex.claims, amt("-100"));
    assert_eq!(alex.stipend, amt("500"));
    assert_eq!(alex.due, amt("-600"));
    assert_eq!(alex.paid, amt("-600"));
    assert_eq!(alex.outstanding, Amount::ZERO);
    assert_eq!(alex.implied_stipend().unwrap(), amt("500"));
    assert_eq!(
        alex.lines.iter().map(|l| l.kind).collect::<Vec<_>>(),
        [LineKind::Claim, LineKind::Claim, LineKind::Payment]
    );

    let sam = june.edges.iter().find(|e| e.child == h.sam).unwrap();
    assert_eq!(sam.outstanding, amt("-40"), "the Household still owes Sam");

    // July carries June's parameters forward: Alex's stipend is due again.
    let july = s.statement(month("2026-07")).unwrap();
    assert_eq!(july.params_from, Some(month("2026-06")));
    let alex = july.edges.iter().find(|e| e.child == h.alex).unwrap();
    assert_eq!(
        (alex.due, alex.paid, alex.lines.len()),
        (amt("-500"), Amount::ZERO, 0)
    );
    assert_eq!(alex.outstanding, amt("-500"));
    // What June left unsettled is carried into July.
    assert_eq!(alex.carried, Amount::ZERO);
    let sam = july.edges.iter().find(|e| e.child == h.sam).unwrap();
    assert_eq!(sam.carried, amt("-40"));
    assert_eq!(
        sam.outstanding,
        Amount::sum([sam.carried, sam.due, -sam.paid]).unwrap()
    );
}

#[test]
fn routing_follows_the_funder() {
    let h = H::new();
    let items = h.items(&[
        (
            date!(2026 - 06 - 10),
            spend(&h, h.sam_credit, "100", h.studio),
        ),
        (
            date!(2026 - 07 - 10),
            spend(&h, h.sam_credit, "100", h.studio),
        ),
    ]);
    let params = BTreeMap::from([
        (month("2026-06"), params(&[], &[(h.studio, h.household)])),
        (month("2026-07"), params(&[], &[(h.studio, h.sam)])),
    ]);
    let s = Settlement::compute(&h.parties, &params, &items, month("2026-12")).unwrap();
    let edge = |m: &str, child| {
        s.statement(month(m))
            .unwrap()
            .edges
            .into_iter()
            .find(|e| e.child == child)
            .unwrap()
    };

    // June: Studio → Household → Sam.
    assert_eq!(edge("2026-06", h.studio).claims, amt("100"));
    assert_eq!(edge("2026-06", h.studio).parent, h.household);
    assert_eq!(edge("2026-06", h.sam).claims, amt("-100"));
    // July, funded by Sam: Studio → Sam directly; the Household isn't involved.
    assert_eq!(edge("2026-07", h.studio).claims, amt("100"));
    assert_eq!(edge("2026-07", h.studio).parent, h.sam);
    assert_eq!(edge("2026-07", h.sam).claims, Amount::ZERO);
    // A business edge's outstanding is its net funding.
    assert_eq!(edge("2026-07", h.studio).outstanding, amt("200"));
}

#[test]
fn liquidations_stay_in_transit_until_moved() {
    let h = H::new();
    let mut items = h.items(&[
        (
            date!(2026 - 05 - 20),
            transfer(&h, h.alex_brokerage, h.alex_checking, "10000"),
        ),
        (
            date!(2026 - 05 - 28),
            spend(&h, h.alex_credit, "80", h.household),
        ),
        // Moved on to shared savings in June: clears the line.
        (
            date!(2026 - 06 - 03),
            transfer(&h, h.alex_checking, h.joint_savings, "10000"),
        ),
        // The regular settlement of May.
        (
            date!(2026 - 06 - 05),
            transfer(&h, h.joint_savings, h.alex_checking, "80"),
        ),
    ]);
    let s = Settlement::compute(&h.parties, &BTreeMap::new(), &items, month("2026-12")).unwrap();
    let alex = |m: &str| {
        s.statement(month(m))
            .unwrap()
            .edges
            .into_iter()
            .find(|e| e.child == h.alex)
            .unwrap()
    };

    let may = alex("2026-05");
    assert_eq!(
        may.claims,
        amt("-80"),
        "the liquidation isn't a regular claim"
    );
    assert_eq!(
        may.paid,
        amt("-80"),
        "the clearing transfer isn't a regular payment"
    );
    assert_eq!(may.in_transit.len(), 1);
    assert_eq!(may.in_transit[0].amount, amt("10000"));
    assert_eq!(
        may.in_transit[0].cleared_by, None,
        "not cleared by the end of May"
    );
    assert_eq!(may.outstanding, amt("10000"));
    assert_eq!(may.to_settle, Amount::ZERO);

    let june = alex("2026-06");
    assert_eq!(june.in_transit.len(), 1);
    assert!(june.in_transit[0].cleared_by.is_some());
    assert_eq!(
        (june.outstanding, june.to_settle),
        (Amount::ZERO, Amount::ZERO)
    );
    assert!(june.lines.iter().any(|l| l.kind == LineKind::Clearing));
    assert!(
        alex("2026-07").in_transit.is_empty(),
        "cleared lines don't carry forward"
    );

    // Linked by hand to a month, the same transfer is a regular payment and
    // the line stays open.
    items[2].settles = Some(month("2026-06"));
    let s = Settlement::compute(&h.parties, &BTreeMap::new(), &items, month("2026-12")).unwrap();
    let july = s.statement(month("2026-07")).unwrap();
    let alex = july.edges.iter().find(|e| e.child == h.alex).unwrap();
    assert_eq!(alex.in_transit.len(), 1, "carried forward as a reminder");
    assert_eq!(alex.in_transit[0].cleared_by, None);
}

#[test]
fn a_payment_can_settle_any_month() {
    let h = H::new();
    let mut items = h.items(&[
        (
            date!(2026 - 03 - 10),
            spend(&h, h.alex_credit, "70", h.household),
        ),
        (
            date!(2026 - 03 - 12),
            transfer(&h, h.joint_savings, h.alex_checking, "70"),
        ),
    ]);
    items[1].settles = Some(month("2026-03"));
    let s = Settlement::compute(&h.parties, &BTreeMap::new(), &items, month("2026-12")).unwrap();
    let march = s.statement(month("2026-03")).unwrap();
    let alex = march.edges.iter().find(|e| e.child == h.alex).unwrap();
    assert_eq!(
        (alex.due, alex.paid, alex.outstanding),
        (amt("-70"), amt("-70"), Amount::ZERO)
    );
    assert_eq!(march.params_from, None);
    let feb = s.statement(month("2026-02")).unwrap();
    assert!(feb.edges.iter().all(|e| e.outstanding.is_zero()));
}

proptest! {
    /// Whatever the claims and the funders, paying each claim back in the
    /// month it counts in leaves nothing outstanding on any edge.
    #[test]
    fn paid_back_claims_net_to_zero(
        claims in prop::collection::vec((0usize..4, 0usize..4, 1u32..100_000, 1u8..=12), 0..30),
        studio_under_sam in any::<bool>(),
    ) {
        let h = H::new();
        let ids = [h.household, h.alex, h.sam, h.studio];
        let mut items = Vec::new();
        for (i, (a, b, cents, m)) in claims.into_iter().enumerate() {
            if a == b {
                continue;
            }
            let date = Date::from_calendar_date(2026, m.try_into().unwrap(), 15).unwrap();
            let amount = Amount::from_decimal(rust_decimal::Decimal::new(cents.into(), 2));
            let source = |position| SplitRef { transaction: TransactionId::new(), position };
            items.push(Item {
                source: source(i),
                date,
                effect: Effect::Claim { debtor: ids[a], creditor: ids[b], liquidation: false },
                amount,
                settles: None,
            });
            items.push(Item {
                source: source(i),
                date,
                effect: Effect::Payment { payer: ids[a], payee: ids[b] },
                amount,
                settles: Some(Month::of(date)),
            });
        }
        let funder = if studio_under_sam { h.sam } else { h.household };
        let params = BTreeMap::from([(month("2026-01"), params(&[], &[(h.studio, funder)]))]);
        let s = Settlement::compute(&h.parties, &params, &items, month("2026-12")).unwrap();
        for m in 1..=12 {
            let st = s.statement(Month::new(2026, m).unwrap()).unwrap();
            for e in st.edges {
                prop_assert_eq!(e.due, e.paid);
                prop_assert!(e.outstanding.is_zero());
            }
        }
    }
}

#[test]
fn unknown_funders_are_refused() {
    let h = H::new();
    let bad = BTreeMap::from([(month("2026-01"), params(&[], &[(h.studio, h.studio)]))]);
    assert!(matches!(
        Settlement::compute(&h.parties, &bad, &[], month("2026-12")),
        Err(SettlementError::Tree(_))
    ));
}

#[test]
fn only_payments_settle_a_month() {
    let h = H::new();
    let settles = Some(month("2026-05"));
    let payment = Split {
        settles,
        ..transfer(&h, h.alex_checking, h.joint_savings, "50")
    };
    assert!(payment.classify(&h.book).is_ok());
    for s in [
        transfer(&h, h.alex_checking, h.alex_credit, "50"),
        transfer(&h, h.alex_brokerage, h.alex_checking, "50"),
        spend(&h, h.alex_credit, "50", h.household),
    ] {
        let s = Split { settles, ..s };
        assert_eq!(
            s.classify(&h.book).unwrap_err(),
            SplitError::SettlesNotAllowed
        );
    }
}

#[test]
fn settlement_starts_with_the_first_month_with_parameters() {
    let h = H::new();
    // Paid in January for December, whose claims were never recorded.
    let items = h.items(&[
        (
            date!(2026 - 01 - 10),
            transfer(&h, h.alex_checking, h.joint_savings, "700"),
        ),
        (
            date!(2026 - 01 - 20),
            spend(&h, h.alex_credit, "50", h.household),
        ),
    ]);
    let params = BTreeMap::from([(month("2026-01"), params(&[(h.alex, "0")], &[]))]);
    let s = Settlement::compute(&h.parties, &params, &items, month("2026-12")).unwrap();
    let alex = |m: &str| {
        s.statement(month(m))
            .unwrap()
            .edges
            .into_iter()
            .find(|e| e.child == h.alex)
            .unwrap()
    };
    assert_eq!(alex("2026-01").outstanding, amt("-50"));
    // December itself still shows what happened in it.
    assert_eq!(alex("2025-12").paid, amt("700"));
    assert_eq!(alex("2025-12").outstanding, amt("-700"));
}
