//! `/api/v1/accounts/{id}/numbers`: the numbers bank files name accounts by,
//! never shown or logged in full. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::H;
use common::{Response, call};
use domain::AccountNumber;
use serde_json::json;
use sqlx::PgPool;

const FULL: &str = "000123456789";

async fn add(pool: &PgPool, h: &H, account: &str, etag: &str, number: &str) -> Response {
    call(pool, "POST", format!("/api/v1/accounts/{account}/numbers"))
        .token(&h.t)
        .if_match(etag)
        .json(json!({ "number": number }))
        .send()
        .await
}

async fn read(pool: &PgPool, h: &H, account: &str) -> Response {
    h.send(pool, "GET", &format!("/api/v1/accounts/{account}"), None)
        .await
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn an_account_has_numbers_shown_by_their_last_four(pool: PgPool) {
    let h = H::new(&pool).await;
    let card = read(&pool, &h, &h.alex_credit).await;
    assert_eq!(card.body["numbers"], json!([]));

    for bad in ["", " - ", "12*34", &"1".repeat(35)] {
        let r = add(&pool, &h, &h.alex_credit, &card.etag(), bad).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad:?}");
    }
    let r = add(&pool, &h, &h.alex_credit, "\"999\"", FULL).await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED);

    // Spaces and dashes are ignored; a card can have a second number (the
    // one its payments arrive under).
    let r = add(&pool, &h, &h.alex_credit, &card.etag(), "0001-2345 6789").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let r = add(&pool, &h, &h.alex_credit, &r.etag(), "4000111122223333").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let last4: Vec<_> = r.body["numbers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["last4"].as_str().unwrap())
        .collect();
    assert_eq!(last4, ["6789", "3333"]);
    assert!(!r.body.to_string().contains(FULL));

    // Adding it again changes nothing; another account can't have it.
    let etag = r.etag();
    let again = add(&pool, &h, &h.alex_credit, &etag, FULL).await;
    assert_eq!((again.status, again.etag()), (StatusCode::OK, etag.clone()));
    let checking = read(&pool, &h, &h.alex_checking).await;
    let r = add(&pool, &h, &h.alex_checking, &checking.etag(), FULL).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert!(
        r.body["detail"].as_str().unwrap().contains("…6789"),
        "{}",
        r.body
    );
    assert!(!r.body.to_string().contains(FULL));

    // A number resolves to its account.
    let found = store::account_numbers::find(&pool, &AccountNumber::new(FULL).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.account_id.to_string(), h.alex_credit);
    assert!(
        store::account_numbers::find(&pool, &AccountNumber::new("1").unwrap())
            .await
            .unwrap()
            .is_none()
    );

    // Removing one.
    let id = again.body["numbers"][0]["id"].as_str().unwrap().to_owned();
    let path = format!("/api/v1/accounts/{}/numbers/{id}", h.alex_credit);
    let r = call(&pool, "DELETE", &path)
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["numbers"].as_array().unwrap().len(), 1);
    let r = call(&pool, "DELETE", &path)
        .token(&h.t)
        .if_match(&r.etag())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // Now free for another account.
    let r = add(&pool, &h, &h.alex_checking, &checking.etag(), FULL).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    // The audit log has only masked numbers.
    let logged: Vec<String> = sqlx::query_scalar(
        "SELECT coalesce(before::text, '') || coalesce(after::text, '') FROM audit_log",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(logged.iter().any(|l| l.contains("6789")));
    assert!(!logged.iter().any(|l| l.contains(FULL)));
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn numbers_go_with_their_account(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = h
        .send(
            &pool,
            "POST",
            "/api/v1/accounts",
            Some(json!({ "name": "Spare", "kind": "cash", "owner_id": h.alex })),
        )
        .await;
    let id = r.id();
    let r = add(&pool, &h, &id, &r.etag(), FULL).await;
    let r = call(&pool, "DELETE", format!("/api/v1/accounts/{id}"))
        .token(&h.t)
        .if_match(&r.etag())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM account_number")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
