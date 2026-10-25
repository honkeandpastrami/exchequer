//! `/api/v1/sessions`: the clients using tokens (design §10). A browser or
//! the CLI signed in is a session; so is each tool, on each host, using a
//! script's token. Signing a session out revokes its token.

use std::net::SocketAddr;

use axum::Extension;
use axum::extract::{ConnectInfo, FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode, header};
use domain::TokenId;
use serde::Serialize;
use store::tokens::{Client, Kind, SessionRow};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Principal;
use crate::problem::{Path, Problem};
use crate::tokens;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(sign_out))
}

/// The header a client names its host in.
pub const HOST_HEADER: &str = "exchequer-host";

/// What signed a session in.
#[derive(Serialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    /// A browser, with a passkey.
    Browser,
    /// `exchequer login`, approved in a browser.
    Cli,
    /// A script, with a token made for it.
    Script,
}

impl From<Kind> for SessionKind {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Browser => Self::Browser,
            Kind::Cli => Self::Cli,
            Kind::Script => Self::Script,
        }
    }
}

/// A client using a token that still works.
#[derive(Serialize, ToSchema)]
pub struct Session {
    pub id: Uuid,
    /// The token it uses: signing out revokes it, ending every session
    /// using it.
    token_id: Uuid,
    kind: SessionKind,
    user_id: Option<Uuid>,
    /// The passkey's name, or the script's token's; none if the passkey was
    /// deleted, or for a session older than this record.
    signed_in_with: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    /// To the minute.
    #[serde(with = "time::serde::rfc3339")]
    last_used_at: OffsetDateTime,
    /// When its token stops working, unless revoked first.
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    /// What it last used: `Safari on iPhone`, `exchequer-cli 1.4.0`. As the
    /// client reports it (`User-Agent`).
    client: Option<String>,
    /// The host it last used it from, as the client reports it
    /// (`Exchequer-Host`).
    host: Option<String>,
    /// The address its last request came from, as the reverse proxy reports
    /// it (`X-Forwarded-For`).
    ip: Option<String>,
}

impl From<SessionRow> for Session {
    fn from(r: SessionRow) -> Self {
        Self {
            id: r.id,
            token_id: *r.token_id.as_uuid(),
            kind: r.kind.into(),
            user_id: r.user_id.map(|u| *u.as_uuid()),
            signed_in_with: r.signed_in_with,
            started_at: r.started_at,
            last_used_at: r.last_used_at,
            expires_at: r.expires_at,
            client: r.agent.as_deref().and_then(describe),
            host: r.host,
            ip: r.ip,
        }
    }
}

/// The request's client, as it describes itself. Informational only: any
/// client can say anything, and none is refused for it.
pub struct ClientOf(pub Client);

impl<S: Send + Sync> FromRequestParts<S> for ClientOf {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0);
        Ok(Self(client(&parts.headers, peer)))
    }
}

/// A header's text, trimmed and cut to `max` characters; `None` if empty.
fn text(headers: &HeaderMap, name: impl header::AsHeaderName, max: usize) -> Option<String> {
    let v = headers.get(name)?.to_str().ok()?.trim();
    (!v.is_empty()).then(|| v.chars().take(max).collect())
}

/// The client's `User-Agent` and host, and the address the request came
/// from: the last one the reverse proxy added to `X-Forwarded-For`, else
/// the peer's.
pub(crate) fn client(headers: &HeaderMap, peer: Option<SocketAddr>) -> Client {
    let forwarded = text(headers, "x-forwarded-for", 512).and_then(|v| {
        v.rsplit(',')
            .map(str::trim)
            .find(|a| !a.is_empty())
            .map(|a| a.chars().take(64).collect())
    });
    Client {
        agent: text(headers, header::USER_AGENT, 512),
        host: text(headers, HOST_HEADER, 64),
        ip: forwarded.or_else(|| peer.map(|p| p.ip().to_string())),
    }
}

