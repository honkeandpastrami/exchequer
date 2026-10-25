//! `/api/v1/tokens`: create, list and revoke API tokens (design §10). Every
//! token belongs to a user and expires; `DELETE /tokens` revokes them all.

use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use domain::{TokenId, UserId};
use serde::{Deserialize, Serialize};
use store::audit::{self, Action};
use store::tokens::TokenRow;
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{self, Principal};
use crate::problem::{Json, Path, Problem, is_unique_violation};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create, revoke_all))
        .routes(routes!(revoke))
}

/// The longest time to live a token can be given.
pub(crate) const MAX_TTL_DAYS: u32 = 365;

/// How a token came to be.
#[derive(Serialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    /// A browser signed in with a passkey; signing out revokes it.
    Session,
    /// `exchequer login`.
    Cli,
    /// Made by hand, for a script.
    Script,
}

impl From<store::tokens::Kind> for TokenKind {
    fn from(k: store::tokens::Kind) -> Self {
        match k {
            store::tokens::Kind::Session => Self::Session,
            store::tokens::Kind::Cli => Self::Cli,
            store::tokens::Kind::Script => Self::Script,
        }
    }
}

/// A token, without its secret.
#[derive(Serialize, ToSchema)]
pub struct Token {
    id: Uuid,
    name: String,
    kind: TokenKind,
    /// The user it belongs to; none only for old tokens, all revoked.
    user_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    /// When it stops working, unless revoked first.
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    /// To the minute.
    #[serde(with = "time::serde::rfc3339::option")]
    last_used_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    revoked_at: Option<OffsetDateTime>,
}

impl From<TokenRow> for Token {
    fn from(r: TokenRow) -> Self {
        Self {
            id: *r.id.as_uuid(),
            name: r.name,
            kind: r.kind.into(),
            user_id: r.user_id.map(|u| *u.as_uuid()),
            created_at: r.created_at,
            expires_at: r.expires_at,
            last_used_at: r.last_used_at,
            revoked_at: r.revoked_at,
        }
    }
}

/// A time to live in days, `1..=MAX_TTL_DAYS`.
pub(crate) fn ttl(days: u32) -> Result<Duration, Problem> {
    if !(1..=MAX_TTL_DAYS).contains(&days) {
        return Err(Problem::invalid(format!(
            "ttl_days must be 1-{MAX_TTL_DAYS}"
        )));
    }
    Ok(Duration::days(days.into()))
}

/// A token as the audit log records it.
pub(crate) fn audit_json(r: TokenRow) -> Result<serde_json::Value, Problem> {
    serde_json::to_value(Token::from(r)).map_err(|_| Problem::internal())
}

/// The person behind the request: the user of a session or a CLI's token.
/// A script's token can't make tokens or approve sign-ins, so a leaked one
/// can't make itself successors.
pub(crate) async fn person_of(
    db: impl sqlx::PgExecutor<'_>,
    principal: Principal,
) -> Result<UserId, Problem> {
    let refused = || Problem::forbidden("only a person signed in (a browser or the CLI) can");
    let id = principal.actor().ok_or_else(refused)?;
    let token = store::tokens::find(db, id).await?.ok_or_else(refused)?;
    match token.kind {
        store::tokens::Kind::Session | store::tokens::Kind::Cli => {
            token.user_id.ok_or_else(refused)
        }
        store::tokens::Kind::Script => Err(refused()),
    }
}

/// Makes the user a token and records it in the audit log; the actor is
/// `by`, or the new token itself when a user signs in. Returns it and its full
/// text.
pub(crate) async fn issue(
    tx: &mut sqlx::PgConnection,
    by: Option<Principal>,
    name: &str,
    kind: store::tokens::Kind,
    user: UserId,
    expires_at: OffsetDateTime,
) -> Result<(Token, String), Problem> {
    let generated = auth::generate(auth::Kind::Token)?;
    let id = TokenId::from_uuid(generated.id);
    let row = store::tokens::insert(&mut *tx, id, name, kind, &generated.hash, user, expires_at)
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
            actor: by.map_or(Some(id), Principal::actor),
            action: Action::Create,
            entity: "token",
            entity_id: token.id,
            before: None,
            after: Some(serde_json::to_value(&token).map_err(|_| Problem::internal())?),
        },
    )
    .await?;
    Ok((token, generated.text))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewToken {
    /// What uses the token, e.g. `backup script`. Unique.
    name: String,
    /// How many days it works, 1-365. It can be revoked sooner.
    ttl_days: u32,
}

/// What `DELETE /tokens` did.
#[derive(Serialize, ToSchema)]
pub struct RevokedAll {
    /// How many tokens were still working.
    revoked: usize,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedToken {
    #[serde(flatten)]
    meta: Token,
    /// The full token. Shown only in this response; store it now.
    token: String,
}

/// List every token, revoked and expired ones included.
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

/// Make a token for a script: it belongs to the user signed in (in a browser
/// or the CLI), and works for `ttl_days` unless revoked.
#[utoipa::path(
    post,
    path = "/tokens",
    tag = "tokens",
    security(("bearer" = [])),
    request_body = NewToken,
    responses(
        (status = CREATED, body = CreatedToken),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Not a session or the CLI's token", body = Problem, content_type = "application/problem+json"),
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
    let expires_at = OffsetDateTime::now_utc() + ttl(req.ttl_days)?;
    let mut tx = state.pool.begin().await?;
    let user = person_of(&mut *tx, principal).await?;
    let (token, text) = issue(
        &mut tx,
        Some(principal),
        name,
        store::tokens::Kind::Script,
        user,
        expires_at,
    )
    .await?;
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        axum::Json(CreatedToken {
            meta: token,
            token: text,
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
        audit::record(
            &mut *tx,
            audit::Entry {
                actor: principal.actor(),
                action: Action::Update,
                entity: "token",
                entity_id: *id.as_uuid(),
                before: Some(audit_json(before)?),
                after: Some(audit_json(after)?),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The kill switch: revoke every token, everyone's, this request's too.
/// Everyone is signed out, and the CLI and scripts stop working; passkeys
/// still sign in, and `exchequer login` makes the CLI a new token.
#[utoipa::path(
    delete,
    path = "/tokens",
    tag = "tokens",
    security(("bearer" = [])),
    responses(
        (status = OK, body = RevokedAll),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn revoke_all(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<axum::Json<RevokedAll>, Problem> {
    let mut tx = state.pool.begin().await?;
    let revoked = store::tokens::revoke_all(&mut *tx).await?;
    for after in &revoked {
        let before = TokenRow {
            revoked_at: None,
            ..after.clone()
        };
        audit::record(
            &mut *tx,
            audit::Entry {
                actor: principal.actor(),
                action: Action::Update,
                entity: "token",
                entity_id: *after.id.as_uuid(),
                before: Some(audit_json(before)?),
                after: Some(audit_json(after.clone())?),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(axum::Json(RevokedAll {
        revoked: revoked.len(),
    }))
}
