//! The suggestion stack's parameters (docs/ai.md), shared by every level.

/// Confidence, per mille, from which a suggestion counts as made: committed
/// as shown unless you choose otherwise, and not asked of the next level.
pub const CONFIDENT: u16 = 900;

/// How far apart the two sides of a transfer may be dated: money can take a
/// few business days to arrive.
pub const TRANSFER_DAYS: i64 = 5;
