use super::*;

fn a(s: &str) -> Amount {
    s.parse().unwrap()
}

#[test]
fn json_is_a_string_and_numbers_are_rejected() {
    assert_eq!(
        serde_json::to_string(&a("1500.00")).unwrap(),
        r#""1500.00""#
    );
    assert_eq!(
        serde_json::from_str::<Amount>(r#""-0.10""#).unwrap(),
        a("-0.1")
    );
    for bad in ["1500.00", "1500", r#""1e3""#, r#""abc""#, "null"] {
        assert!(serde_json::from_str::<Amount>(bad).is_err(), "{bad}");
    }
}

#[test]
fn point_one_plus_point_two_is_point_three() {
    assert_eq!(a("0.1").checked_add(a("0.2")).unwrap(), a("0.3"));
}

#[test]
fn trailing_zeros_do_not_matter() {
    assert_eq!(a("80"), a("80.00"));
    assert_eq!(a("1.50").significant_scale(), 1);
    assert_eq!(a("1200").significant_scale(), 0);
}

#[test]
fn exponents_and_garbage_are_rejected() {
    for bad in ["1e3", "1E3", "abc", "", "1.2.3", "NaN", "inf"] {
        assert!(bad.parse::<Amount>().is_err(), "{bad}");
        assert!(bad.parse::<Rate>().is_err(), "{bad}");
    }
}

#[test]
fn overflow_is_an_error() {
    let max = Amount::from_decimal(Decimal::MAX);
    assert_eq!(max.checked_add(a("1")), Err(NumberError::Overflow));
}

#[test]
fn large_balances_stay_exact() {
    let big = a("99999999999999999.99");
    assert_eq!(big.checked_add(a("0.01")).unwrap(), a("100000000000000000"));
}

#[test]
fn implied_rate_and_conversion() {
    // USD 1,100 paid for EUR 1,000: 1 EUR = 1.1 USD.
    let r = Rate::implied(a("1100"), a("1000")).unwrap();
    assert_eq!(r.to_string(), "1.1");
    assert_eq!(r.convert(a("250"), 2).unwrap(), a("275"));
    assert_eq!(
        Rate::implied(a("1"), a("0")),
        Err(NumberError::DivisionByZero)
    );
}

#[test]
fn conversion_rounds_once_half_even() {
    let r: Rate = "0.5".parse().unwrap();
    assert_eq!(r.convert(a("0.05"), 2).unwrap(), a("0.02")); // 0.025 → 0.02
    assert_eq!(r.convert(a("0.07"), 2).unwrap(), a("0.04")); // 0.035 → 0.04
    // A zero-decimal currency.
    let jpy: Rate = "151.237".parse().unwrap();
    assert_eq!(jpy.convert(a("10"), 0).unwrap(), a("1512"));
}

#[test]
fn repeating_rates_are_rounded_to_rate_scale() {
    let r = Rate::implied(a("1"), a("3")).unwrap();
    assert_eq!(r.to_string(), "0.33333333333333333333");
    assert_eq!(r.inverse().unwrap().to_string(), "3.00000000000000000003");
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    /// Amounts with up to 6 decimals and magnitudes up to a trillion: room for
    /// thousands of additions without overflow.
    fn amount() -> impl Strategy<Value = Amount> {
        (
            -1_000_000_000_000_000_000i64..1_000_000_000_000_000_000,
            0u32..=6,
        )
            .prop_map(|(m, s)| Amount::from_decimal(Decimal::new(m, s)))
    }

    proptest! {
        #[test]
        fn add_then_subtract_is_identity(x in amount(), y in amount()) {
            prop_assert_eq!(x.checked_add(y)?.checked_sub(y)?, x);
        }

        #[test]
        fn sums_do_not_depend_on_order(mut xs in prop::collection::vec(amount(), 0..50)) {
            let forward = Amount::sum(xs.iter().copied())?;
            xs.reverse();
            prop_assert_eq!(Amount::sum(xs.iter().copied())?, forward);
            xs.sort();
            prop_assert_eq!(Amount::sum(xs.iter().copied())?, forward);
        }

        #[test]
        fn display_round_trips(x in amount()) {
            prop_assert_eq!(x.to_string().parse::<Amount>()?, x);
        }

        #[test]
        fn converting_at_rate_one_is_identity(x in amount()) {
            let one: Rate = "1".parse()?;
            prop_assert_eq!(one.convert(x, 6)?, x);
        }
    }
}
