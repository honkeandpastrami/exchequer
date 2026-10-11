//! Calls the app in-process, as a client would.
#![allow(dead_code)] // each test file uses a different subset

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
}

pub fn call<'a>(pool: &'a PgPool, method: &'a str, path: impl Into<String>) -> Call<'a> {
    Call {
        pool,
        method,
        path: path.into(),
        headers: vec![],
        body: None,
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

    pub async fn send(self) -> Response {
        let app = api::app(api::AppState {
            pool: self.pool.clone(),
            bootstrap_token: Some(BOOTSTRAP.into()),
        });
        let mut req = Request::builder().method(self.method).uri(&self.path);
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }
        let req = match self.body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
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
