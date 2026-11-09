use time::macros::date;

use super::*;

#[test]
fn parse_and_display() {
    let m: Month = "2026-07".parse().unwrap();
    assert_eq!(m.to_string(), "2026-07");
    assert_eq!(m.first_day(), date!(2026 - 07 - 01));
    assert_eq!(m.last_day(), date!(2026 - 07 - 31));
    for bad in [
        "2026-7",
        "2026-13",
        "2026-00",
        "26-07",
        "2026/07",
        "2026-07-01",
        "+026-07",
    ] {
        assert!(bad.parse::<Month>().is_err(), "{bad}");
    }
}

#[test]
fn neighbours() {
    let jan: Month = "2026-01".parse().unwrap();
    assert_eq!(jan.previous().to_string(), "2025-12");
    assert_eq!(jan.previous().next(), jan);
    assert_eq!(
        "2024-02".parse::<Month>().unwrap().last_day(),
        date!(2024 - 02 - 29)
    );
    assert!(jan < jan.next());
    assert_eq!(Month::of(date!(2026 - 01 - 31)), jan);
}
