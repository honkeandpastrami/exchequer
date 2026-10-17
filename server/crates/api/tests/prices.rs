//! `/api/v1/prices` (design §3.4): manual prices, and valuations that
//! prefer one price to a chain. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::{H, create};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn fetched(pool: &PgPool, base: &str, quote: &str, date: &str, rate: &str, source: &str) {
    sqlx::query(
        "INSERT INTO price (id, base_id, quote_id, date, rate, source)
         VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3::date, $4::numeric, $5)",
    )
    .bind(base)
    .bind(quote)
    .bind(date)
    .bind(rate)
    .bind(source)
    .execute(pool)
    .await
    .unwrap();
}

async fn fiat(h: &H, pool: &PgPool, code: &str) -> String {
    create(
        pool,
        &h.t,
        "/api/v1/commodities",
        json!({ "code": code, "kind": "fiat", "precision": 2 }),
    )
    .await
}

async fn value(h: &H, pool: &PgPool, query: &str) -> (StatusCode, Value) {
    let r = h
        .send(pool, "GET", &format!("/api/v1/prices/value?{query}"), None)
        .await;
    (r.status, r.body)
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn manual_prices(pool: PgPool) {
    let h = H::new(&pool).await;
    let eur = fiat(&h, &pool, "EUR").await;
    let new = json!({ "base_id": eur, "quote_id": h.usd, "date": "2026-06-01", "rate": "1.0842" });
    let r = h
        .send(&pool, "POST", "/api/v1/prices", Some(new.clone()))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["source"], "manual");
    assert_eq!(r.body["rate"], "1.0842");
    let (id, etag) = (r.id(), r.etag());

    let again = h.send(&pool, "POST", "/api/v1/prices", Some(new)).await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.body);
    for bad in [
        json!({ "base_id": eur, "quote_id": h.usd, "date": "2026-06-02", "rate": "0" }),
        json!({ "base_id": eur, "quote_id": h.usd, "date": "2026-06-02", "rate": 1.1 }),
        json!({ "base_id": eur, "quote_id": eur, "date": "2026-06-02", "rate": "1" }),
    ] {
        let r = h
            .send(&pool, "POST", "/api/v1/prices", Some(bad.clone()))
            .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.body
        );
    }

    let (status, v) = value(&h, &pool, "from=USD&to=EUR&date=2026-06-03").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["prices"][0]["id"], json!(id));
    assert_eq!(v["prices"][0]["inverted"], true);
    assert_eq!(v["derived"], false);
    assert_eq!(v["stale"], false);

    let r = common::call(&pool, "PATCH", format!("/api/v1/prices/{id}"))
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "rate": "1.25" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let (_, v) = value(&h, &pool, "from=USD&to=EUR&date=2026-06-03").await;
    assert_eq!(v["rate"], "0.8");

    let (status, _) = value(&h, &pool, "from=USD&to=EUR&date=2026-05-31").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = value(&h, &pool, "from=USD&to=XYZ").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let r = common::call(&pool, "DELETE", format!("/api/v1/prices/{id}"))
        .token(&h.t)
        .if_match(&r.etag())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let (status, _) = value(&h, &pool, "from=USD&to=EUR&date=2026-06-03").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn one_source_before_a_chain(pool: PgPool) {
    let h = H::new(&pool).await;
    let eur = fiat(&h, &pool, "EUR").await;
    let gbp = fiat(&h, &pool, "GBP").await;
    fetched(&pool, &eur, &h.usd, "2026-06-01", "1.25", "provider:ecb").await;
    fetched(&pool, &eur, &gbp, "2026-06-01", "0.80", "provider:ecb").await;

    // Only the ECB's two rates connect GBP and USD: derived, one source.
    let (_, v) = value(&h, &pool, "from=GBP&to=USD&date=2026-06-01").await;
    assert_eq!(v["rate"], "1.5625");
    assert_eq!(v["derived"], true);
    assert_eq!(v["prices"].as_array().unwrap().len(), 2);

    // A direct quote wins, whatever its source.
    fetched(&pool, &gbp, &h.usd, "2026-06-01", "1.55", "provider:tiingo").await;
    let (_, v) = value(&h, &pool, "from=GBP&to=USD&date=2026-06-01").await;
    assert_eq!(v["rate"], "1.55");
    assert_eq!(v["derived"], false);
    assert_eq!(v["prices"][0]["source"], "provider:tiingo");

    // A fetched price can't be edited.
    let list = h
        .send(&pool, "GET", "/api/v1/prices?source=provider:tiingo", None)
        .await;
    let fetched_id = list.body["items"][0]["id"].as_str().unwrap().to_owned();
    let one = h
        .send(&pool, "GET", &format!("/api/v1/prices/{fetched_id}"), None)
        .await;
    let r = common::call(&pool, "PATCH", format!("/api/v1/prices/{fetched_id}"))
        .token(&h.t)
        .if_match(&one.etag())
        .json(json!({ "rate": "1.6" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
}
