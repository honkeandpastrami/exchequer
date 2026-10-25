//! Conventions shared by every resource (design §5): optimistic concurrency
//! with ETag/If-Match, cursor pagination, and idempotency keys on POST.

use axum::body::{Body, to_bytes};
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderName, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use store::idempotency::Stored;
use utoipa::{IntoParams, ToSchema};

use crate::AppState;
use crate::auth::{Principal, hex};
use crate::problem::Problem;

// ---------------------------------------------------------------- ETag ----

/// The `ETag` header for an entity at `version`.
pub fn etag(version: i64) -> [(HeaderName, HeaderValue); 1] {
    let value = HeaderValue::from_str(&format!("\"{version}\"")).expect("digits are valid");
    [(header::ETAG, value)]
}

/// The version a client read, from `If-Match`. Required on every edit, so two
/// people editing the same thing can't silently overwrite each other.
pub struct IfMatch(pub i64);

impl<S: Send + Sync> FromRequestParts<S> for IfMatch {
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Problem> {
        let value = parts.headers.get(header::IF_MATCH).ok_or_else(|| {
            Problem::new(
                StatusCode::PRECONDITION_REQUIRED,
                "if-match-required",
                "If-Match required",
            )
            .detail("send the ETag from your last read in If-Match")
        })?;
        value
            .to_str()
            .ok()
            .map(|v| v.trim().trim_start_matches("W/").trim_matches('"'))
            .and_then(|v| v.parse().ok())
            .map(Self)
            .ok_or_else(|| Problem::invalid("If-Match must be an ETag from this API"))
    }
}

impl Problem {
    /// Someone else edited the entity since the client read it.
    pub fn stale() -> Self {
        Self::new(
            StatusCode::PRECONDITION_FAILED,
            "stale",
            "Changed since you read it",
        )
        .detail("read it again and reapply your change")
    }
}

// ---------------------------------------------------------- Pagination ----

const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageParams {
    /// Items per page (default 100, max 500).
    pub limit: Option<u32>,
    /// `next_cursor` from the previous page. Opaque.
    pub cursor: Option<String>,
}

impl PageParams {
    pub fn limit(&self) -> i64 {
        i64::from(self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT))
    }
}

/// One page of results.
#[derive(Serialize, ToSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Pass as `cursor` to get the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl<T> Page<T> {
    /// Builds a page from `limit + 1` fetched rows: the extra row only tells
    /// whether there's a next page. `cursor` gives the cursor after an item.
    pub fn from_rows(mut rows: Vec<T>, limit: i64, cursor: impl Fn(&T) -> String) -> Self {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next_cursor = if more { rows.last().map(cursor) } else { None };
        Self {
            items: rows,
            next_cursor,
        }
    }
}

// --------------------------------------------------------- Idempotency ----

const MAX_BODY: usize = 2 * 1024 * 1024;

/// Middleware: a POST with an `Idempotency-Key` header runs once; retries
/// with the same key and the same request get the stored response back. The
/// same key with a different request is an error. Keys are per token and
/// remembered for 24 hours.
pub async fn idempotency(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Problem> {
    let key = req
        .headers()
        .get("idempotency-key")
        .map(|v| v.to_str().map(str::to_owned));
    let principal = req.extensions().get::<Principal>().copied();
    let (Some(key), Method::POST, Some(Principal { token, .. })) =
        (key, req.method().clone(), principal)
    else {
        return Ok(next.run(req).await);
    };
    let key = key
        .ok()
        .filter(|k| (1..=255).contains(&k.len()))
        .ok_or_else(|| {
            Problem::invalid("Idempotency-Key must be 1-255 visible ASCII characters")
        })?;

    let (parts, body) = req.into_parts();
    let bytes = to_bytes(body, MAX_BODY)
        .await
        .map_err(|_| Problem::invalid("request body too large"))?;
    let mut hasher = Sha256::new();
    hasher.update(parts.uri.path().as_bytes());
    hasher.update(b"\n");
    hasher.update(&bytes);
    let request_hash = hex(&hasher.finalize());

    if let Some(stored) = store::idempotency::find(&state.pool, token, &key).await? {
        if stored.request_hash != request_hash {
            return Err(Problem::invalid(
                "this Idempotency-Key was already used for a different request",
            ));
        }
        return Ok(replay(&stored));
    }

    let res = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if res.status().is_server_error() {
        return Ok(res);
    }
    let (res_parts, res_body) = res.into_parts();
    let res_bytes = to_bytes(res_body, MAX_BODY)
        .await
        .map_err(|_| Problem::internal())?;
    let body = if res_bytes.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(&res_bytes).map_err(|_| Problem::internal())?)
    };
    let stored = Stored {
        request_hash,
        status: i16::try_from(res_parts.status.as_u16()).map_err(|_| Problem::internal())?,
        body,
    };
    store::idempotency::save(&state.pool, token, &key, &stored).await?;
    store::idempotency::prune(&state.pool).await?;
    Ok(Response::from_parts(res_parts, Body::from(res_bytes)))
}

fn replay(stored: &Stored) -> Response {
    let status = u16::try_from(stored.status)
        .ok()
        .and_then(|s| StatusCode::from_u16(s).ok())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut res = match &stored.body {
        Some(body) => (status, axum::Json(body)).into_response(),
        None => status.into_response(),
    };
    if status.is_client_error() {
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
    }
    res.headers_mut()
        .insert("idempotency-replayed", HeaderValue::from_static("true"));
    res
}