/// A `User-Agent` for people: a browser and the device it runs on
/// (`Safari on iPhone`), or a tool and its version (`exchequer-cli 1.4.0`).
pub(crate) fn describe(agent: &str) -> Option<String> {
    if agent.starts_with("Mozilla/") {
        let has = |s: &str| agent.contains(s);
        let browser = if has("Edg/") || has("EdgiOS/") || has("EdgA/") {
            "Edge"
        } else if has("Firefox/") || has("FxiOS/") {
            "Firefox"
        } else if has("Chrome/") || has("CriOS/") {
            "Chrome"
        } else if has("Safari/") {
            "Safari"
        } else {
            "A browser"
        };
        let device = [
            ("iPhone", "iPhone"),
            ("iPad", "iPad"),
            ("Android", "Android"),
            ("CrOS", "ChromeOS"),
            ("Macintosh", "Mac"),
            ("Windows", "Windows"),
            ("Linux", "Linux"),
        ]
        .into_iter()
        .find(|(k, _)| has(k));
        return Some(match device {
            Some((_, d)) => format!("{browser} on {d}"),
            None => browser.to_owned(),
        });
    }
    let first = agent.split_whitespace().next()?;
    Some(match first.split_once('/') {
        Some((tool, version)) if !version.is_empty() => format!("{tool} {version}"),
        _ => first.trim_end_matches('/').to_owned(),
    })
}

/// The sessions whose tokens still work, last used first.
#[utoipa::path(
    get,
    path = "/sessions",
    tag = "sessions",
    security(("bearer" = [])),
    responses(
        (status = OK, body = Vec<Session>),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list(State(state): State<AppState>) -> Result<axum::Json<Vec<Session>>, Problem> {
    let rows = store::tokens::sessions(&state.pool).await?;
    Ok(axum::Json(rows.into_iter().map(Session::from).collect()))
}

/// Sign a session out: its token is revoked, so every session using that
/// token ends (only a script's token has more than one).
#[utoipa::path(
    delete,
    path = "/sessions/{id}",
    tag = "sessions",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Session ID")),
    responses(
        (status = NO_CONTENT),
        (status = UNAUTHORIZED, body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such session, or it ended", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn sign_out(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Problem> {
    let mut tx = state.pool.begin().await?;
    let session = store::tokens::session(&mut *tx, id)
        .await?
        .ok_or_else(Problem::not_found)?;
    tokens::revoke(&mut tx, principal, session.token_id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The request's session, for `/me`.
pub(crate) async fn current(
    db: impl sqlx::PgExecutor<'_>,
    principal: Principal,
) -> Result<Option<Session>, Problem> {
    Ok(store::tokens::session(db, principal.session)
        .await?
        .map(Session::from))
}

/// Records the client using a token just made for a person, so their
/// session shows from the start.
pub(crate) async fn start(
    db: impl sqlx::PgExecutor<'_>,
    token: TokenId,
    kind: Kind,
    client: &Client,
) -> Result<(), Problem> {
    store::tokens::session_used(db, token, kind, client).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsers_and_tools_are_described() {
        for (agent, expected) in [
            (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 \
                 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
                "Safari on iPhone",
            ),
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",
                "Edge on Mac",
            ),
            (
                "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0",
                "Firefox on Linux",
            ),
            (
                "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/130.0 Mobile Safari/537.36",
                "Chrome on Android",
            ),
            (
                "exchequer-cli/1.4.0 python-httpx/0.28.1",
                "exchequer-cli 1.4.0",
            ),
            ("backup.py", "backup.py"),
            ("curl/8.7.1", "curl 8.7.1"),
        ] {
            assert_eq!(describe(agent).as_deref(), Some(expected), "{agent}");
        }
        assert_eq!(describe("   "), None);
    }

    #[test]
    fn the_address_is_the_one_the_proxy_added() {
        let mut headers = HeaderMap::new();
        let peer: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        assert_eq!(
            client(&headers, Some(peer)).ip.as_deref(),
            Some("127.0.0.1")
        );
        headers.insert("x-forwarded-for", "10.0.0.9, 192.168.1.20".parse().unwrap());
        headers.insert(HOST_HEADER, " laptop ".parse().unwrap());
        let c = client(&headers, Some(peer));
        assert_eq!(c.ip.as_deref(), Some("192.168.1.20"));
        assert_eq!(c.host.as_deref(), Some("laptop"));
        assert_eq!(c.agent, None);
    }
}
