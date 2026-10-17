//! The API's suggestion parameters: how much one backtest or dry run may
//! cover. What each level suggests is tuned in its own crate's `params`.

/// Most months one backtest covers.
pub const BACKTEST_MONTHS: usize = 24;
/// Most months one backtest with remote AI covers: each row local AI is
/// unsure of is a request.
pub const BACKTEST_REMOTE_MONTHS: usize = 3;
/// Rows of a rules dry run listed at most.
pub const DRY_RUN_ITEMS: usize = 500;
/// Longest rules dry run, in days.
pub const DRY_RUN_DAYS: i64 = 3 * 366;
