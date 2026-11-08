use super::*;

const CHECKING: Uuid = Uuid::from_u128(1);
const CARD: Uuid = Uuid::from_u128(2);
const SAVINGS: Uuid = Uuid::from_u128(3);
const USD: Uuid = Uuid::from_u128(9);

#[test]
fn how_sure_matching_is() {
    // A transfer: surer the closer its sides are dated.
    let transfer: Vec<i32> = [Some(0), Some(1), Some(3), Some(5), None]
        .into_iter()
        .map(transfer_confidence)
        .collect();
    assert_eq!(transfer, [1000, 970, 910, 850, 500]);
    // A possible duplicate never reaches confident, however alike.
    assert_eq!(near_confidence(0, 1000), 890);
    assert_eq!(near_confidence(1, 1000), 830);
    assert_eq!(near_confidence(2, 300), 147);
    assert_eq!(near_confidence(3, 0), 0);
}

fn near(row: u32, t: u128, similarity: i32, days_apart: i32) -> Near<u32> {
    Near {
        row,
        transaction: Uuid::from_u128(t),
        similarity,
        days_apart,
    }
}

#[test]
fn duplicates_are_one_to_one_best_first() {
    // Two identical coffees and one recorded coffee: one match. The
    // better description wins a transaction; closer dates break ties.
    let got = one_to_one(
        vec![
            near(0, 1, 200, 0),
            near(1, 1, 900, 2),
            near(2, 2, 500, 3),
            near(2, 3, 500, 1),
            near(3, 4, 1000, 0),
        ],
        &HashSet::from([3]),
        &HashSet::from([Uuid::from_u128(2)]),
    );
    assert_eq!(
        got,
        HashMap::from([(1, Uuid::from_u128(1)), (2, Uuid::from_u128(3))])
    );
}

fn staged(key: u32, account: Uuid, day: u8, cents: i64, side: Side) -> Staged<u32> {
    Staged {
        key,
        account,
        commodity: USD,
        date: Date::from_calendar_date(2026, time::Month::March, day).unwrap(),
        amount: Decimal::new(cents, 2),
        side,
        eligible: true,
    }
}

#[test]
fn rows_that_fit_pair_unless_you_chose_otherwise() {
    let rows = vec![
        // A card payment from checking and its arrival on the card:
        // neither side chosen, still a pair.
        staged(1, CHECKING, 13, -64_218, Side::Open),
        staged(2, CARD, 10, 64_218, Side::Open),
        // A counterparty you chose: never paired.
        staged(3, CHECKING, 9, -5000, Side::Open),
        staged(4, SAVINGS, 9, 5000, Side::Outside),
        // A transfer you chose to another account: not paired either.
        staged(5, CHECKING, 20, -700, Side::Transfer(CARD)),
        staged(6, SAVINGS, 20, 700, Side::Open),
        // A day past the window: too far.
        staged(7, CHECKING, 1, -900, Side::Open),
        staged(
            8,
            CARD,
            2 + u8::try_from(TRANSFER_DAYS).unwrap(),
            900,
            Side::Open,
        ),
    ];
    assert_eq!(transfer_matches(&rows), HashMap::from([(1, 2), (2, 1)]));
}

#[test]
fn pairs_are_one_to_one_chosen_first_then_closest() {
    let rows = vec![
        staged(1, CHECKING, 10, -10_000, Side::Transfer(SAVINGS)),
        // Closer, but not chosen.
        staged(2, SAVINGS, 10, 10_000, Side::Open),
        // Chosen as the transfer back: pairs chosen on both sides go
        // first, the closest of them (with 4) before the other (with 1).
        staged(3, SAVINGS, 13, 10_000, Side::Transfer(CHECKING)),
        staged(4, CHECKING, 11, -10_000, Side::Transfer(SAVINGS)),
        // Too far apart, the wrong sign: never.
        staged(5, SAVINGS, 20, 10_000, Side::Open),
        staged(6, SAVINGS, 10, -10_000, Side::Open),
    ];
    let got = transfer_matches(&rows);
    // 1 takes the one left.
    assert_eq!(got, HashMap::from([(3, 4), (4, 3), (1, 2), (2, 1)]));
    // A row that can't be matched (a duplicate, or split) isn't.
    let mut rows = rows;
    rows[2].eligible = false;
    rows[3].eligible = false;
    assert_eq!(transfer_matches(&rows), HashMap::from([(1, 2), (2, 1)]));
}

#[test]
fn a_transfer_chosen_elsewhere_is_a_conflict() {
    let rows = vec![
        // You chose savings, but the card holds the other side.
        staged(1, CHECKING, 13, -100_429, Side::Transfer(SAVINGS)),
        staged(2, CARD, 10, 100_429, Side::Open),
        // Another, the same day.
        staged(3, CHECKING, 13, -2000, Side::Transfer(SAVINGS)),
        staged(4, CARD, 13, 2000, Side::Open),
        // Paired with the account you chose: no conflict with the card.
        staged(5, CHECKING, 13, -300, Side::Transfer(SAVINGS)),
        staged(6, SAVINGS, 13, 300, Side::Open),
        staged(7, CARD, 13, 300, Side::Open),
    ];
    let pairs = transfer_matches(&rows);
    assert_eq!(pairs, HashMap::from([(5, 6), (6, 5)]));
    assert_eq!(
        transfer_conflicts(&rows, &pairs),
        HashMap::from([(1, 2), (2, 1), (3, 4), (4, 3)])
    );
}
