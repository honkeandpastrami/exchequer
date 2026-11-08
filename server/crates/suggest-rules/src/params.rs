//! The rules' parameters (docs/ai.md).

/// Longest script accepted, in bytes.
pub const MAX_SOURCE: usize = 200_000;
/// Operations one call may take before it's stopped (a loop).
pub const MAX_OPERATIONS: u64 = 100_000;
/// What a rule sets is a suggestion this sure, per mille: you wrote it.
pub const CONFIDENCE: u16 = 1000;
