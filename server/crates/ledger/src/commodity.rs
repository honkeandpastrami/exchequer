//! Commodities: currencies, securities, funds, crypto (design §3.1).

use crate::amount::Amount;
use crate::ids::CommodityId;

/// Most decimal places a commodity may declare; `Amount` holds 28 digits.
pub const MAX_PRECISION: u32 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommodityKind {
    Fiat,
    Security,
    Fund,
    Crypto,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommodityError {
    #[error("commodity code must be 1-16 characters of A-Z, 0-9, '.', '-': {0:?}")]
    BadCode(String),
    #[error("precision {0} is above the maximum of {MAX_PRECISION}")]
    BadPrecision(u32),
    #[error("{amount} has more than {precision} decimal places allowed for {code}")]
    TooPrecise {
        amount: Amount,
        code: String,
        precision: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commodity {
    pub id: CommodityId,
    code: String,
    pub kind: CommodityKind,
    precision: u32,
}

impl Commodity {
    /// `code` is a ticker or currency code such as `USD`, `VTI` or `BRK.B`.
    pub fn new(
        id: CommodityId,
        code: &str,
        kind: CommodityKind,
        precision: u32,
    ) -> Result<Self, CommodityError> {
        let valid = (1..=16).contains(&code.len())
            && code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '-');
        if !valid {
            return Err(CommodityError::BadCode(code.to_owned()));
        }
        if precision > MAX_PRECISION {
            return Err(CommodityError::BadPrecision(precision));
        }
        Ok(Self {
            id,
            code: code.to_owned(),
            kind,
            precision,
        })
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Decimal places a quantity may have: 2 for USD, 0 for JPY, more for shares.
    #[must_use]
    pub const fn precision(&self) -> u32 {
        self.precision
    }

    /// Ledger amounts are exact: too many decimals is an error, never rounded.
    /// Trailing zeros don't count (`80.000` is fine for USD).
    pub fn check(&self, amount: Amount) -> Result<(), CommodityError> {
        if amount.significant_scale() > self.precision {
            return Err(CommodityError::TooPrecise {
                amount,
                code: self.code.clone(),
                precision: self.precision,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
