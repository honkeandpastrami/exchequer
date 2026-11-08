//! Matching's parameters (design §6.3-§6.4, docs/ai.md): which rows are
//! matches, and how sure matching is of each. Changing how sure it is
//! makes it a new version ([`MATCHER`]).

/// Matching as a suggester: its name and version.
pub const MATCHER: (&str, &str) = ("matching", "2");
/// How far apart, in days, a possible duplicate by amount may be dated.
pub const NEAR_DAYS: i32 = 3;

/// Per mille: a new row (nothing like it) and an exact duplicate (the same
/// bank row, imported before) are sure.
pub const SURE: i32 = 1000;
/// The two sides of a transfer: what each day apart takes off.
pub const TRANSFER_DAY: i32 = 30;
/// A transfer matched before the days apart were kept.
pub const UNMEASURED: i32 = 500;
/// The same bank row, changed since it was imported: very likely the
/// transaction it was, but not sure.
pub const CHANGED: i32 = 890;
/// A transfer pair whose rows you gave another other side: your choice
/// stays the default, short of sure.
pub const DISAGREES: i32 = 500;
/// A possible duplicate by amount: this share of its descriptions' likeness, in percent,
/// less a day's worth for each day apart.
pub const NEAR_LIKENESS: i32 = 89;
pub const NEAR_DAY: i32 = 60;
/// A possible duplicate by amount is never confident: it may be
/// another, identical purchase. Below [`suggest::CONFIDENT`].
pub const NEAR_MOST: i32 = 890;
const _: () = assert!(NEAR_MOST < suggest::CONFIDENT as i32);
