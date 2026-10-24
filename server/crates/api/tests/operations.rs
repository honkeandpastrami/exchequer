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
        passkeys: None,
        zone: jiff::tz::TimeZone::get("America/New_York").unwrap(),
        suggestions: api::Suggestions::default(),
        pricing: api::Pricing::default(),
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

/// Generated clients key operations by ID: a duplicate silently merges two.
#[test]
fn operation_ids_are_unique() {
    let doc = serde_json::to_value(api::openapi()).unwrap();
    let mut seen = std::collections::HashSet::new();
    for item in doc["paths"].as_object().unwrap().values() {
        for op in item.as_object().unwrap().values() {
            let id = op["operationId"]
                .as_str()
                .expect("every operation has an ID");
            assert!(seen.insert(id.to_owned()), "duplicate operationId {id}");
        }
    }
}
