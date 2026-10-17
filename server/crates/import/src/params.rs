//! Matching's parameters (design §6.3-§6.4, docs/ai.md): which rows are
//! matches, and how sure matching is of each. Changing how sure it is
//! makes it a new version ([`MATCHER`]).

/// Matching as a suggester: its name and version.
pub const MATCHER: (&str, &str) = ("matching", "1");
/// How far apart, in days, a near duplicate's date may be.
pub const NEAR_DAYS: i32 = 3;

/// Per mille: a new row (nothing like it) and an exact duplicate (the same
/// bank row, imported before) are sure.
pub const SURE: i32 = 1000;
/// The two sides of a transfer: what each day apart takes off.
pub const TRANSFER_DAY: i32 = 30;
/// A transfer matched before the days apart were kept.
pub const UNMEASURED: i32 = 500;
/// A near duplicate: this share of its descriptions' likeness, in percent,
/// less a day's worth for each day apart.
pub const NEAR_LIKENESS: i32 = 89;
pub const NEAR_DAY: i32 = 60;
/// A near duplicate is never sure enough to commit unseen: it may be
/// another, identical purchase. Below [`suggest::CONFIDENT`].
pub const NEAR_MOST: i32 = 890;
const _: () = assert!(NEAR_MOST < suggest::CONFIDENT as i32);
