//! `/api/v1/auth`: getting a token without one (design §10). Registering a
//! passkey from an invite, signing in with one, and the CLI's device sign-in.
//!
//! Each passkey ceremony is two calls: the first returns the options for the
//! browser's `navigator.credentials` and keeps the challenge in the database
//! for five minutes; the second, given the browser's answer, takes it out
//! (so it's used once), checks the answer and starts a session.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use store::audit::{self, Action};
use store::sign_in::Kind;
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use webauthn_rs::prelude::{
    DiscoverableAuthentication, DiscoverableKey, Passkey as Credential, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, Url, Webauthn, WebauthnBuilder,
};

use crate::AppState;
use crate::auth;
use crate::problem::{Json, Path, Problem, is_unique_violation};
use crate::tokens;
use crate::users::{User, valid_name};

/// How long a ceremony's challenge lasts.
const CEREMONY_LIFE: Duration = Duration::minutes(5);
/// How long a browser stays signed in.
const SESSION_LIFE: Duration = Duration::days(30);
/// How long the CLI waits for approval, and how often it asks.
const DEVICE_LIFE: Duration = Duration::minutes(10);
const DEVICE_INTERVAL_SECS: u32 = 2;
/// User codes avoid vowels (no words) and look-alikes.
const CODE_LETTERS: &[u8; 20] = b"BCDFGHJKLMNPQRSTVWXZ";

