//! Each row's ID for exact duplicate detection (design §6.3), the same one
//! the preimporter gave Firefly as `external_id`: the bank's own ID if the
//! row has one, else the first 15 hex digits of SHA-256 over date,
//! description and amount. A repeat within one file gets `-2`, `-3`…, so
//! identical rows stay distinct and re-reading the file gives the same IDs.

use std::collections::HashMap;

use rust_decimal::{Decimal, RoundingStrategy};
use sha2::{Digest, Sha256};

use crate::Row;

const HASH_LENGTH: usize = 15;

/// The amount as the preimporter wrote it: two decimals, half to even.
fn two_decimals(d: Decimal) -> String {
    let mut d = d.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
    d.rescale(2);
    d.to_string()
}

fn hash(row: &Row) -> String {
    let text = format!(
        "{}{}{}",
        row.date,
        row.description,
        two_decimals(row.amount.as_decimal())
    );
    let digest = Sha256::digest(text.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex[..HASH_LENGTH].to_owned()
}

/// One ID per row, in order.
#[must_use]
pub fn fingerprints(rows: &[Row]) -> Vec<String> {
    let mut seen: HashMap<String, u32> = HashMap::new();
    rows.iter()
        .map(|r| {
            let base = r.native_id.clone().unwrap_or_else(|| hash(r));
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            if *n == 1 { base } else { format!("{base}-{n}") }
        })
        .collect()
}

#[cfg(test)]
mod tests;
