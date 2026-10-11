//! Commodities, accounts, counterparties, categories and tags.
//! Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::{Response, call, new_token};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn post(pool: &PgPool, t: &str, path: &str, body: Value) -> Response {
    call(pool, "POST", path).token(t).json(body).send().await
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn commodities(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;
    let r = post(
        &pool,
        &t,
        "/api/v1/commodities",
        json!({ "code": "USD", "kind": "fiat", "precision": 2 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let path = format!("/api/v1/commodities/{}", r.id());

    for bad in [
        json!({ "code": "usd", "kind": "fiat", "precision": 2 }),
        json!({ "code": "EUR", "kind": "fiat", "precision": 19 }),
        json!({ "code": "EUR", "kind": "money", "precision": 2 }),
        json!({ "code": "EUR", "kind": "fiat", "precision": -1 }),
    ] {
        let r = post(&pool, &t, "/api/v1/commodities", bad.clone()).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    let r = post(
        &pool,
        &t,
        "/api/v1/commodities",
        json!({ "code": "USD", "kind": "fiat", "precision": 2 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    // Precision can go up, never down.
    let etag = call(&pool, "GET", &path).token(&t).send().await.etag();
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .if_match(&etag)
        .json(json!({ "precision": 1 }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .if_match(&etag)
        .json(json!({ "precision": 4 }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        (r.body["code"].as_str(), r.body["precision"].as_u64()),
        (Some("USD"), Some(4))
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn accounts(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;
    let alex = post(
        &pool,
        &t,
        "/api/v1/parties",
        json!({ "name": "Alex", "kind": "person" }),
    )
    .await
    .id();
    let sam = post(
        &pool,
        &t,
        "/api/v1/parties",
        json!({ "name": "Sam", "kind": "person" }),
    )
    .await
    .id();

    let checking = post(
        &pool,
        &t,
        "/api/v1/accounts",
        json!({ "name": "Alex Checking", "kind": "checking", "owner_id": alex }),
    )
    .await;
    assert_eq!(checking.status, StatusCode::CREATED, "{}", checking.body);
    assert_eq!(checking.body["liquid"], true);
    let brokerage = post(
        &pool,
        &t,
        "/api/v1/accounts",
        json!({ "name": "Alex Brokerage", "kind": "brokerage", "owner_id": alex }),
    )
    .await;
    assert_eq!(brokerage.body["liquid"], false);

    let r = post(&pool, &t, "/api/v1/accounts", json!({ "name": "Ghost", "kind": "cash", "owner_id": "00000000-0000-0000-0000-000000000000" })).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // The owner is set once: an update that tries to change it is refused.
    let path = format!("/api/v1/accounts/{}", checking.id());
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .if_match(&checking.etag())
        .json(json!({ "owner_id": sam }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .if_match(&checking.etag())
        .json(json!({ "name": "Alex Main" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        (r.body["name"].as_str(), r.body["owner_id"].as_str()),
        (Some("Alex Main"), Some(alex.as_str()))
    );

    // Archived flag and credit limit: set, checked against the commodity, cleared.
    let usd = post(
        &pool,
        &t,
        "/api/v1/commodities",
        json!({ "code": "USD", "kind": "fiat", "precision": 2 }),
    )
    .await
    .id();
    let card = post(
        &pool,
        &t,
        "/api/v1/accounts",
        json!({ "name": "Alex Credit", "kind": "credit_card", "owner_id": alex,
                "credit_limit": { "commodity_id": usd, "quantity": "9000.00" } }),
    )
    .await;
    assert_eq!(card.status, StatusCode::CREATED, "{}", card.body);
    assert_eq!(
        (card.body["archived"].as_bool(), &card.body["credit_limit"]),
        (
            Some(false),
            &json!({ "commodity_id": usd, "quantity": "9000.00" })
        )
    );
    assert!(checking.body.get("credit_limit").is_none());
    for bad in ["9000.001", "0", "-5"] {
        let r = post(&pool, &t, "/api/v1/accounts", json!({ "name": "Bad Card", "kind": "credit_card", "owner_id": alex, "credit_limit": { "commodity_id": usd, "quantity": bad } })).await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.body
        );
    }
    let card_path = format!("/api/v1/accounts/{}", card.id());
    let r = call(&pool, "PATCH", &card_path)
        .token(&t)
        .if_match(&card.etag())
        .json(json!({ "archived": true }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["archived"], true);
    assert_eq!(
        r.body["credit_limit"]["quantity"], "9000.00",
        "absent keeps it"
    );
    let r = call(&pool, "PATCH", &card_path)
        .token(&t)
        .if_match(&r.etag())
        .json(json!({ "credit_limit": null }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert!(r.body.get("credit_limit").is_none(), "null clears it");

    // A party that owns accounts can't be deleted.
    let party = call(&pool, "GET", format!("/api/v1/parties/{alex}"))
        .token(&t)
        .send()
        .await;
    let r = call(&pool, "DELETE", format!("/api/v1/parties/{alex}"))
        .token(&t)
        .if_match(&party.etag())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn counterparties(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;
    let r = call(&pool, "GET", "/api/v1/counterparties")
        .token(&t)
        .send()
        .await;
    let items = r.body["items"].as_array().unwrap().clone();
    let systems: Vec<&str> = items.iter().filter_map(|c| c["system"].as_str()).collect();
    assert_eq!(systems.len(), 2, "{items:?}");

    // Built-ins can be renamed but not deleted.
    let builtin = items
        .iter()
        .find(|c| c["system"] == "reconciliation")
        .unwrap();
    let path = format!("/api/v1/counterparties/{}", builtin["id"].as_str().unwrap());
    let etag = call(&pool, "GET", &path).token(&t).send().await.etag();
    let r = call(&pool, "DELETE", &path)
        .token(&t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = call(&pool, "PATCH", &path)
        .token(&t)
        .if_match(&etag)
        .json(json!({ "name": "Adjustments" }))
        .send()
        .await;
    assert_eq!(
        (r.status, r.body["system"].as_str()),
        (StatusCode::OK, Some("reconciliation"))
    );

    let grocer = post(
        &pool,
        &t,
        "/api/v1/counterparties",
        json!({ "name": "Grocer" }),
    )
    .await;
    assert_eq!(
        (grocer.status, &grocer.body["system"]),
        (StatusCode::CREATED, &Value::Null)
    );
    let path = format!("/api/v1/counterparties/{}", grocer.id());
    let r = call(&pool, "DELETE", &path)
        .token(&t)
        .if_match(&grocer.etag())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn categories_and_tags(pool: PgPool) {
    let (_, t) = new_token(&pool, "cli").await;
    for resource in ["categories", "tags"] {
        let list = format!("/api/v1/{resource}");
        let r = post(&pool, &t, &list, json!({ "name": "Groceries" })).await;
        assert_eq!(r.status, StatusCode::CREATED, "{resource}");
        assert_eq!(
            post(&pool, &t, &list, json!({ "name": "Groceries" }))
                .await
                .status,
            StatusCode::CONFLICT
        );
        assert_eq!(
            post(&pool, &t, &list, json!({ "name": " " })).await.status,
            StatusCode::UNPROCESSABLE_ENTITY
        );

        let path = format!("{list}/{}", r.id());
        let renamed = call(&pool, "PATCH", &path)
            .token(&t)
            .if_match(&r.etag())
            .json(json!({ "name": "Food" }))
            .send()
            .await;
        assert_eq!(renamed.body["name"], "Food");
        let gone = call(&pool, "DELETE", &path)
            .token(&t)
            .if_match(&renamed.etag())
            .send()
            .await;
        assert_eq!(gone.status, StatusCode::NO_CONTENT);
    }
}
