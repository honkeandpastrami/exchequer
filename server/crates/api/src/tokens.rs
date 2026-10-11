//! `/api/v1/tokens`: create, list and revoke API tokens (design §10).

use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use domain::TokenId;
use serde::{Deserialize, Serialize};
use store::audit::{self, Action};
use store::tokens::TokenRow;
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{self, Principal};
use crate::problem::{Json, Path, Problem, is_unique_violation};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(revoke))
}

/// A token, without its secret.
#[derive(Serialize, ToSchema)]
pub struct Token {
    id: Uuid,
    name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    revoked_at: Option<OffsetDateTime>,
}

impl From<TokenRow> for Token {
    fn from(r: TokenRow) -> Self {
        Self {
            id: *r.id.as_uuid(),
            name: r.name,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct NewToken {
    /// Which client uses the token, e.g. `alex-laptop-cli`. Unique.
    name: String,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedToken {
    #[serde(flatten)]
    meta: Token,
    /// The full token. Shown only in this response; store it now.
    token: String,
}

/// List every token, revoked ones included.
#[utoipa::path(
    get,
    path = "/tokens",
    tag = "tokens",
    security(("bearer" = [])),
    responses(
        (status = OK, body = Vec<Token>),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list(State(state): State<AppState>) -> Result<axum::Json<Vec<Token>>, Problem> {
    let rows = store::tokens::list(&state.pool).await?;
    Ok(axum::Json(rows.into_iter().map(Token::from).collect()))
}

/// Create a token for one client.
#[utoipa::path(
    post,
    path = "/tokens",
    tag = "tokens",
    security(("bearer" = [])),
    request_body = NewToken,
    responses(
        (status = CREATED, body = CreatedToken),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Name already used", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewToken>,
) -> Result<(StatusCode, axum::Json<CreatedToken>), Problem> {
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(Problem::invalid("name must be 1-64 characters"));
    }
    let generated = tokio::task::spawn_blocking(auth::generate)
        .await
        .map_err(|_| Problem::internal())??;

    let mut tx = state.pool.begin().await?;
    let row = store::tokens::insert(&mut *tx, generated.id, name, &generated.secret_hash)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                Problem::conflict(format!("a token named {name:?} already exists"))
            } else {
                e.into()
            }
        })?;
    let token = Token::from(row);
    audit::record(
        &mut *tx,
        audit::Entry {
            actor: principal.actor(),
            action: Action::Create,
            entity: "token",
            entity_id: token.id,
            before: None,
            after: Some(serde_json::to_value(&token).map_err(|_| Problem::internal())?),
        },
    )
    .await?;
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        axum::Json(CreatedToken {
            meta: token,
            token: generated.token,
        }),
    ))
}

/// Revoke a token. It stops working immediately; revoking twice is harmless.
#[utoipa::path(
    delete,
    path = "/tokens/{id}",
    tag = "tokens",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Token ID")),
    responses(
        (status = NO_CONTENT),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn revoke(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Problem> {
    let id = TokenId::from_uuid(id);
    let mut tx = state.pool.begin().await?;
    let before = store::tokens::revoke(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    if before.revoked_at.is_none() {
        let after = store::tokens::find(&mut *tx, id)
            .await?
            .ok_or_else(Problem::internal)?;
        let json =
            |r: TokenRow| serde_json::to_value(Token::from(r)).map_err(|_| Problem::internal());
        audit::record(
            &mut *tx,
            audit::Entry {
                actor: principal.actor(),
                action: Action::Update,
                entity: "token",
                entity_id: *id.as_uuid(),
                before: Some(json(before)?),
                after: Some(json(after)?),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
