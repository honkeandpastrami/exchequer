use super::*;

#[test]
fn scores_count_by_confidence() {
    let mut s = Score::default();
    let g = |value: u8, confidence: u16| Guess {
        value,
        confidence,
        reason: String::new(),
    };
    let class = |v: &u8| Value::Id(Uuid::from_u128(u128::from(*v)));
    s.add(Some(&g(1, 950)), &1, class);
    s.add(Some(&g(1, 1000)), &2, class);
    s.add(Some(&g(1, 420)), &1, class);
    s.add(None, &1, class);
    assert_eq!(
        (
            s.asked,
            s.guessed,
            s.correct,
            s.confident,
            s.confident_correct
        ),
        (4, 3, 2, 2, 1)
    );
    assert_eq!((s.bins[9], s.bins[4]), ((2, 1), (1, 1)));
}

#[test]
fn metrics_are_the_usual_ones() {
    // Truth a a a b, guesses a (sure) b (sure) a (unsure) and none.
    let mut s = Score::default();
    let g = |value: u8, confidence: u16| Guess {
        value,
        confidence,
        reason: String::new(),
    };
    let class = |v: &u8| Value::Id(Uuid::from_u128(u128::from(*v)));
    s.add(Some(&g(1, 950)), &1, class);
    s.add(Some(&g(2, 950)), &1, class);
    s.add(Some(&g(1, 500)), &1, class);
    s.add(None, &2, class);
    let pm = |x: Option<f64>| x.map(|x| (x * 1000.0).round() as i64);
    let all = s.metrics(false);
    // Micro: 2 right of 3 guesses, of 4 rows; F1 = 2·2 / (3 + 4).
    assert_eq!(
        [
            all.coverage,
            all.precision,
            all.recall,
            all.accuracy,
            all.f1
        ]
        .map(pm),
        [Some(750), Some(667), Some(500), Some(500), Some(571)]
    );
    // a: P 2/2, R 2/3, F1 0.8; b: P 0/1, R 0/1, F1 0.
    assert_eq!(
        [all.macro_precision, all.macro_recall, all.macro_f1].map(pm),
        [Some(500), Some(333), Some(400)]
    );
    // 1 right of 2 confident guesses.
    let sure = s.metrics(true);
    assert_eq!(
        [sure.coverage, sure.precision, sure.recall].map(pm),
        [Some(500), Some(500), Some(250)]
    );
    let mut twice = s.clone();
    twice.merge(&s);
    assert_eq!(twice.metrics(false).f1, all.f1);
    assert_eq!(twice.asked, 8);
}
