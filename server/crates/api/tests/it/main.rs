//! Every API integration test, as one binary: one link of `api` instead of
//! one per file, and all the tests run in parallel. Needs `just db-up`.

mod common;

mod account_numbers;
mod auth;
mod balance_checks;
mod import_batches;
mod import_commit;
mod import_suggestions;
mod imports;
mod operations;
mod parties;
mod passkeys;
mod prices;
mod reference;
mod remote_ai;
mod reports;
mod rules;
mod settlements;
mod transactions;
mod transfers;
mod web;
