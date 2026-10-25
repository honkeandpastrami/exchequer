//! Calls the app in-process, as a client would.
#![allow(dead_code)] // each test file uses a different subset

pub mod authenticator;
pub mod household;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

/// Where the web app is served, for passkeys.
pub const ORIGIN: &str = "https://books.example.org";

pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}

impl Response {
    pub fn etag(&self) -> String {
        self.headers[header::ETAG].to_str().unwrap().to_owned()
    }

    pub fn id(&self) -> String {
        self.body["id"].as_str().unwrap().to_owned()
    }
}

/// A request under construction.
pub struct Call<'a> {
    pool: &'a PgPool,
    method: &'a str,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Value>,
    bytes: Option<Vec<u8>>,
    suggestions: Option<api::Suggestions>,
    pricing: Option<api::Pricing>,
    passkeys_off: bool,
}

pub fn call<'a>(pool: &'a PgPool, method: &'a str, path: impl Into<String>) -> Call<'a> {
    Call {
        pool,
        method,
        path: path.into(),
        headers: vec![],
        body: None,
        bytes: None,
        suggestions: None,
        pricing: None,
        passkeys_off: false,
    }
}

impl Call<'_> {
    pub fn token(mut self, token: &str) -> Self {
        self.headers
            .push((header::AUTHORIZATION.to_string(), format!("Bearer {token}")));
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn if_match(self, etag: &str) -> Self {
        self.header("if-match", etag)
    }

    pub fn json(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }

    /// Another suggestion stack than the default (e.g. with remote AI).
    pub fn suggestions(mut self, s: api::Suggestions) -> Self {
        self.suggestions = Some(s);
        self
    }

    /// Price sources (by default, none).
    pub fn pricing(mut self, p: api::Pricing) -> Self {
        self.pricing = Some(p);
        self
    }

    /// A server without `EXCHEQUER_ORIGIN`: no passkeys.
    pub fn passkeys_off(mut self) -> Self {
        self.passkeys_off = true;
        self
    }

    /// A raw body, e.g. an uploaded file.
    pub fn bytes(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.bytes = Some(body.into());
        self
    }

    pub async fn send(self) -> Response {
        let app = api::app(api::AppState {
            pool: self.pool.clone(),
            passkeys: (!self.passkeys_off).then(|| api::passkeys(ORIGIN).unwrap()),
            zone: jiff::tz::TimeZone::get("America/New_York").unwrap(),
            suggestions: self.suggestions.unwrap_or_default(),
            pricing: self.pricing.unwrap_or_default(),
        });
        let mut req = Request::builder().method(self.method).uri(&self.path);
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }
        let req = match (self.body, self.bytes) {
            (Some(b), _) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            (None, Some(b)) => req
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(b)),
            (None, None) => req.body(Body::empty()),
        }
        .unwrap();
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        Response {
            status,
            headers,
            body,
        }
    }
}

/// The user tests' tokens belong to, added on first use.
pub const TESTER: &str = "Tester";

/// A token for `TESTER`, as if they signed the CLI in; returns `(id, token)`.
pub async fn new_token(pool: &PgPool) -> (String, String) {
    token_of(pool, store::tokens::NewToken::Cli(None)).await
}

/// A script's token for `TESTER`; returns `(id, token)`.
pub async fn script_token(pool: &PgPool, name: &str) -> (String, String) {
    token_of(pool, store::tokens::NewToken::Script(name)).await
}

async fn token_of(pool: &PgPool, new: store::tokens::NewToken<'_>) -> (String, String) {
    let (user,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO app_user (id, name) VALUES ($1, $2)
         ON CONFLICT (name) WHERE removed_at IS NULL DO UPDATE SET name = excluded.name
         RETURNING id",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(TESTER)
    .fetch_one(pool)
    .await
    .unwrap();
    let secret = api::auth::generate(api::auth::Kind::Token).unwrap();
    store::tokens::insert(
        pool,
        domain::TokenId::from_uuid(secret.id),
        new,
        &secret.hash,
        domain::UserId::from_uuid(user),
        time::OffsetDateTime::now_utc() + time::Duration::days(30),
    )
    .await
    .unwrap();
    (secret.id.to_string(), secret.text)
}
