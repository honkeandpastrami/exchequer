use time::macros::date;

use super::*;

#[test]
fn buckets() {
    let d = date!(2026 - 08 - 19); // a Wednesday
    assert_eq!(Bucket::Day.start(d), d);
    assert_eq!(Bucket::Week.start(d), date!(2026 - 08 - 17));
    assert_eq!(Bucket::Month.start(d), date!(2026 - 08 - 01));
    assert_eq!(Bucket::Quarter.start(d), date!(2026 - 07 - 01));
    assert_eq!(Bucket::Year.start(d), date!(2026 - 01 - 01));
    assert_eq!(
        Bucket::Quarter.next(date!(2026 - 10 - 01)),
        date!(2027 - 01 - 01)
    );
    assert_eq!(
        Bucket::Month.next(date!(2026 - 12 - 01)),
        date!(2027 - 01 - 01)
    );
    assert_eq!(
        Bucket::Week.next(date!(2026 - 08 - 17)),
        date!(2026 - 08 - 24)
    );
}
