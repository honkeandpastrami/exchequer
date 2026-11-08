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
mod tests;