/// The relying party for `origin` (`https://host[:port]`, any path ignored):
/// passkeys are bound to its host.
pub fn passkeys(origin: &str) -> Result<Arc<Webauthn>, String> {
    let url = Url::parse(origin).map_err(|e| format!("{origin:?}: {e}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| format!("{origin:?} has no host"))?
        .to_owned();
    // Browsers offer passkeys only to secure origins: HTTPS, or this machine.
    if url.scheme() != "https" && !(url.scheme() == "http" && host == "localhost") {
        return Err(format!("{origin:?} must be https:// (or http://localhost)"));
    }
    let origin = Url::parse(&url.origin().ascii_serialization()).map_err(|e| e.to_string())?;
    WebauthnBuilder::new(&host, &origin)
        .and_then(|b| b.rp_name("Exchequer").build())
        .map(Arc::new)
        .map_err(|e| format!("{origin}: {e}"))
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(info))
        .routes(routes!(start_register))
        .routes(routes!(finish_register))
        .routes(routes!(start_sign_in))
        .routes(routes!(finish_sign_in))
        .routes(routes!(start_device))
        .routes(routes!(device_token))
}

/// What this server offers.
#[derive(Serialize, ToSchema)]
pub struct AuthInfo {
    /// Whether passkey sign-in is on (`EXCHEQUER_ORIGIN` is set).
    passkeys: bool,
}

/// A ceremony started: hand `options` to the browser.
#[derive(Serialize, ToSchema)]
pub struct Ceremony {
    id: Uuid,
    /// Whose passkey a registration makes; none when signing in.
    user: Option<String>,
    /// For `navigator.credentials.create` or `.get`, binary fields as
    /// base64url: `{ "publicKey": … }`.
    options: serde_json::Value,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartRegistration {
    /// The invite's code.
    invite: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FinishRegistration {
    /// The browser's `PublicKeyCredential`, as JSON (binary fields base64url).
    credential: serde_json::Value,
    /// What to call the passkey, e.g. `iPhone`.
    name: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FinishSignIn {
    /// The browser's `PublicKeyCredential`, as JSON (binary fields base64url).
    credential: serde_json::Value,
}

/// A browser signed in: send `token` as the bearer token until it expires.
#[derive(Serialize, ToSchema)]
pub struct Session {
    /// Shown only in this response.
    token: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    user: User,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartDevice {
    /// What the CLI calls itself, shown to the approver: `cli on laptop`.
    client: String,
    /// How many days its token works, 1-365; shown to the approver.
    ttl_days: u32,
}

/// Show `user_code` to the user; poll with `device_code` until approved.
#[derive(Serialize, ToSchema)]
pub struct DeviceCode {
    /// The CLI's secret. Shown only in this response.
    device_code: String,
    /// What the user approves in the web app, at `#/device/<user_code>`.
    user_code: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    /// Seconds between polls.
    interval: u32,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PollDevice {
    device_code: String,
}

/// The CLI's token once approved; none while waiting.
#[derive(Serialize, ToSchema)]
pub struct DeviceToken {
    /// Shown only once: store it in the token file.
    token: Option<String>,
}

fn webauthn(state: &AppState) -> Result<&Webauthn, Problem> {
    state.passkeys.as_deref().ok_or_else(|| {
        Problem::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "passkeys-off",
            "Passkey sign-in is off",
        )
        .detail("the server has no EXCHEQUER_ORIGIN; sign in with a token")
    })
}

fn rejected() -> Problem {
    Problem::unauthorized().detail("that passkey wasn't accepted; start again")
}

fn to_json(v: &impl Serialize) -> Result<serde_json::Value, Problem> {
    serde_json::to_value(v).map_err(|_| Problem::internal())
}

/// `ABCD-EFGH` from whatever the user typed (`abcd efgh`), if it can be one.
pub(crate) fn user_code(typed: &str) -> Option<String> {
    let letters: String = typed
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (letters.len() == 8 && letters.bytes().all(|b| CODE_LETTERS.contains(&b)))
        .then(|| format!("{}-{}", &letters[..4], &letters[4..]))
}

fn new_user_code() -> Result<String, Problem> {
    let letters: String = auth::random::<8>()?
        .iter()
        // 256 is a multiple of 20 plus 16: the first 16 letters are a hair
        // likelier, which costs well under a bit of the code's 34.
        .map(|b| char::from(CODE_LETTERS[usize::from(*b) % CODE_LETTERS.len()]))
        .collect();
    Ok(format!("{}-{}", &letters[..4], &letters[4..]))
}

/// Starts a session for the user: a token that expires.
async fn session(
    tx: &mut sqlx::PgConnection,
    user: store::users::UserRow,
) -> Result<Session, Problem> {
    let expires_at = OffsetDateTime::now_utc() + SESSION_LIFE;
    let name = format!("{} · web · {}", user.name, auth::hex(&auth::random::<3>()?));
    let (_, token) = tokens::issue(
        tx,
        None,
        &name,
        store::tokens::Kind::Session,
        user.id,
        expires_at,
    )
    .await?;
    let passkeys = store::users::passkeys(&mut *tx).await?;
    Ok(Session {
        token,
        expires_at,
        user: User::new(user, &passkeys),
    })
}

/// What this server offers for signing in.
#[utoipa::path(
    get,
    path = "/auth",
    tag = "auth",
    responses((status = OK, body = AuthInfo)),
)]
async fn info(State(state): State<AppState>) -> axum::Json<AuthInfo> {
    axum::Json(AuthInfo {
        passkeys: state.passkeys.is_some(),
    })
}

/// Start registering a passkey with an invite.
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    request_body = StartRegistration,
    responses(
        (status = OK, body = Ceremony),
        (status = NOT_FOUND, description = "No such invite, or it was used or expired", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Passkey sign-in is off", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn start_register(
    State(state): State<AppState>,
    Json(req): Json<StartRegistration>,
) -> Result<axum::Json<Ceremony>, Problem> {
    let webauthn = webauthn(&state)?;
    let gone = || Problem::not_found().detail("this invite was used, expired or doesn't exist");
    let mut tx = state.pool.begin().await?;
    store::sign_in::prune(&mut tx).await?;
    let id = auth::id_of(auth::Kind::Invite, &req.invite).ok_or_else(gone)?;
    let invite = store::users::open_invite(&mut *tx, id)
        .await?
        .ok_or_else(gone)?;
    if !auth::check(auth::Kind::Invite, &req.invite, |_| {
        Some(invite.secret_hash.clone())
    }) {
        return Err(gone());
    }
    let user = store::users::find(&mut *tx, invite.user_id)
        .await?
        .ok_or_else(gone)?;
    let existing = store::users::credential_ids(&mut *tx, user.id).await?;
    let (options, registration) = webauthn
        .start_passkey_registration(
            *user.id.as_uuid(),
            &user.name,
            &user.name,
            Some(existing.into_iter().map(Into::into).collect()),
        )
        .map_err(|_| Problem::internal())?;
    // A discoverable credential, so signing in needs no user name. Platform
    // passkeys are discoverable anyway; this asks security keys for it too.
    let mut options = to_json(&options)?;
    let selection = &mut options["publicKey"]["authenticatorSelection"];
    selection["residentKey"] = "required".into();
    selection["requireResidentKey"] = true.into();
    let ceremony = Uuid::now_v7();
    let expires_at = OffsetDateTime::now_utc() + CEREMONY_LIFE;
    store::sign_in::start(
        &mut *tx,
        ceremony,
        Kind::Register,
        Some(invite.id),
        &to_json(&registration)?,
        expires_at,
    )
    .await?;
    tx.commit().await?;
    Ok(axum::Json(Ceremony {
        id: ceremony,
        user: Some(user.name),
        options,
    }))
}

/// Finish registering: the passkey is kept, the invite used up, and the
/// browser signed in as its user.
#[utoipa::path(
    post,
    path = "/auth/register/{id}",
    tag = "auth",
    params(("id" = Uuid, Path, description = "Ceremony ID")),
    request_body = FinishRegistration,
    responses(
        (status = OK, body = Session),
        (status = UNAUTHORIZED, description = "The passkey wasn't accepted, or the ceremony expired", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "That passkey is registered already", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Passkey sign-in is off", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn finish_register(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<FinishRegistration>,
) -> Result<axum::Json<Session>, Problem> {
    let webauthn = webauthn(&state)?;
    let mut tx = state.pool.begin().await?;
    let (Some(invite), registration) = store::sign_in::take(&mut *tx, id, Kind::Register)
        .await?
        .ok_or_else(rejected)?
    else {
        return Err(Problem::internal());
    };
    // Taken out: whatever happens next, this ceremony is over.
    tx.commit().await?;

    let registration: PasskeyRegistration =
        serde_json::from_value(registration).map_err(|_| Problem::internal())?;
    let response: RegisterPublicKeyCredential =
        serde_json::from_value(req.credential).map_err(|_| rejected())?;
    let credential = webauthn
        .finish_passkey_registration(&response, &registration)
        .map_err(|_| rejected())?;

    let mut tx = state.pool.begin().await?;
    let open = store::users::open_invite(&mut *tx, invite)
        .await?
        .ok_or_else(rejected)?;
    if !store::users::use_invite(&mut *tx, invite).await? {
        return Err(rejected());
    }
    let user = store::users::find(&mut *tx, open.user_id)
        .await?
        .ok_or_else(rejected)?;
    let name = match req.name.as_deref().map(str::trim) {
        Some(n) if !n.is_empty() => valid_name(n)?.to_owned(),
        _ => format!("Passkey of {}", OffsetDateTime::now_utc().date()),
    };
    let passkey = store::users::insert_passkey(
        &mut *tx,
        domain::PasskeyId::new(),
        user.id,
        credential.cred_id().as_ref(),
        &name,
        &to_json(&credential)?,
    )
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            Problem::conflict("that passkey is registered already")
        } else {
            e.into()
        }
    })?;
    // Replacing lost passkeys: the others go, and whatever was signed in
    // with them, before this passkey's session starts.
    let (removed, revoked) = if open.replace {
        (
            store::users::delete_passkeys_of(&mut *tx, user.id, Some(passkey.id)).await?,
            store::tokens::revoke_all_of(&mut *tx, user.id).await?,
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let session = session(&mut tx, user).await?;
    let token = auth::id_of(auth::Kind::Token, &session.token).map(domain::TokenId::from_uuid);
    for p in removed {
        audit::record(
            &mut *tx,
            audit::Entry {
                actor: token,
                action: Action::Delete,
                entity: "passkey",
                entity_id: *p.id.as_uuid(),
                before: Some(serde_json::json!({
                    "user_id": p.user_id.as_uuid(),
                    "name": p.name,
                })),
                after: None,
            },
        )
        .await?;
    }
    for after in revoked {
        let before = store::tokens::TokenRow {
            revoked_at: None,
            ..after.clone()
        };
        audit::record(
            &mut *tx,
            audit::Entry {
                actor: token,
                action: Action::Update,
                entity: "token",
                entity_id: *after.id.as_uuid(),
                before: Some(tokens::audit_json(before)?),
                after: Some(tokens::audit_json(after)?),
            },
        )
        .await?;
    }
    audit::record(
        &mut *tx,
        audit::Entry {
            actor: token,
            action: Action::Create,
            entity: "passkey",
            entity_id: *passkey.id.as_uuid(),
            before: None,
            after: Some(serde_json::json!({
                "user_id": passkey.user_id.as_uuid(),
                "name": passkey.name,
            })),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(axum::Json(session))
}

/// Start signing in: the browser offers the passkeys it holds for this
/// server.
#[utoipa::path(
    post,
    path = "/auth/sign-in",
    tag = "auth",
    responses(
        (status = OK, body = Ceremony),
        (status = SERVICE_UNAVAILABLE, description = "Passkey sign-in is off", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn start_sign_in(State(state): State<AppState>) -> Result<axum::Json<Ceremony>, Problem> {
    let webauthn = webauthn(&state)?;
    let (mut options, authentication) = webauthn
        .start_discoverable_authentication()
        .map_err(|_| Problem::internal())?;
    // Started by a button, so the browser's usual dialog, not autofill.
    options.mediation = None;
    let mut tx = state.pool.begin().await?;
    store::sign_in::prune(&mut tx).await?;
    let id = Uuid::now_v7();
    store::sign_in::start(
        &mut *tx,
        id,
        Kind::SignIn,
        None,
        &to_json(&authentication)?,
        OffsetDateTime::now_utc() + CEREMONY_LIFE,
    )
    .await?;
    tx.commit().await?;
    Ok(axum::Json(Ceremony {
        id,
        user: None,
        options: to_json(&options)?,
    }))
}

/// Finish signing in with the browser's answer.
#[utoipa::path(
    post,
    path = "/auth/sign-in/{id}",
    tag = "auth",
    params(("id" = Uuid, Path, description = "Ceremony ID")),
    request_body = FinishSignIn,
    responses(
        (status = OK, body = Session),
        (status = UNAUTHORIZED, description = "The passkey wasn't accepted, or the ceremony expired", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Passkey sign-in is off", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn finish_sign_in(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<FinishSignIn>,
) -> Result<axum::Json<Session>, Problem> {
    let webauthn = webauthn(&state)?;
    let mut tx = state.pool.begin().await?;
    let (_, authentication) = store::sign_in::take(&mut *tx, id, Kind::SignIn)
        .await?
        .ok_or_else(rejected)?;
    tx.commit().await?;

    let authentication: DiscoverableAuthentication =
        serde_json::from_value(authentication).map_err(|_| Problem::internal())?;
    let response: PublicKeyCredential =
        serde_json::from_value(req.credential).map_err(|_| rejected())?;
    let (user_id, credential_id) = webauthn
        .identify_discoverable_authentication(&response)
        .map_err(|_| rejected())?;

    let mut tx = state.pool.begin().await?;
    let row = store::users::passkey_by_credential(&mut *tx, credential_id)
        .await?
        .filter(|p| *p.user_id.as_uuid() == user_id)
        .ok_or_else(rejected)?;
    let mut credential: Credential =
        serde_json::from_value(row.credential).map_err(|_| Problem::internal())?;
    let result = webauthn
        .finish_discoverable_authentication(
            &response,
            authentication,
            &[DiscoverableKey::from(&credential)],
        )
        .map_err(|_| rejected())?;
    credential.update_credential(&result);
    store::users::passkey_used(&mut *tx, row.id, &to_json(&credential)?).await?;
    let user = store::users::find(&mut *tx, row.user_id)
        .await?
        .filter(|u| u.removed_at.is_none())
        .ok_or_else(rejected)?;
    let session = session(&mut tx, user).await?;
    tx.commit().await?;
    Ok(axum::Json(session))
}

/// The CLI starts signing in: it shows `user_code` and a link, and polls.
#[utoipa::path(
    post,
    path = "/auth/device",
    tag = "auth",
    request_body = StartDevice,
    responses(
        (status = OK, body = DeviceCode),
        (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn start_device(
    State(state): State<AppState>,
    Json(req): Json<StartDevice>,
) -> Result<axum::Json<DeviceCode>, Problem> {
    let client = valid_name(&req.client)?;
    tokens::ttl(req.ttl_days)?;
    let ttl_days = i32::try_from(req.ttl_days).map_err(|_| Problem::internal())?;
    let mut tx = state.pool.begin().await?;
    store::sign_in::prune(&mut tx).await?;
    let secret = auth::generate(auth::Kind::Device)?;
    let expires_at = OffsetDateTime::now_utc() + DEVICE_LIFE;
    let user_code = new_user_code()?;
    store::sign_in::start_device(
        &mut *tx,
        secret.id,
        &secret.hash,
        &user_code,
        client,
        ttl_days,
        expires_at,
    )
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            Problem::conflict("try again")
        } else {
            e.into()
        }
    })?;
    tx.commit().await?;
    Ok(axum::Json(DeviceCode {
        device_code: secret.text,
        user_code,
        expires_at,
        interval: DEVICE_INTERVAL_SECS,
    }))
}

/// The CLI polls: its token once a user has approved, handed out once.
#[utoipa::path(
    post,
    path = "/auth/device/token",
    tag = "auth",
    request_body = PollDevice,
    responses(
        (status = OK, body = DeviceToken),
        (status = NOT_FOUND, description = "No such device code, or it expired", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn device_token(
    State(state): State<AppState>,
    Json(req): Json<PollDevice>,
) -> Result<axum::Json<DeviceToken>, Problem> {
    let gone = || Problem::not_found().detail("this sign-in expired; start again");
    let id = auth::id_of(auth::Kind::Device, &req.device_code).ok_or_else(gone)?;
    let mut tx = state.pool.begin().await?;
    let device = store::sign_in::device(&mut *tx, id)
        .await?
        .ok_or_else(gone)?;
    if !auth::check(auth::Kind::Device, &req.device_code, |_| {
        Some(device.secret_hash.clone())
    }) {
        return Err(gone());
    }
    let Some(user) = device.approved_by else {
        return Ok(axum::Json(DeviceToken { token: None }));
    };
    if !store::sign_in::take_device(&mut *tx, id).await? {
        return Err(gone());
    }
    let user = store::users::find(&mut *tx, user)
        .await?
        .filter(|u| u.removed_at.is_none())
        .ok_or_else(gone)?;
    let name = format!(
        "{} · {} · {}",
        user.name,
        device.client,
        auth::hex(&auth::random::<3>()?)
    );
    let days = u32::try_from(device.ttl_days).map_err(|_| Problem::internal())?;
    let expires_at = OffsetDateTime::now_utc() + tokens::ttl(days)?;
    let (_, token) = tokens::issue(
        &mut tx,
        None,
        &name,
        store::tokens::Kind::Cli,
        user.id,
        expires_at,
    )
    .await?;
    tx.commit().await?;
    Ok(axum::Json(DeviceToken { token: Some(token) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_codes_are_read_forgivingly() {
        assert_eq!(user_code("bcdf ghjk").as_deref(), Some("BCDF-GHJK"));
        assert_eq!(user_code("BCDF-GHJK").as_deref(), Some("BCDF-GHJK"));
        assert_eq!(user_code("BCDF-GHJ"), None);
        assert_eq!(user_code("ABCD-EFGH"), None); // vowels never appear
        let fresh = new_user_code().unwrap();
        assert_eq!(user_code(&fresh), Some(fresh));
    }

    #[test]
    fn the_origin_names_the_relying_party() {
        assert!(passkeys("https://books.example.org/exchequer/").is_ok());
        assert!(passkeys("http://localhost:5173").is_ok());
        assert!(passkeys("not a url").is_err());
        assert!(passkeys("http://books.example.org").is_err());
    }
}
