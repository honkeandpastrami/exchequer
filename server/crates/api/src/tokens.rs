//! `/api/v1/tokens`: create, list and revoke scripts' tokens (design §10).
//! Browsers and the CLI sign in with tokens too, but they're managed as
//! sessions (`/sessions`). Every token belongs to a user and expires;
//! `POST /revoke-all` revokes them all.

use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use domain::{PasskeyId, TokenId, UserId};
use serde::{Deserialize, Serialize};
use store::audit::{self, Action};
use store::tokens::{Client, Kind, TokenRow};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{self, Principal};
use crate::problem::{Json, Path, Problem, is_unique_violation};
use crate::sessions;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(remove))
        .routes(routes!(revoke_all))
}

/// The longest time to live a token can be given.
pub(crate) const MAX_TTL_DAYS: u32 = 365;

/// A script's token, without its secret.
#[derive(Serialize, ToSchema)]
pub struct Token {
    id: Uuid,
    name: String,
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
            name: r.name.unwrap_or_default(),
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

/// A token as the audit log records it: any kind, never its secret.
pub(crate) fn audit_json(r: &TokenRow) -> serde_json::Value {
    let at = |t: OffsetDateTime| t.format(&Rfc3339).ok();
    serde_json::json!({
        "id": r.id.as_uuid(),
        "kind": r.kind.as_str(),
        "name": r.name,
        "user_id": r.user_id.map(|u| *u.as_uuid()),
        "passkey_id": r.passkey_id.map(|p| *p.as_uuid()),
        "created_at": at(r.created_at),
        "expires_at": at(r.expires_at),
        "revoked_at": r.revoked_at.and_then(at),
    })
}

/// The person behind the request: the user of a browser's or the CLI's
/// token, and the passkey it was signed in with. A script's token can't
/// make tokens or approve sign-ins, so a leaked one can't make itself
/// successors.
pub(crate) async fn person_of(
    db: impl sqlx::PgExecutor<'_>,
    principal: Principal,
) -> Result<(UserId, Option<PasskeyId>), Problem> {
    let refused = || Problem::forbidden("only a person signed in (a browser or the CLI) can");
    let id = principal.actor().ok_or_else(refused)?;
    let token = store::tokens::find(db, id).await?.ok_or_else(refused)?;
    match token.kind {
        Kind::Browser | Kind::Cli => Ok((token.user_id.ok_or_else(refused)?, token.passkey_id)),
        Kind::Script => Err(refused()),
    }
}

/// Makes the user a token and records it in the audit log; the actor is
/// `by`, or the new token itself when a user signs in. A person's token's
/// session starts with `client`. Returns it and its full text.
pub(crate) async fn issue(
    tx: &mut sqlx::PgConnection,
    by: Option<Principal>,
    new: store::tokens::NewToken<'_>,
    user: UserId,
    expires_at: OffsetDateTime,
    client: &Client,
) -> Result<(TokenRow, String), Problem> {
    let generated = auth::generate(auth::Kind::Token)?;
    let id = TokenId::from_uuid(generated.id);
    let row = store::tokens::insert(&mut *tx, id, new, &generated.hash, user, expires_at)
        .await
        .map_err(|e| match new {
            store::tokens::NewToken::Script(name) if is_unique_violation(&e) => {
                Problem::conflict(format!("a token named {name:?} already exists"))
            }
            _ => e.into(),
        })?;
    if row.kind != Kind::Script {
        sessions::start(&mut *tx, id, row.kind, client).await?;
    }
    audit::record(
        &mut *tx,
        audit::Entry {
            actor: by.map_or(Some(id), Principal::actor),
            action: Action::Create,
            entity: "token",
            entity_id: *id.as_uuid(),
            before: None,
            after: Some(audit_json(&row)),
        },
    )
    .await?;
    Ok((row, generated.text))
}

/// Revokes the token, recording it in the audit log unless it was revoked
/// already. Returns it as it was, or `None` if there's no such token.
pub(crate) async fn revoke(
    tx: &mut sqlx::PgConnection,
    by: Principal,
    id: TokenId,
) -> Result<Option<TokenRow>, Problem> {
    let Some(before) = store::tokens::revoke(&mut *tx, id).await? else {
        return Ok(None);
    };
    if before.revoked_at.is_none() {
        let after = store::tokens::find(&mut *tx, id)
            .await?
            .ok_or_else(Problem::internal)?;
        audit::record(
            &mut *tx,
            audit::Entry {
                actor: by.actor(),
                action: Action::Update,
                entity: "token",
                entity_id: *id.as_uuid(),
                before: Some(audit_json(&before)),
                after: Some(audit_json(&after)),
            },
        )
        .await?;
    }
    Ok(Some(before))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewToken {
    /// What uses the token, e.g. `backup script`. Unique.
    name: String,
    /// How many days it works, 1-365. It can be revoked sooner.
    ttl_days: u32,
}

/// What `POST /revoke-all` did.
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

/// Every script's token, revoked and expired ones included. (Browsers and
/// the CLI are signed in with tokens too: see `/sessions`.)
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
    let rows = store::tokens::scripts(&state.pool).await?;
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
        (status = FORBIDDEN, description = "Not a browser's or the CLI's token", body = Problem, content_type = "application/problem+json"),
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
    let (user, _) = person_of(&mut *tx, principal).await?;
    let (row, text) = issue(
        &mut tx,
        Some(principal),
        store::tokens::NewToken::Script(name),
        user,
        expires_at,
        &Client::default(),
    )
    .await?;
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        axum::Json(CreatedToken {
            meta: row.into(),
            token: text,
        }),
    ))
}

/// Revoke a script's token. It stops working immediately, and its sessions
/// end; revoking twice is harmless.
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
async fn remove(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Problem> {
    let id = TokenId::from_uuid(id);
    let mut tx = state.pool.begin().await?;
    match store::tokens::find(&mut *tx, id).await? {
        Some(t) if t.kind == Kind::Script => {}
        _ => return Err(Problem::not_found()),
    }
    revoke(&mut tx, principal, id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The kill switch: sign everyone out and revoke every token, this
/// request's too. Browsers, the CLI and scripts all stop working; passkeys
/// still sign in, and `exchequer login` signs the CLI in again.
#[utoipa::path(
    post,
    path = "/revoke-all",
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
                before: Some(audit_json(&before)),
                after: Some(audit_json(after)),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(axum::Json(RevokedAll {
        revoked: revoked.len(),
    }))
}
