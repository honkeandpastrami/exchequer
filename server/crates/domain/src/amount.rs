//! Exact numbers for money and quantities (design §3.5).
//!
//! [`Amount`] holds amounts and commodity quantities; [`Rate`] holds prices and
//! exchange rates. Neither has a constructor or operator that takes `f32`/`f64`,
//! so mixing in a float is a compile error, and arithmetic that would overflow
//! returns an error instead of rounding.

use std::fmt;
use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode, Zero};
use rust_decimal::Decimal;

/// Scale at which derived rates (e.g. implied prices) are stored.
pub const RATE_SCALE: i64 = 20;

/// The one rounding mode used at explicit rounding boundaries.
const ROUNDING: RoundingMode = RoundingMode::HalfEven;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NumberError {
    #[error("not a decimal number: {0:?}")]
    Parse(String),
    #[error("arithmetic overflow")]
    Overflow,
    #[error("division by zero")]
    DivisionByZero,
}

/// An exact decimal amount or quantity (28 significant digits).
///
/// Equality ignores trailing zeros: `80` equals `80.00`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Amount(Decimal);

impl Amount {
    pub const ZERO: Self = Self(Decimal::ZERO);

    #[must_use]
    pub const fn from_decimal(d: Decimal) -> Self {
        Self(d)
    }

    #[must_use]
    pub const fn as_decimal(&self) -> Decimal {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, NumberError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(NumberError::Overflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, NumberError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(NumberError::Overflow)
    }

    /// Sums exactly; fails on overflow rather than losing digits.
    pub fn sum<I: IntoIterator<Item = Self>>(items: I) -> Result<Self, NumberError> {
        items
            .into_iter()
            .try_fold(Self::ZERO, |acc, x| acc.checked_add(x))
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.0.is_sign_positive() && !self.0.is_zero()
    }

    /// Decimal places actually needed, ignoring trailing zeros (`1.50` → 1).
    #[must_use]
    pub fn significant_scale(&self) -> u32 {
        self.0.normalize().scale()
    }

    fn to_big(self) -> BigDecimal {
        // Through the exact decimal string: no binary conversion involved.
        BigDecimal::from_str(&self.0.to_string()).expect("a Decimal always prints as a decimal")
    }
}

impl std::ops::Neg for Amount {
    type Output = Self;

    /// Negation never overflows: `Decimal` is sign-magnitude.
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl FromStr for Amount {
    type Err = NumberError;

    /// Accepts plain decimal notation only (`-1234.50`); no exponents.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Decimal::from_str_exact(s)
            .map(Self)
            .map_err(|_| NumberError::Parse(s.to_owned()))
    }
}

/// JSON: always a string (`"1500.00"`), never a number (design §3.5). A JSON
/// number is rejected, because by the time it's parsed it may already have
/// passed through a float.
impl serde::Serialize for Amount {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Amount {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Amount;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a decimal number as a string, e.g. \"1500.00\"")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Amount, E> {
                v.parse().map_err(E::custom)
            }
        }
        d.deserialize_str(Visitor)
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Amount({})", self.0)
    }
}

/// An exact price or exchange rate with arbitrary precision: `1 BASE = rate QUOTE`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rate(BigDecimal);

impl Rate {
    /// The rate implied by giving `paid` of one commodity for `received` of
    /// another: `paid / received`, rounded once to [`RATE_SCALE`].
    pub fn implied(paid: Amount, received: Amount) -> Result<Self, NumberError> {
        if received.is_zero() {
            return Err(NumberError::DivisionByZero);
        }
        let exact = paid.to_big() / received.to_big();
        Ok(Self(exact.with_scale_round(RATE_SCALE, ROUNDING)))
    }

    /// Converts `amount` at this rate, computing at full precision and rounding
    /// once, at the end, to `precision` decimal places.
    pub fn convert(&self, amount: Amount, precision: u32) -> Result<Amount, NumberError> {
        let exact = amount.to_big() * &self.0;
        let rounded = exact.with_scale_round(i64::from(precision), ROUNDING);
        Decimal::from_str_exact(&rounded.to_plain_string())
            .map(Amount)
            .map_err(|_| NumberError::Overflow)
    }

    /// The inverse rate (`1 QUOTE = 1/rate BASE`), rounded once to [`RATE_SCALE`].
    pub fn inverse(&self) -> Result<Self, NumberError> {
        if self.0.is_zero() {
            return Err(NumberError::DivisionByZero);
        }
        Ok(Self(
            self.0.inverse().with_scale_round(RATE_SCALE, ROUNDING),
        ))
    }
}

impl FromStr for Rate {
    type Err = NumberError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Reject exponents so the accepted syntax matches `Amount`.
        if s.contains(['e', 'E']) {
            return Err(NumberError::Parse(s.to_owned()));
        }
        BigDecimal::from_str(s)
            .map(Self)
            .map_err(|_| NumberError::Parse(s.to_owned()))
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.normalized().to_plain_string())
    }
}

impl fmt::Debug for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rate({self})")
    }
}

#[cfg(test)]
mod tests {
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
}

#[cfg(test)]
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
