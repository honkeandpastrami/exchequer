//! Bank account numbers, masked everywhere by default (design §3.6).

use std::fmt;

/// A bank account or card number.
///
/// `Display` and `Debug` both print only the last four characters (`…1234`),
/// so logging or formatting one by accident never leaks it. The full value is
/// available only through [`AccountNumber::reveal`], meant for parsers and the
/// database layer.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AccountNumber(String);

impl AccountNumber {
    /// Spaces and dashes are dropped, so `1234-5678` and `1234 5678` are the
    /// same number. Returns `None` if nothing is left.
    #[must_use]
    pub fn new(number: &str) -> Option<Self> {
        let n: String = number
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        (!n.is_empty()).then_some(Self(n))
    }

    /// The full number. Never log or display the result.
    #[must_use]
    pub fn reveal(&self) -> &str {
        &self.0
    }

    /// The last four characters (fewer if the number is shorter).
    #[must_use]
    pub fn last4(&self) -> &str {
        let start = self.0.char_indices().rev().nth(3).map_or(0, |(i, _)| i);
        &self.0[start..]
    }
}

impl fmt::Display for AccountNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "…{}", self.last4())
    }
}

impl fmt::Debug for AccountNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountNumber(…{})", self.last4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn masked_by_default() {
        let n = AccountNumber::new(" 000123456789 ").unwrap();
        assert_eq!(n.to_string(), "…6789");
        assert_eq!(format!("{n:?}"), "AccountNumber(…6789)");
        assert_eq!(n.reveal(), "000123456789");
        assert_eq!(AccountNumber::new("12").unwrap().to_string(), "…12");
        assert!(AccountNumber::new("  ").is_none());
        assert!(AccountNumber::new(" - ").is_none());
        assert_eq!(
            AccountNumber::new("1234-5678 90"),
            AccountNumber::new("1234567890")
        );
    }

    proptest! {
        #[test]
        fn formatting_shows_at_most_four_digits(digits in "[0-9]{5,20}") {
            let n = AccountNumber::new(&digits).unwrap();
            for shown in [n.to_string(), format!("{n:?}"), format!("{:?}", Some(&n))] {
                let count = shown.chars().filter(char::is_ascii_digit).count();
                prop_assert!(count <= 4, "{shown}");
            }
        }
    }
}
