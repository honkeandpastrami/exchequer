//! Postgres access for Exchequer. The schema lives in `server/migrations/`
//! and is compiled into the binary.

pub use sqlx::PgPool;

/// Every schema migration, embedded at compile time. Applied once at startup.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
