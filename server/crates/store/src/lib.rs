//! Postgres access for Exchequer. The schema lives in `server/migrations/`
//! and is compiled into the binary.

pub use sqlx::PgPool;

/// Every schema migration, embedded at compile time. Applied once at startup.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

/// Version of the newest migration built into this binary.
#[must_use]
pub fn schema_version() -> i64 {
    MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0)
}

pub mod account_numbers;
pub mod accounts;
pub mod audit;
pub mod balance_checks;
pub mod categories;
pub mod commodities;
pub mod counterparties;
pub mod history;
pub mod idempotency;
pub mod import_batches;
pub mod imports;
pub mod parties;
pub mod prices;
pub mod rules;
pub mod settlements;
pub mod tags;
pub mod tokens;
pub mod transactions;
