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
mod tests {
    use super::*;
    use time::macros::date;

    fn row(amount: &str, description: &str, native: Option<&str>) -> Row {
        Row {
            date: date!(2026 - 03 - 02),
            amount: amount.parse().unwrap(),
            description: description.to_owned(),
            native_id: native.map(str::to_owned),
            raw: serde_json::Map::new(),
        }
    }

    #[test]
    fn same_ids_as_the_preimporter() {
        // hashlib.sha256(b"2026-03-02GROCER #12-80.00").hexdigest()[:15]
        let expected = {
            let d = Sha256::digest(b"2026-03-02GROCER #12-80.00");
            d.iter().map(|b| format!("{b:02x}")).collect::<String>()[..15].to_owned()
        };
        let rows = [
            row("-80", "GROCER #12", None),
            row("-80.00", "GROCER #12", None),
            row("-80.001", "OTHER", None),
            row("5", "X", Some("FIT-1")),
            row("5", "X", Some("FIT-1")),
        ];
        let ids = fingerprints(&rows);
        assert_eq!(ids[0], expected);
        assert_eq!(ids[1], format!("{expected}-2"));
        assert_eq!(ids[2].len(), 15);
        assert_eq!(&ids[3..], ["FIT-1", "FIT-1-2"]);
        assert_eq!(two_decimals("-80.005".parse().unwrap()), "-80.00");
        assert_eq!(two_decimals("2.675".parse().unwrap()), "2.68");
    }
}
