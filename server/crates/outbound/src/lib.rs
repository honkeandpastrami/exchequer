//! Outbound calls (design §15): the only place household data leaves the
//! server, and only while handling a request (design §2).
//!
//! - [`decision`]: a decision model on Azure AI Foundry (remote AI, design
//!   §8, docs/ai.md).

pub mod decision;
