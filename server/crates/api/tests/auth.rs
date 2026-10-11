//! Token authentication and `/api/v1/tokens`, end to end.
//! Needs the dev database: `just db-up`.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const BOOTSTRAP: &str = "bootstrap-token-for-tests-0123456789abcdef";

struct Response {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: Value,
}

async fn call(
    pool: &PgPool,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Response {
    let app = api::app(api::AppState {
        pool: pool.clone(),
        bootstrap_token: Some(BOOTSTRAP.into()),
    });
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = match body {
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

async fn create_token(pool: &PgPool, name: &str) -> (String, String) {
    let r = call(
        pool,
        "POST",
        "/api/v1/tokens",
        Some(BOOTSTRAP),
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    (
        r.body["id"].as_str().unwrap().to_owned(),
        r.body["token"].as_str().unwrap().to_owned(),
    )
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn requests_without_a_valid_token_are_rejected(pool: PgPool) {
    let (_, token) = create_token(&pool, "cli").await;
    let wrong_secret = format!(
        "{}{}",
        &token[..token.len() - 1],
        if token.ends_with('0') { '1' } else { '0' }
    );

    for t in [
        None,
        Some("garbage"),
        Some(wrong_secret.as_str()),
        Some(
            "exq_0123456789abcdef0123456789abcdef_0000000000000000000000000000000000000000000000000000000000000000",
        ),
    ] {
        let r = call(&pool, "GET", "/api/v1/tokens", t, None).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{t:?}");
        assert_eq!(r.headers[header::WWW_AUTHENTICATE], "Bearer");
        assert_eq!(r.headers[header::CONTENT_TYPE], "application/problem+json");
        assert_eq!(r.body["type"], "urn:exchequer:problem:unauthorized");
    }
    // Unknown paths need a token too, so they don't reveal which routes exist.
    let r = call(&pool, "GET", "/api/v1/no-such-thing", None, None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let r = call(&pool, "GET", "/api/v1/no-such-thing", Some(&token), None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // Operations endpoints stay open.
    assert_eq!(
        call(&pool, "GET", "/healthz", None, None).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn token_lifecycle(pool: PgPool) {
    let (id, token) = create_token(&pool, "alex-laptop").await;
    assert!(token.starts_with("exq_"));

    // The new token works, and listing never shows secrets.
    let r = call(&pool, "GET", "/api/v1/tokens", Some(&token), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body.as_array().unwrap().len(), 1);
    assert_eq!(r.body[0]["name"], "alex-laptop");
    assert!(r.body[0]["revoked_at"].is_null());
    assert!(!r.body.to_string().contains(&token[37..]));
    assert!(r.body[0].get("secret_hash").is_none());

    // A token can create others; names are unique.
    let r = call(
        &pool,
        "POST",
        "/api/v1/tokens",
        Some(&token),
        Some(json!({ "name": "web" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    let r = call(
        &pool,
        "POST",
        "/api/v1/tokens",
        Some(&token),
        Some(json!({ "name": "web" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(r.body["type"], "urn:exchequer:problem:conflict");

    // Revoking stops it immediately; revoking twice is harmless.
    let path = format!("/api/v1/tokens/{id}");
    assert_eq!(
        call(&pool, "DELETE", &path, Some(BOOTSTRAP), None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&pool, "GET", "/api/v1/tokens", Some(&token), None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&pool, "DELETE", &path, Some(BOOTSTRAP), None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );

    let r = call(
        &pool,
        "DELETE",
        "/api/v1/tokens/00000000-0000-0000-0000-000000000000",
        Some(BOOTSTRAP),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // The audit log has create, create, revoke, and never a secret.
    let rows: Vec<(String, Option<Value>)> =
        sqlx::query_as("SELECT action, after FROM audit_log ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let actions: Vec<&str> = rows.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(actions, ["create", "create", "update"]);
    let all = format!("{rows:?}");
    assert!(!all.contains(&token[37..]));
    assert!(!all.contains("argon2"));
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn bootstrap_token_only_manages_tokens(pool: PgPool) {
    let r = call(&pool, "GET", "/api/v1/tokens", Some(BOOTSTRAP), None).await;
    assert_eq!(r.status, StatusCode::OK);
    let r = call(&pool, "GET", "/api/v1/no-such-thing", Some(BOOTSTRAP), None).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.body["type"], "urn:exchequer:problem:forbidden");
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn bad_requests_are_problem_details(pool: PgPool) {
    for body in [
        json!({}),
        json!({ "name": "  " }),
        json!({ "name": "x".repeat(65) }),
    ] {
        let r = call(&pool, "POST", "/api/v1/tokens", Some(BOOTSTRAP), Some(body)).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(r.headers[header::CONTENT_TYPE], "application/problem+json");
    }
    let r = call(
        &pool,
        "DELETE",
        "/api/v1/tokens/not-a-uuid",
        Some(BOOTSTRAP),
        None,
    )
    .await;
    assert_eq!(r.body["type"], "urn:exchequer:problem:invalid-request");
}
