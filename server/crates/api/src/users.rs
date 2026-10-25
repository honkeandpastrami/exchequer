//! `/api/v1/users`, `/invites`, `/passkeys`, `/me` and `/devices`: who can
//! sign in (design §10). Everyone signed in manages everyone, like the rest
//! of the ledger. An invite is the one way in: it adds the user if need be,
//! and registers a passkey; `exchequer-server invite` makes one on the server.

use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use domain::{PasskeyId, TokenId, UserId};
use serde::{Deserialize, Serialize};
use store::audit::{self, Action};
use store::users::{PasskeyRow, UserRow};
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{self, Principal};
use crate::problem::{Json, Path, Problem, is_unique_violation};
use crate::tokens::{Token, person_of};

/// How long an invite link works.
const INVITE_LIFE: Duration = Duration::hours(3);

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(remove))
        .routes(routes!(create_invite))
        .routes(routes!(remove_passkey))
        .routes(routes!(me))
        .routes(routes!(sign_out))
        .routes(routes!(device))
        .routes(routes!(approve_device))
}

/// A household member who signs in.
#[derive(Serialize, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    pub passkeys: Vec<Passkey>,
}

impl User {
    pub(crate) fn new(row: UserRow, passkeys: &[PasskeyRow]) -> Self {
        Self {
            id: *row.id.as_uuid(),
            name: row.name,
            created_at: row.created_at,
            passkeys: passkeys
                .iter()
                .filter(|p| p.user_id == row.id)
                .map(Passkey::from)
                .collect(),
        }
    }
}

/// A passkey: only what tells it apart, never its key.
#[derive(Serialize, ToSchema)]
pub struct Passkey {
    id: Uuid,
    name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    last_used_at: Option<OffsetDateTime>,
}

impl From<&PasskeyRow> for Passkey {
    fn from(p: &PasskeyRow) -> Self {
        Self {
            id: *p.id.as_uuid(),
            name: p.name.clone(),
            created_at: p.created_at,
            last_used_at: p.last_used_at,
        }
    }
}

