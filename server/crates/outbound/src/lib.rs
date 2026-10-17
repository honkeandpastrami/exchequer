//! Outbound calls (design §15): the only place household data leaves the
//! server, and only while handling a request (design §2).
//!
//! - [`decision`]: a decision model on Azure AI Foundry (remote AI, design
//!   §8, docs/ai.md).
//! - [`prices`]: price sources (design §3.4): Tiingo and the ECB's
//!   reference rates. Nothing about the household is sent: only commodity
//!   codes and dates.

pub mod decision;
pub mod prices;

#[cfg(test)]
mod test_server;
