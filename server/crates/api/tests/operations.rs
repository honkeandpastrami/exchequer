//! `/healthz`, `/version`, and the committed OpenAPI document.
//! Needs the dev database: `just db-up`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

async fn get(pool: PgPool, path: &str) -> (StatusCode, serde_json::Value) {
    let res = api::app(api::AppState {
        pool,
        bootstrap_token: None,
    })
    .oneshot(Request::get(path).body(Body::empty()).unwrap())
    .await
    .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap())
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn healthz(pool: PgPool) {
    let (status, body) = get(pool.clone(), "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    pool.close().await;
    let (status, body) = get(pool, "/healthz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "database unavailable");
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn version(pool: PgPool) {
    let (status, body) = get(pool, "/version").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["schema"], store::schema_version());
    assert!(body["version"].is_string());
}

/// The committed spec must match the code. Regenerate it with `just spec`.
#[test]
fn committed_spec_is_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../api/openapi.json");
    let committed = std::fs::read_to_string(path).unwrap_or_default();
    let generated = api::openapi().to_pretty_json().unwrap() + "\n";
    assert!(
        committed == generated,
        "api/openapi.json is out of date: run `just spec`"
    );
}
