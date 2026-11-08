//! Valuation (design §3.4): what one commodity is worth in another on a date,
//! from stored prices. Pure: the caller loads the prices it may need.
//!
//! A figure comes from one price when it can. In order:
//!
//! 1. **Fresh before stale:** a price at most `fresh_days` older than the date
//!    beats any older one. Only when nothing fresh exists is the newest stale
//!    figure used, and it says so.
//! 2. **Direct before derived:** one price (or its inverse) beats a chain of
//!    two through a pivot commodity; a chain whose two prices come from one
//!    source beats one mixing sources.
//! 3. **Source order:** manual prices first, then the configured sources in
//!    order, then sources no longer configured (their prices outlive them).

use std::fmt;

use time::Date;

use crate::amount::Rate;
use crate::ids::CommodityId;

/// Where a price came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    /// Entered by hand.
    Manual,
    /// Fetched from a price source, by its name (`tiingo`, `ecb`).
    Provider(String),
}

impl Source {
    /// `manual` or `provider:<name>`.
    pub fn parse(s: &str) -> Option<Self> {
        if s == "manual" {
            return Some(Self::Manual);
        }
        let name = s.strip_prefix("provider:")?;
        let valid = (1..=32).contains(&name.len())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        valid.then(|| Self::Provider(name.to_owned()))
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual => f.write_str("manual"),
            Self::Provider(name) => write!(f, "provider:{name}"),
        }
    }
}

/// `1 base = rate quote` on a date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Price {
    pub base: CommodityId,
    pub quote: CommodityId,
    pub date: Date,
    pub rate: Rate,
    pub source: Source,
    /// For a day still in progress: replaced by the close once available.
    pub provisional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PriceError {
    #[error("a price must be positive")]
    NotPositive,
    #[error(
        "a price may have at most {} decimal places",
        crate::amount::RATE_SCALE
    )]
    TooPrecise,
    #[error("a price is between two different commodities")]
    SameCommodity,
}

impl Price {
    /// Checks what the database also checks.
    pub fn check(&self) -> Result<(), PriceError> {
        if self.base == self.quote {
            return Err(PriceError::SameCommodity);
        }
        if !self.rate.is_positive() {
            return Err(PriceError::NotPositive);
        }
        if self.rate.significant_scale() > crate::amount::RATE_SCALE {
            return Err(PriceError::TooPrecise);
        }
        Ok(())
    }
}

/// A stored price used in a valuation, as stored or inverted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Used {
    pub price: Price,
    pub inverted: bool,
}

/// `1 from = rate to`, and the prices it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valuation {
    pub rate: Rate,
    /// Empty when the commodities are the same; two for a chain.
    pub used: Vec<Used>,
    /// Nothing fresh was found: the newest older figure was used.
    pub stale: bool,
}

impl Valuation {
    /// Chained through a pivot commodity, not one price.
    #[must_use]
    pub fn derived(&self) -> bool {
        self.used.len() > 1
    }

    #[must_use]
    pub fn provisional(&self) -> bool {
        self.used.iter().any(|u| u.price.provisional)
    }
}

/// The prices a request may need, and how to choose among them.
#[derive(Debug, Clone)]
pub struct Prices {
    prices: Vec<Price>,
    /// Provider names, most preferred first.
    order: Vec<String>,
    fresh_days: i64,
}

/// A way to value: its prices, and what ranks it.
struct Candidate {
    used: Vec<Used>,
    /// The oldest price's date.
    date: Date,
    /// The least preferred source's rank.
    rank: usize,
}

impl Prices {
    /// `order`: the configured sources, most preferred first. A price at
    /// most `fresh_days` older than the date asked is fresh.
    #[must_use]
    pub fn new(prices: Vec<Price>, order: Vec<String>, fresh_days: u32) -> Self {
        Self {
            prices,
            order,
            fresh_days: i64::from(fresh_days),
        }
    }

