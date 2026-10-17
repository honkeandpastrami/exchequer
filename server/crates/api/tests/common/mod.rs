//! Calls the app in-process, as a client would.
#![allow(dead_code)] // each test file uses a different subset

pub mod household;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

pub const BOOTSTRAP: &str = "bootstrap-token-for-tests-0123456789abcdef";

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

    /// A raw body, e.g. an uploaded file.
    pub fn bytes(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.bytes = Some(body.into());
        self
    }

    pub async fn send(self) -> Response {
        let app = api::app(api::AppState {
            pool: self.pool.clone(),
            bootstrap_token: Some(BOOTSTRAP.into()),
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

/// Creates a token with the bootstrap token; returns `(id, token)`.
pub async fn new_token(pool: &PgPool, name: &str) -> (String, String) {
    let r = call(pool, "POST", "/api/v1/tokens")
        .token(BOOTSTRAP)
        .json(json!({ "name": name }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    (r.id(), r.body["token"].as_str().unwrap().to_owned())
}
