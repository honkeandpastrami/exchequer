//! `/api/v1/parties`, plus the §5 conventions (ETag/If-Match, pagination,
//! idempotency keys) exercised through it. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::{call, new_token};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn crud_with_etags(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;

    let r = call(&pool, "POST", "/api/v1/parties")
        .token(&t)
        .json(json!({ "name": "Alex", "kind": "person" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["kind"], "person");
    let id = r.id();
    let path = format!("/api/v1/parties/{id}");
    assert_eq!(r.headers["location"], path.as_str());
    let v1 = r.etag();

    let r = call(&pool, "GET", &path).token(&t).send().await;
    assert_eq!((r.status, r.etag()), (StatusCode::OK, v1.clone()));

    // Edits need If-Match.
    let rename = json!({ "name": "Alexandra" });
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .json(rename.clone())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_REQUIRED);
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .if_match(&v1)
        .json(rename.clone())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["name"], "Alexandra");
    let v2 = r.etag();
    assert_ne!(v1, v2);

    // A stale ETag is refused: someone else edited it in between.
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .if_match(&v1)
        .json(json!({ "name": "Al" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(r.body["type"], "urn:exchequer:problem:stale");
    let r = call(&pool, "DELETE", &path)
        .token(&t)
        .if_match(&v1)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED);

    let r = call(&pool, "DELETE", &path)
        .token(&t)
        .if_match(&v2)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = call(&pool, "GET", &path).token(&t).send().await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    let actions: Vec<(String,)> =
        sqlx::query_as("SELECT action FROM audit_log WHERE entity = 'party' ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        actions.into_iter().map(|a| a.0).collect::<Vec<_>>(),
        ["create", "update", "delete"]
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn validation(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;
    let post = |body| call(&pool, "POST", "/api/v1/parties").token(&t).json(body);

    assert_eq!(
        post(json!({ "name": "Household", "kind": "household" }))
            .send()
            .await
            .status,
        StatusCode::CREATED
    );
    let r = post(json!({ "name": "Second", "kind": "household" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        post(json!({ "name": "Household", "kind": "person" }))
            .send()
            .await
            .status,
        StatusCode::CONFLICT
    );
    for bad in [
        json!({ "name": "", "kind": "person" }),
        json!({ "name": "X", "kind": "pet" }),
        json!({ "name": "X" }),
    ] {
        assert_eq!(
            post(bad).send().await.status,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn pagination(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;
    for i in 0..5 {
        call(&pool, "POST", "/api/v1/parties")
            .token(&t)
            .json(json!({ "name": format!("P{i}"), "kind": "person" }))
            .send()
            .await;
    }
    let mut names = vec![];
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let path = match &cursor {
            Some(c) => format!("/api/v1/parties?limit=2&cursor={c}"),
            None => "/api/v1/parties?limit=2".to_owned(),
        };
        let r = call(&pool, "GET", path).token(&t).send().await;
        assert_eq!(r.status, StatusCode::OK);
        pages += 1;
        for p in r.body["items"].as_array().unwrap() {
            names.push(p["name"].as_str().unwrap().to_owned());
        }
        match r.body.get("next_cursor") {
            Some(c) => cursor = Some(c.as_str().unwrap().to_owned()),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(names, ["P0", "P1", "P2", "P3", "P4"]);

    let r = call(&pool, "GET", "/api/v1/parties?cursor=nope")
        .token(&t)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn idempotency_keys(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;
    let (_, other) = new_token(&pool, "web").await;
    let post = |token: &str, body| {
        call(&pool, "POST", "/api/v1/parties")
            .token(token)
            .header("idempotency-key", "k-1")
            .json(body)
    };

    let first = post(&t, json!({ "name": "Sam", "kind": "person" }))
        .send()
        .await;
    assert_eq!(first.status, StatusCode::CREATED);
    // A retry gets the same response, and nothing is created twice.
    let retry = post(&t, json!({ "name": "Sam", "kind": "person" }))
        .send()
        .await;
    assert_eq!(retry.status, StatusCode::CREATED);
    assert_eq!(retry.id(), first.id());
    assert_eq!(retry.headers["idempotency-replayed"], "true");
    // The same key for a different request is an error.
    let r = post(&t, json!({ "name": "Other", "kind": "person" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    // Keys are per token.
    let r = post(&other, json!({ "name": "Studio", "kind": "business" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED);

    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM party")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2);
}
