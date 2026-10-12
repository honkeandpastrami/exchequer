//! `/api/v1/settlements`: USD, a month's parameters,
//! statements, implied stipends, closing and reopening, and the closed-month
//! guard on transactions. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::{H, edge};
use common::{Response, call};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn settlement_is_in_usd(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = h
        .send(&pool, "GET", "/api/v1/settlements/2026-06/statement", None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["currency_id"], json!(h.usd));
    assert_eq!(r.body["edges"].as_array().unwrap().len(), 2);

    // Parties have no currency.
    let path = format!("/api/v1/parties/{}", h.household);
    let etag = h.send(&pool, "GET", &path, None).await.etag();
    let r = call(&pool, "PATCH", &path)
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "currency_id": h.usd }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);

    // Without a USD commodity there's no statement.
    let path = format!("/api/v1/commodities/{}", h.usd);
    let etag = h.send(&pool, "GET", &path, None).await.etag();
    let r = call(&pool, "PATCH", &path)
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "code": "USX" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let r = h
        .send(&pool, "GET", "/api/v1/settlements/2026-06/statement", None)
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn statement_and_implied_stipend(pool: PgPool) {
    let h = H::new(&pool).await;
    let household = json!({ "purpose_id": h.household });
    let store = ("counterparty_id", h.store.as_str());

    // June: Alex buys groceries for the Household; Studio gets funding.
    let r = h
        .tx(
            &pool,
            "2026-06-05",
            h.acct(&h.alex_credit),
            store,
            "80.00",
            household.clone(),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    h.tx(
        &pool,
        "2026-06-20",
        h.acct(&h.alex_credit),
        store,
        "20.00",
        household.clone(),
    )
    .await;
    // Settled early in July: Household → Alex, stipend 500 + 100.
    h.tx(
        &pool,
        "2026-07-02",
        h.acct(&h.joint_savings),
        h.acct(&h.alex_checking),
        "600.00",
        json!({}),
    )
    .await;

    let r = h
        .put_params(
            &pool,
            "2026-06",
            json!({ "stipends": [{ "party_id": h.alex, "amount": "500.00" }],
                    "funders": [{ "business_id": h.studio, "funder_id": h.household }] }),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["params_from"], "2026-06");

    let st = h
        .send(&pool, "GET", "/api/v1/settlements/2026-06/statement", None)
        .await
        .body;
    let alex = edge(&st, &h.alex);
    assert_eq!(alex["parent_id"], json!(h.household));
    assert_eq!(alex["claims"], "-100.00");
    assert_eq!(alex["stipend"], "500.00");
    assert_eq!(alex["due"], "-600.00");
    assert_eq!(alex["paid"], "-600.00");
    assert_eq!(alex["outstanding"], "0.00");
    assert_eq!(alex["implied_stipend"], "500.00");
    assert_eq!(alex["lines"].as_array().unwrap().len(), 3);
    assert_eq!(alex["lines"][2]["kind"], "payment");

    // History over several months: parameters carry forward, no lines.
    let r = h
        .send(
            &pool,
            "GET",
            "/api/v1/settlements?from=2026-05&to=2026-07",
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let months = r.body["months"].as_array().unwrap();
    assert_eq!(months.len(), 3);
    assert!(months[0].get("params_from").is_none());
    assert_eq!(months[2]["params_from"], "2026-06");
    assert_eq!(edge(&months[2], &h.alex)["stipend"], "500.00");
    assert!(edge(&months[1], &h.alex).get("lines").is_none());

    // Parameters are checked.
    for bad in [
        json!({ "stipends": [{ "party_id": h.studio, "amount": "1" }] }),
        json!({ "stipends": [{ "party_id": h.alex, "amount": "-1" }] }),
        json!({ "stipends": [{ "party_id": h.alex, "amount": "1.001" }] }),
        json!({ "funders": [{ "business_id": h.alex, "funder_id": h.household }] }),
        json!({ "funders": [{ "business_id": h.studio, "funder_id": h.studio }] }),
    ] {
        let r = h.put_params(&pool, "2026-07", bad.clone()).await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.body
        );
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn settles_and_liquidations(pool: PgPool) {
    let h = H::new(&pool).await;

    // A payment linked by hand to the month it's made in.
    let r = h
        .tx(
            &pool,
            "2026-03-12",
            h.acct(&h.joint_savings),
            h.acct(&h.alex_checking),
            "70.00",
            json!({ "settles": "2026-03" }),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["splits"][0]["settles"], "2026-03");
    // Only settlement payments take one.
    let r = h
        .tx(
            &pool,
            "2026-03-12",
            h.acct(&h.alex_checking),
            h.acct(&h.alex_credit),
            "70.00",
            json!({ "settles": "2026-03" }),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    let st = h
        .send(&pool, "GET", "/api/v1/settlements/2026-03/statement", None)
        .await
        .body;
    assert_eq!(edge(&st, &h.alex)["paid"], "-70.00");

    // A liquidation into Alex's account is held in transit for the Household.
    h.tx(
        &pool,
        "2026-05-20",
        h.acct(&h.alex_brokerage),
        h.acct(&h.alex_checking),
        "1000.00",
        json!({}),
    )
    .await;
    let st = h
        .send(&pool, "GET", "/api/v1/settlements/2026-05/statement", None)
        .await
        .body;
    let alex = edge(&st, &h.alex);
    assert_eq!(alex["in_transit"][0]["amount"], "1000.00");
    assert_eq!(alex["claims"], "0.00");
    // The Household's 70 owed nothing, so Alex owes it back.
    assert_eq!(alex["to_settle"], "70.00");
    assert_eq!(alex["outstanding"], "1070.00");
}

async fn july_alex(h: &H, pool: &PgPool) -> serde_json::Value {
    let st = h
        .send(pool, "GET", "/api/v1/settlements/2026-07/statement", None)
        .await
        .body;
    edge(&st, &h.alex).clone()
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn corrections_carry_forward(pool: PgPool) {
    let h = H::new(&pool).await;
    let household = json!({ "purpose_id": h.household });
    let store = ("counterparty_id", h.store.as_str());
    let card = h.acct(&h.alex_credit);
    let (joint, checking) = (h.acct(&h.joint_savings), h.acct(&h.alex_checking));
    h.tx(
        &pool,
        "2026-06-05",
        card,
        store,
        "100.00",
        household.clone(),
    )
    .await;
    h.tx(&pool, "2026-07-02", joint, checking, "100.00", json!({}))
        .await;
    h.put_params(
        &pool,
        "2026-06",
        json!({ "stipends": [{ "party_id": h.alex, "amount": "0" }],
                "funders": [{ "business_id": h.studio, "funder_id": h.household }] }),
    )
    .await;
    assert_eq!(
        h.post_month(&pool, "2026-06", "close").await.status,
        StatusCode::OK
    );
    let alex = july_alex(&h, &pool).await;
    assert_eq!(alex["carried"], "0.00");
    assert_eq!(alex["to_settle"], "0.00");

    // A charge June missed: reopen, add it, close again. June's due
    // changes; what was paid doesn't, so the difference is carried.
    h.post_month(&pool, "2026-06", "reopen").await;
    h.tx(&pool, "2026-06-30", card, store, "30.00", household.clone())
        .await;
    h.post_month(&pool, "2026-06", "close").await;
    let alex = july_alex(&h, &pool).await;
    assert_eq!(alex["carried"], "-30.00");
    assert_eq!(alex["due"], "0.00");
    assert_eq!(alex["to_settle"], "-30.00", "the Household owes Alex 30");

    // Paid whenever: here with July's settlement.
    h.tx(&pool, "2026-08-03", joint, checking, "30.00", json!({}))
        .await;
    let alex = july_alex(&h, &pool).await;
    assert_eq!(alex["carried"], "-30.00");
    assert_eq!(alex["paid"], "-30.00");
    assert_eq!(alex["outstanding"], "0.00");
    assert_eq!(
        alex["implied_stipend"], "30.00",
        "a catch-up, not a stipend"
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn close_and_reopen(pool: PgPool) {
    let h = H::new(&pool).await;
    let store = ("counterparty_id", h.store.as_str());
    let r = h
        .tx(
            &pool,
            "2026-06-05",
            h.acct(&h.alex_credit),
            store,
            "80.00",
            json!({}),
        )
        .await;
    let tx_path = format!("/api/v1/transactions/{}", r.id());
    let tx_etag = r.etag();
    let tx_body = json!({ "date": "2026-06-05", "description": "y", "splits": [
        { "from": h.leg(h.acct(&h.alex_credit), "81.00"), "to": h.leg(store, "81.00") }] });

    // Closing needs every person's stipend and every business's funder.
    let r = h.post_month(&pool, "2026-06", "close").await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    h.put_params(
        &pool,
        "2026-06",
        json!({ "stipends": [{ "party_id": h.alex, "amount": "0" }] }),
    )
    .await;
    let r = h.post_month(&pool, "2026-06", "close").await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert!(
        r.body["detail"]
            .as_str()
            .unwrap()
            .contains("funder for Studio")
    );
    h.put_params(
        &pool,
        "2026-06",
        json!({ "stipends": [{ "party_id": h.alex, "amount": "0" }],
                "funders": [{ "business_id": h.studio, "funder_id": h.household }] }),
    )
    .await;
    let r = h.post_month(&pool, "2026-06", "close").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["closed"], true);
    assert!(r.body["closed_at"].is_string());

    // Closed: transactions dated in it can't change, and neither can its
    // parameters.
    let closed = |r: &Response| {
        assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
        assert_eq!(r.body["type"], "urn:exchequer:problem:month-closed");
        assert_eq!(r.body["detail"], "2026-06 is closed; reopen it to edit");
    };
    closed(
        &h.tx(
            &pool,
            "2026-06-30",
            h.acct(&h.alex_credit),
            store,
            "1.00",
            json!({}),
        )
        .await,
    );
    closed(
        &call(&pool, "PUT", &tx_path)
            .token(&h.t)
            .if_match(&tx_etag)
            .json(tx_body.clone())
            .send()
            .await,
    );
    closed(
        &call(&pool, "DELETE", &tx_path)
            .token(&h.t)
            .if_match(&tx_etag)
            .send()
            .await,
    );
    closed(&h.put_params(&pool, "2026-06", json!({})).await);
    // Other months are fine, and statements still compute.
    let r = h
        .tx(
            &pool,
            "2026-07-01",
            h.acct(&h.alex_credit),
            store,
            "1.00",
            json!({}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let st = h
        .send(&pool, "GET", "/api/v1/settlements/2026-06/statement", None)
        .await;
    assert_eq!(st.body["closed"], true);
    assert_eq!(
        h.post_month(&pool, "2026-06", "close").await.status,
        StatusCode::CONFLICT
    );

    // Reopen: editable again.
    let r = h.post_month(&pool, "2026-06", "reopen").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["closed"], false);
    let r = call(&pool, "PUT", &tx_path)
        .token(&h.t)
        .if_match(&tx_etag)
        .json(tx_body)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        h.post_month(&pool, "2026-06", "reopen").await.status,
        StatusCode::CONFLICT
    );

    // A stale ETag is refused.
    let r = call(&pool, "POST", "/api/v1/settlements/2026-06/close")
        .token(&h.t)
        .if_match("\"1\"")
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED, "{}", r.body);

    // Bad months are 422s.
    let r = h
        .send(&pool, "GET", "/api/v1/settlements/2026-13", None)
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
}