    /// What `1 from` is worth in `to` at the end of `date`, or `None` when no
    /// stored price connects them.
    #[must_use]
    pub fn value(&self, from: CommodityId, to: CommodityId, date: Date) -> Option<Valuation> {
        if from == to {
            return Some(Valuation {
                rate: Rate::one(),
                used: Vec::new(),
                stale: false,
            });
        }
        let tiers = [
            self.direct(from, to, date),
            self.chains(from, to, date, true),
            self.chains(from, to, date, false),
        ];
        let fresh = |c: &&Candidate| (date - c.date).whole_days() <= self.fresh_days;
        let best_fresh = tiers.iter().find_map(|tier| {
            tier.iter()
                .filter(fresh)
                .min_by(|a, b| a.rank.cmp(&b.rank).then(b.date.cmp(&a.date)))
        });
        let (best, stale) = match best_fresh {
            Some(c) => (c, false),
            // The newest, then direct before derived, then by source.
            None => (
                tiers
                    .iter()
                    .enumerate()
                    .flat_map(|(t, tier)| tier.iter().map(move |c| (t, c)))
                    .min_by(|(ta, a), (tb, b)| {
                        b.date
                            .cmp(&a.date)
                            .then(ta.cmp(tb))
                            .then(a.rank.cmp(&b.rank))
                    })?
                    .1,
                true,
            ),
        };
        let rate = best.used.iter().try_fold(Rate::one(), |acc, u| {
            let r = if u.inverted {
                u.price.rate.inverse().ok()?
            } else {
                u.price.rate.clone()
            };
            Some(acc.then(&r))
        })?;
        Some(Valuation {
            rate,
            used: best.used.clone(),
            stale,
        })
    }

    fn rank(&self, source: &Source) -> usize {
        match source {
            Source::Manual => 0,
            Source::Provider(name) => self
                .order
                .iter()
                .position(|n| n == name)
                .map_or(self.order.len() + 1, |i| i + 1),
        }
    }

    /// Each source's latest price for `from → to` on or before `date`,
    /// stored either way round (as stored wins a tie).
    fn latest(&self, from: CommodityId, to: CommodityId, date: Date) -> Vec<Used> {
        let mut best: Vec<Used> = Vec::new();
        for p in self.prices.iter().filter(|p| p.date <= date) {
            let inverted = if p.base == from && p.quote == to {
                false
            } else if p.base == to && p.quote == from {
                true
            } else {
                continue;
            };
            let newer = |u: &Used| (p.date, !inverted) > (u.price.date, !u.inverted);
            match best.iter_mut().find(|u| u.price.source == p.source) {
                Some(u) if newer(u) => {
                    *u = Used {
                        price: p.clone(),
                        inverted,
                    };
                }
                Some(_) => {}
                None => best.push(Used {
                    price: p.clone(),
                    inverted,
                }),
            }
        }
        best
    }

    fn direct(&self, from: CommodityId, to: CommodityId, date: Date) -> Vec<Candidate> {
        self.latest(from, to, date)
            .into_iter()
            .map(|u| Candidate {
                date: u.price.date,
                rank: self.rank(&u.price.source),
                used: vec![u],
            })
            .collect()
    }

    /// Chains `from → pivot → to`: with `one_source`, both prices from the
    /// same source; otherwise each leg's best price, whatever its source.
    fn chains(
        &self,
        from: CommodityId,
        to: CommodityId,
        date: Date,
        one_source: bool,
    ) -> Vec<Candidate> {
        let mut pivots: Vec<CommodityId> = self
            .prices
            .iter()
            .filter_map(|p| match (p.base == from, p.quote == from) {
                (true, _) => Some(p.quote),
                (_, true) => Some(p.base),
                _ => None,
            })
            .filter(|c| *c != to)
            .collect();
        pivots.sort();
        pivots.dedup();
        let fresh = |u: &Used| (date - u.price.date).whole_days() <= self.fresh_days;
        // A leg's best price: fresh first, then by source, then newest.
        let best = |legs: Vec<Used>| {
            legs.into_iter().min_by(|a, b| {
                fresh(b)
                    .cmp(&fresh(a))
                    .then(self.rank(&a.price.source).cmp(&self.rank(&b.price.source)))
                    .then(b.price.date.cmp(&a.price.date))
            })
        };
        let mut out = Vec::new();
        for pivot in pivots {
            let first = self.latest(from, pivot, date);
            let second = self.latest(pivot, to, date);
            let pairs: Vec<(Used, Used)> = if one_source {
                first
                    .into_iter()
                    .filter_map(|a| {
                        let b = second.iter().find(|b| b.price.source == a.price.source)?;
                        Some((a, b.clone()))
                    })
                    .collect()
            } else {
                best(first).zip(best(second)).into_iter().collect()
            };
            for (a, b) in pairs {
                out.push(Candidate {
                    date: a.price.date.min(b.price.date),
                    rank: self.rank(&a.price.source).max(self.rank(&b.price.source)),
                    used: vec![a, b],
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
