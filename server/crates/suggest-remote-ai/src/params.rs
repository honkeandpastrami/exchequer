//! Remote AI's parameters (docs/ai.md): how much of the ledger the
//! decision model is shown per question. Changing one changes what it's
//! asked: it's then a new version of the questions ([`crate::QUESTIONS`]).

use time::Duration;

/// Similar past transactions shown per row.
pub const SIMILAR: usize = 10;
/// Counterparty candidates offered per row.
pub const CANDIDATES: usize = 10;
/// A statement's rows shown, spread over it.
pub const STATEMENT_ROWS: usize = 8;
/// What "usual" covers: the year before the ledger's latest transaction.
pub const USUAL: Duration = Duration::days(365);
/// An account's usage: its last six months.
pub const USAGE: Duration = Duration::days(183);
/// Bank texts' length shown.
pub const TEXT: usize = 100;
