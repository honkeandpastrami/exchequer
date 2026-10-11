//! The append-only audit log: who changed what (design §3.1).

use domain::TokenId;
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Create,
    Update,
    Delete,
}

impl Action {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

/// One change. `actor` is `None` for the bootstrap token. `before`/`after`
/// are the API representations of the entity, never secrets.
pub struct Entry<'a> {
    pub actor: Option<TokenId>,
    pub action: Action,
    pub entity: &'a str,
    pub entity_id: Uuid,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

pub async fn record(db: impl PgExecutor<'_>, e: Entry<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO audit_log (id, token_id, action, entity, entity_id, before, after)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        Uuid::now_v7(),
        e.actor.map(|t| *t.as_uuid()),
        e.action.as_str(),
        e.entity,
        e.entity_id,
        e.before,
        e.after,
    )
    .execute(db)
    .await?;
    Ok(())
}
