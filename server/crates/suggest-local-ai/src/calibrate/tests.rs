use super::*;

#[test]
fn confidence_rises_with_the_score_and_needs_evidence() {
    let mut points = Vec::new();
    // Low scores: right half the time; high scores: always right.
    for i in 0..100 {
        points.push((0.2, i % 2 == 0));
        points.push((0.9, true));
    }
    // A violator: a mid score worse than the low ones.
    points.push((0.5, false));
    let c = Curve::fit(points);
    assert!(c.apply(0.2) <= 500, "{c:?}");
    assert_eq!(c.apply(0.95), 990);
    assert_eq!(c.apply(0.0), c.apply(0.2));
    assert!(c.apply(0.5) <= c.apply(0.9));
    // One right guess is not enough to be confident.
    assert!(Curve::fit(vec![(1.0, true)]).apply(1.0) < 900);
    assert_eq!(Curve::default().apply(1.0), 500);
}
