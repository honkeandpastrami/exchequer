//! Local AI's parameters (docs/ai.md): `lexical-knn-v2`'s neighbors, how
//! their circumstances weigh, and its calibration. Changing one changes
//! what the method suggests: it's then a new version of it.

/// Neighbors that vote.
pub const K: usize = 15;
/// Less similar texts don't count as neighbors.
pub const MIN_SIMILARITY: f32 = 0.3;
/// Added to the votes' total weight: what a lone neighbor can't outweigh.
pub const PRIOR: f32 = 0.5;
/// The latest rows with the same text that alias memory looks at.
pub const ALIAS: usize = 5;
/// How much more a transfer to an account counts when the row has a
/// potential transfer match there.
pub const CANDIDATE: f32 = 2.0;
/// Rows of a statement that vote on its account.
pub const STATEMENT_ROWS: usize = 40;

/// How much a neighbor in another account counts: one of the same owner's,
/// and anyone else's.
pub const SAME_OWNER: f32 = 0.7;
pub const OTHER_ACCOUNT: f32 = 0.5;
/// How much a neighbor whose money went the other way counts.
pub const OPPOSITE_DIRECTION: f32 = 0.15;
/// How much more a neighbor with the same amount counts, and one of the
/// same order of magnitude.
pub const SAME_AMOUNT: f32 = 1.5;
pub const SAME_MAGNITUDE: f32 = 1.15;
/// A neighbor this many days older counts half as much.
pub const RECENCY_DAYS: f32 = 730.0;

/// Calibration: the latest labeled transactions suggested again.
pub const CALIBRATION_ROWS: usize = 2000;
/// Calibration: the latest statements (an account's month) suggested an
/// account, and how many of each one's rows vote.
pub const CALIBRATION_STATEMENTS: usize = 150;
pub const CALIBRATION_STATEMENT_ROWS: usize = 20;
/// Without calibration data, a score's confidence is the score times this,
/// per mille: well below confident.
pub const UNCALIBRATED: f32 = 500.0;
