//! `/api/v1/import-fingerprints` (design §6.3): the IDs of rows already
//! imported into each account, so the same row is never imported twice.
//! Committing an import records them; this endpoint records them for
//! transactions that came in another way, e.g. migrated ones the old tool
//! imported from bank files.

use std::collections::HashSet;

use axum::Extension;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_json::json;
use store::audit::Action;
use store::imports::Fingerprint;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::common::{audit, write_error};
use crate::problem::{Json, Problem};

/// Most items in one request.
const MAX_ITEMS: usize = 1000;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(add_fingerprints))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportFingerprint {
    account_id: Uuid,
    /// The row's ID: the bank's own (OFX `FITID`), or the hash of its date,
    /// description and amount (design §6.3). 1-200 characters.
    fingerprint: String,
    /// The transaction the row became; it must have a split on the account.
    transaction_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewImportFingerprints {
    /// 1-1000 items, each (account, fingerprint) at most once.
    items: Vec<ImportFingerprint>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportFingerprintsAdded {
    /// Recorded now.
    added: u64,
    /// Already recorded for the same transaction.
    unchanged: u64,
    /// Already recorded for another transaction: left as they were.
    conflicts: Vec<ImportFingerprint>,
}

/// Record that rows were already imported. Re-runnable: what's recorded
/// stays as it is, and items recorded for another transaction are returned
/// as conflicts.
#[utoipa::path(post, path = "/import-fingerprints", tag = "imports", security(("bearer" = [])),
    request_body = NewImportFingerprints,
    responses((status = OK, body = ImportFingerprintsAdded),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn add_fingerprints(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewImportFingerprints>,
) -> Result<axum::Json<ImportFingerprintsAdded>, Problem> {
    if req.items.is_empty() || req.items.len() > MAX_ITEMS {
        return Err(Problem::invalid(format!(
            "items must have 1-{MAX_ITEMS} entries"
        )));
    }
    let mut seen = HashSet::new();
    for (i, f) in req.items.iter().enumerate() {
        if f.fingerprint.trim().is_empty() || f.fingerprint.chars().count() > 200 {
            return Err(Problem::invalid(format!(
                "items[{i}]: fingerprint must be 1-200 characters"
            )));
        }
        if !seen.insert((f.account_id, f.fingerprint.as_str())) {
            return Err(Problem::invalid(format!(
                "items[{i}]: the account and fingerprint are listed twice"
            )));
        }
    }
    let items: Vec<Fingerprint> = req
        .items
        .iter()
        .map(|f| Fingerprint {
            account_id: f.account_id,
            fingerprint: f.fingerprint.clone(),
            transaction_id: f.transaction_id,
            raw: None,
        })
        .collect();

    let mut tx = state.pool.begin().await?;
    if let Some(i) = store::imports::fingerprints_off_account(&mut *tx, &items)
        .await?
        .first()
    {
        return Err(Problem::invalid(format!(
            "items[{i}]: no such transaction with a split on the account"
        )));
    }
    let keys: Vec<(Uuid, String)> = items
        .iter()
        .map(|f| (f.account_id, f.fingerprint.clone()))
        .collect();
    let before = store::imports::recorded_fingerprints(&mut *tx, &keys).await?;
    let added = store::imports::add_fingerprints(&mut *tx, &items)
        .await
        .map_err(|e| write_error(e, String::new))?;
    let conflicts: Vec<ImportFingerprint> = before
        .iter()
        .filter(|b| {
            items.iter().any(|f| {
                f.account_id == b.account_id
                    && f.fingerprint == b.fingerprint
                    && f.transaction_id != b.transaction_id
            })
        })
        .map(|b| {
            let wanted = items
                .iter()
                .find(|f| f.account_id == b.account_id && f.fingerprint == b.fingerprint)
                .map_or(b.transaction_id, |f| f.transaction_id);
            ImportFingerprint {
                account_id: b.account_id,
                fingerprint: b.fingerprint.clone(),
                transaction_id: wanted,
            }
        })
        .collect();
    let unchanged = u64::try_from(before.len() - conflicts.len()).unwrap_or(0);
    if added > 0 {
        audit(
            &mut tx,
            principal,
            Action::Create,
            "import_fingerprint",
            Uuid::now_v7(),
            None,
            Some(&json!({ "added": added })),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(axum::Json(ImportFingerprintsAdded {
        added,
        unchanged,
        conflicts,
    }))
}
