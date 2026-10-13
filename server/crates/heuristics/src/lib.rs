//! Local AI (design §8, docs/ai.md): suggestion methods that learn from the
//! ledger's labeled history, in process. Each is a [`suggest::Method`];
//! [`METHODS`] lists them, and configuration picks one by its ID
//! (`<name>-v<version>`), so a new method replaces an old one without
//! touching anything else.
//!
//! Similarity scores and calibration are floats; they never become money or
//! stored data: what's stored is a confidence, per mille.
#![allow(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    clippy::cast_precision_loss,
    reason = "similarity and calibration are fractions; only per-mille integers leave the crate"
)]

pub mod calibrate;
pub mod lexical;
pub mod normalize;

use suggest::Method;

/// Every local method, the default first.
pub static METHODS: [&dyn Method; 1] = [&lexical::LexicalKnn];

/// The method configuration names `id` (`<name>-v<version>`).
#[must_use]
pub fn method(id: &str) -> Option<&'static dyn Method> {
    METHODS.iter().copied().find(|m| m.id() == id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn methods_are_found_by_id() {
        assert_eq!(
            super::method("lexical-knn-v2").unwrap().name(),
            "lexical-knn"
        );
        assert!(super::method("lexical-knn-v1").is_none());
        assert_eq!(super::METHODS[0].id(), "lexical-knn-v2");
    }
}
