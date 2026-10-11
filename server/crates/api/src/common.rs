//! Helpers every resource handler uses.

use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgConnection;
use store::audit::{self, Action};
use uuid::Uuid;

use crate::auth::Principal;
use crate::problem::Problem;

/// Records a change in the audit log, using the API representation of the
/// entity before and after.
pub async fn audit<T: Serialize>(
    db: &mut PgConnection,
    principal: Principal,
    action: Action,
    entity: &str,
    entity_id: Uuid,
    before: Option<&T>,
    after: Option<&T>,
) -> Result<(), Problem> {
    let json = |v: Option<&T>| {
        v.map(serde_json::to_value)
            .transpose()
            .map_err(|_| Problem::internal())
    };
    audit::record(
        db,
        audit::Entry {
            actor: principal.actor(),
            action,
            entity,
            entity_id,
            before: json(before)?,
            after: json(after)?,
        },
    )
    .await?;
    Ok(())
}

/// Parses an ID cursor (lists ordered by ID).
pub fn id_cursor(cursor: Option<&str>) -> Result<Option<Uuid>, Problem> {
    cursor
        .map(|c| Uuid::try_parse(c).map_err(|_| Problem::invalid("invalid cursor")))
        .transpose()
}

const UNIQUE_VIOLATION: &str = "23505";
const FOREIGN_KEY_VIOLATION: &str = "23503";
const CHECK_VIOLATION: &str = "23514";

fn code(err: &sqlx::Error) -> Option<String> {
    err.as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.into_owned())
}

/// Maps errors from an insert or update: duplicates are a conflict, unknown
/// references and broken constraints are invalid requests.
pub fn write_error(err: sqlx::Error, duplicate: impl FnOnce() -> String) -> Problem {
    match code(&err).as_deref() {
        Some(UNIQUE_VIOLATION) => Problem::conflict(duplicate()),
        Some(FOREIGN_KEY_VIOLATION) => Problem::invalid("refers to something that doesn't exist"),
        Some(CHECK_VIOLATION) => Problem::invalid(format!(
            "violates the ledger rule {}",
            err.as_database_error()
                .and_then(|e| e.constraint())
                .unwrap_or("?")
        )),
        _ => err.into(),
    }
}

/// Maps errors from a delete: still referenced is a conflict.
pub fn delete_error(err: sqlx::Error) -> Problem {
    if code(&err).as_deref() == Some(FOREIGN_KEY_VIOLATION) {
        Problem::conflict("still in use; remove what refers to it first")
    } else {
        err.into()
    }
}

/// Trims a name and checks its length.
pub fn valid_name(name: &str) -> Result<&str, Problem> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(Problem::invalid("name must be 1-100 characters"));
    }
    Ok(name)
}

/// For PATCH fields that can be cleared: absent stays `None` (with
/// `#[serde(default)]`), `null` becomes `Some(None)`.
pub fn nullable<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}