/// A one-time link's secret: the web app opens it at `#/invite/<code>`.
#[derive(Serialize, ToSchema)]
pub struct Invite {
    user_id: Uuid,
    /// Shown only in this response.
    pub code: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    /// Using it removes the user's other passkeys and revokes their tokens.
    replace: bool,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewInvite {
    /// Who it's for, e.g. `Alex`: added if there's no such user yet.
    name: String,
    /// Using it removes their other passkeys and revokes their tokens: for
    /// a lost device.
    #[serde(default)]
    replace: bool,
}

/// An invite made: send `invite.code`'s link to `user`.
#[derive(Serialize, ToSchema)]
pub struct Invited {
    pub user: User,
    pub invite: Invite,
    /// Whether the user was added just now.
    pub added: bool,
    /// How many of their tokens still work: a `replace` invite revokes them
    /// when it's used.
    pub tokens: i64,
}

/// Who the request's token belongs to.
#[derive(Serialize, ToSchema)]
pub struct Me {
    user: Option<User>,
    token: Option<Token>,
}

/// The CLI waiting to sign in, as the approver sees it.
#[derive(Serialize, ToSchema)]
pub struct DeviceSignIn {
    /// What the CLI called itself, e.g. `cli on laptop`.
    client: String,
    /// How many days its token will work.
    ttl_days: i32,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

/// A time as the API writes it.
#[derive(Serialize)]
struct Expiry(#[serde(with = "time::serde::rfc3339")] OffsetDateTime);

fn json(value: &impl Serialize) -> Result<serde_json::Value, Problem> {
    serde_json::to_value(value).map_err(|_| Problem::internal())
}

/// A user's name: 1-64 characters.
pub(crate) fn valid_name(name: &str) -> Result<&str, Problem> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(Problem::invalid("name must be 1-64 characters"));
    }
    Ok(name)
}

/// Invites `name`, adding them if there's no such user: the one way in, for
/// `POST /invites` and `exchequer-server invite` alike. `actor` is `None` on
/// the server.
pub async fn invite(
    tx: &mut sqlx::PgConnection,
    actor: Option<TokenId>,
    name: &str,
    replace: bool,
) -> Result<Invited, Problem> {
    let name = valid_name(name)?;
    let (row, added) = match store::users::find_by_name(&mut *tx, name).await? {
        Some(u) if u.removed_at.is_some() => {
            return Err(Problem::conflict(format!(
                "{name:?} was removed, and the name stays taken; choose another"
            )));
        }
        Some(u) => (u, false),
        None => {
            let row = store::users::insert(&mut *tx, UserId::new(), name)
                .await
                .map_err(|e| {
                    if is_unique_violation(&e) {
                        Problem::conflict("someone just added that name; try again")
                    } else {
                        e.into()
                    }
                })?;
            audit::record(
                &mut *tx,
                audit::Entry {
                    actor,
                    action: Action::Create,
                    entity: "user",
                    entity_id: *row.id.as_uuid(),
                    before: None,
                    after: Some(json(&User::new(row.clone(), &[]))?),
                },
            )
            .await?;
            (row, true)
        }
    };
    let secret = auth::generate(auth::Kind::Invite)?;
    let expires_at = OffsetDateTime::now_utc() + INVITE_LIFE;
    store::users::insert_invite(
        &mut *tx,
        secret.id,
        row.id,
        &secret.hash,
        expires_at,
        replace,
    )
    .await?;
    let invite = Invite {
        user_id: *row.id.as_uuid(),
        code: secret.text,
        expires_at,
        replace,
    };
    audit::record(
        &mut *tx,
        audit::Entry {
            actor,
            action: Action::Create,
            entity: "invite",
            entity_id: secret.id,
            before: None,
            // Never the code.
            after: Some(serde_json::json!({
                "user_id": invite.user_id,
                "expires_at": json(&Expiry(expires_at))?,
                "replace": replace,
            })),
        },
    )
    .await?;
    let passkeys = store::users::passkeys(&mut *tx).await?;
    let tokens = store::tokens::working_of(&mut *tx, row.id).await?;
    Ok(Invited {
        user: User::new(row, &passkeys),
        invite,
        added,
        tokens,
    })
}

/// The users who can sign in, with their passkeys.
#[utoipa::path(
    get,
    path = "/users",
    tag = "users",
    security(("bearer" = [])),
    responses(
        (status = OK, body = Vec<User>),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list(State(state): State<AppState>) -> Result<axum::Json<Vec<User>>, Problem> {
    let users = store::users::list(&state.pool).await?;
    let passkeys = store::users::passkeys(&state.pool).await?;
    Ok(axum::Json(
        users.into_iter().map(|u| User::new(u, &passkeys)).collect(),
    ))
}

/// Invite someone: a one-time link, for 3 hours, that registers a passkey.
/// It adds them if there's no such user; for a user already there it adds a
/// passkey on another device, or, with `replace`, replaces lost ones.
#[utoipa::path(
    post,
    path = "/invites",
    tag = "users",
    security(("bearer" = [])),
    request_body = NewInvite,
    responses(
        (status = CREATED, body = Invited),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The name was a removed user's", body = Problem, content_type = "application/problem+json"),
        (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_invite(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<NewInvite>,
) -> Result<(StatusCode, axum::Json<Invited>), Problem> {
    let mut tx = state.pool.begin().await?;
    let invited = invite(&mut tx, principal.actor(), &req.name, req.replace).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, axum::Json(invited)))
}

/// Remove a user: their passkeys are deleted and their tokens revoked.
#[utoipa::path(
    delete,
    path = "/users/{id}",
    tag = "users",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "User ID")),
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
    let id = UserId::from_uuid(id);
    let mut tx = state.pool.begin().await?;
    let passkeys = store::users::passkeys(&mut *tx).await?;
    let before = store::users::remove(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    store::users::delete_passkeys_of(&mut *tx, id, None).await?;
    store::tokens::revoke_all_of(&mut *tx, id).await?;
    audit::record(
        &mut *tx,
        audit::Entry {
            actor: principal.actor(),
            action: Action::Delete,
            entity: "user",
            entity_id: *id.as_uuid(),
            before: Some(json(&User::new(before, &passkeys))?),
            after: None,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Delete a passkey; it stops working at once.
#[utoipa::path(
    delete,
    path = "/passkeys/{id}",
    tag = "users",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Passkey ID")),
    responses(
        (status = NO_CONTENT),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn remove_passkey(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Problem> {
    let mut tx = state.pool.begin().await?;
    let before = store::users::delete_passkey(&mut *tx, PasskeyId::from_uuid(id))
        .await?
        .ok_or_else(Problem::not_found)?;
    audit::record(
        &mut *tx,
        audit::Entry {
            actor: principal.actor(),
            action: Action::Delete,
            entity: "passkey",
            entity_id: id,
            before: Some(json(&serde_json::json!({
                "user_id": before.user_id.as_uuid(),
                "name": before.name,
            }))?),
            after: None,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The signed-in user and their token.
#[utoipa::path(
    get,
    path = "/me",
    tag = "users",
    security(("bearer" = [])),
    responses(
        (status = OK, body = Me),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn me(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<axum::Json<Me>, Problem> {
    let token = match principal.actor() {
        Some(id) => store::tokens::find(&state.pool, id).await?,
        None => None,
    };
    let user = match token.as_ref().and_then(|t| t.user_id) {
        Some(id) => {
            let passkeys = store::users::passkeys(&state.pool).await?;
            store::users::find(&state.pool, id)
                .await?
                .map(|u| User::new(u, &passkeys))
        }
        None => None,
    };
    Ok(axum::Json(Me {
        user,
        token: token.map(Token::from),
    }))
}

/// Sign out: revokes the request's token if it's a session. Other tokens are
/// left alone (revoke those through `/tokens`).
#[utoipa::path(
    delete,
    path = "/me/session",
    tag = "users",
    security(("bearer" = [])),
    responses(
        (status = NO_CONTENT),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn sign_out(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<StatusCode, Problem> {
    let Some(id) = principal.actor() else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let mut tx = state.pool.begin().await?;
    if let Some(before) = store::tokens::find(&mut *tx, id).await?
        && before.kind == store::tokens::Kind::Session
        && before.revoked_at.is_none()
    {
        store::tokens::revoke(&mut *tx, id).await?;
        let after = store::tokens::find(&mut *tx, id)
            .await?
            .ok_or_else(Problem::internal)?;
        audit::record(
            &mut *tx,
            audit::Entry {
                actor: Some(id),
                action: Action::Update,
                entity: "token",
                entity_id: *id.as_uuid(),
                before: Some(json(&Token::from(before))?),
                after: Some(json(&Token::from(after))?),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The CLI waiting for approval under this code (`ABCD-EFGH`).
#[utoipa::path(
    get,
    path = "/devices/{code}",
    tag = "users",
    security(("bearer" = [])),
    params(("code" = String, Path, description = "The code the CLI shows")),
    responses(
        (status = OK, body = DeviceSignIn),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such code, or it expired", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn device(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<axum::Json<DeviceSignIn>, Problem> {
    let code = crate::sign_in::user_code(&code).ok_or_else(Problem::not_found)?;
    let d = store::sign_in::pending_device(&state.pool, &code)
        .await?
        .ok_or_else(Problem::not_found)?;
    Ok(axum::Json(DeviceSignIn {
        client: d.client,
        ttl_days: d.ttl_days,
        expires_at: d.expires_at,
    }))
}

/// Approve the CLI waiting under this code: it signs in as you.
#[utoipa::path(
    post,
    path = "/devices/{code}/approve",
    tag = "users",
    security(("bearer" = [])),
    params(("code" = String, Path, description = "The code the CLI shows")),
    responses(
        (status = NO_CONTENT),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Not a session or the CLI's token", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such code, or it expired", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn approve_device(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(code): Path<String>,
) -> Result<StatusCode, Problem> {
    let code = crate::sign_in::user_code(&code).ok_or_else(Problem::not_found)?;
    let user = person_of(&state.pool, principal).await?;
    if !store::sign_in::approve_device(&state.pool, &code, user).await? {
        return Err(Problem::not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}
