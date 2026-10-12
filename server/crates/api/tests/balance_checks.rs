//! `/api/v1/balance-checks` (design §7.1): matches, verified-through dates,
//! the sign convention for cards, and candidates for a mismatch.
//! Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::call;
use common::household::H;
use serde_json::{Value, json};
use sqlx::PgPool;

async fn check(h: &H, pool: &PgPool, body: Value) -> Value {
    let r = h
        .send(pool, "POST", "/api/v1/balance-checks", Some(body.clone()))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{body}: {}", r.body);
    r.body
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn checks_and_candidates(pool: PgPool) {
    let h = H::new(&pool).await;
    let store = ("counterparty_id", h.store.as_str());
    let salary = h
        .tx(
            &pool,
            "2026-10-01",
            store,
            h.acct(&h.alex_checking),
            "1000.00",
            json!({}),
        )
        .await;
    assert_eq!(salary.status, StatusCode::CREATED, "{}", salary.body);
    for (date, qty) in [
        ("2026-10-03", "47.30"),
        ("2026-10-04", "47.30"),
        ("2026-10-05", "12.00"),
    ] {
        h.tx(&pool, date, h.acct(&h.alex_checking), store, qty, json!({}))
            .await;
    }

    // The bank agreed on the 1st: verified through then.
    let c = check(
        &h,
        &pool,
        json!({ "account_id": h.alex_checking, "date": "2026-10-01", "amount": "1000" }),
    )
    .await;
    assert_eq!(c["matches"], true);
    assert_eq!(c["ledger"], "1000.00");
    assert_eq!(c["amount"], "1000.00");
    assert_eq!(c["commodity_id"], json!(h.usd), "USD by default");
    assert_eq!(c["verified_through"], "2026-10-01");
    assert!(c.get("candidates").is_none());

    // On the 11th the bank shows 940.70, the ledger 893.40: the 47.30
    // charge was recorded twice.
    let c = check(
        &h,
        &pool,
        json!({ "account_id": h.alex_checking, "date": "2026-10-11", "amount": "940.70" }),
    )
    .await;
    assert_eq!(c["matches"], false);
    assert_eq!(c["ledger"], "893.40");
    assert_eq!(c["difference"], "-47.30");
    assert_eq!(c["verified_through"], "2026-10-01");
    assert_eq!(c["searched_after"], "2026-10-01");
    let candidates = c["candidates"].as_array().unwrap();
    assert_eq!(candidates[0]["reason"], "duplicate_pair");
    assert_eq!(candidates[0]["splits"].as_array().unwrap().len(), 2);

    // Listing shows both, newest first, without candidates; GET one has them.
    let r = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/balance-checks?account_id={}", h.alex_checking),
            None,
        )
        .await;
    let items = r.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["date"], "2026-10-11");
    assert!(items[0].get("candidates").is_none());
    let path = format!("/api/v1/balance-checks/{}", c["id"].as_str().unwrap());
    let r = h.send(&pool, "GET", &path, None).await;
    assert!(r.body["candidates"].is_array());

    // Fix the ledger (delete the duplicate): the same check now matches.
    let dup = &candidates[0]["splits"][1]["transaction_id"];
    let tx_path = format!("/api/v1/transactions/{}", dup.as_str().unwrap());
    let etag = h.send(&pool, "GET", &tx_path, None).await.etag();
    let r = call(&pool, "DELETE", &tx_path)
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = h.send(&pool, "GET", &path, None).await;
    assert_eq!(r.body["matches"], true);
    assert_eq!(r.body["verified_through"], "2026-10-11");

    // Delete the check.
    let r = call(&pool, "DELETE", &path)
        .token(&h.t)
        .if_match(&r.etag())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        h.send(&pool, "GET", &path, None).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn cards_show_owed_as_positive(pool: PgPool) {
    let h = H::new(&pool).await;
    let store = ("counterparty_id", h.store.as_str());
    h.tx(
        &pool,
        "2026-10-02",
        h.acct(&h.alex_credit),
        store,
        "50.00",
        json!({}),
    )
    .await;
    let c = check(
        &h,
        &pool,
        json!({ "account_id": h.alex_credit, "commodity_id": h.usd,
                "date": "2026-10-11", "amount": "50.00" }),
    )
    .await;
    assert_eq!(c["matches"], true, "{c}");
    assert_eq!(c["ledger"], "50.00");

    for bad in [
        json!({ "account_id": h.alex_credit, "commodity_id": h.usd, "amount": "1.001" }),
        json!({ "account_id": h.store, "commodity_id": h.usd, "amount": "1" }),
        json!({ "account_id": h.alex_credit, "commodity_id": h.usd, "amount": 1 }),
    ] {
        let r = h
            .send(&pool, "POST", "/api/v1/balance-checks", Some(bad.clone()))
            .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.body
        );
    }
}
