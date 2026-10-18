//! `/api/v1/query` and `/api/v1/reports` on the fictional household.
//! Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::H;
use serde_json::{Value, json};
use sqlx::PgPool;

async fn query(h: &H, pool: &PgPool, body: Value) -> Vec<Value> {
    let r = h
        .send(pool, "POST", "/api/v1/query", Some(body.clone()))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{body}: {}", r.body);
    r.body["rows"].as_array().unwrap().clone()
}

/// June: groceries for the household (80, tagged), a personal purchase (20),
/// salary for the household (3000), a card payment; July: groceries (40).
async fn june_and_july(h: &H, pool: &PgPool) -> String {
    let groceries = {
        let r = h
            .send(
                pool,
                "POST",
                "/api/v1/categories",
                Some(json!({ "name": "Groceries" })),
            )
            .await;
        r.id()
    };
    let store = ("counterparty_id", h.store.as_str());
    let household = h.household.clone();
    for (date, from, to, qty, extra) in [
        (
            "2026-06-03",
            h.acct(&h.alex_credit),
            store,
            "80.00",
            json!({ "purpose_id": household, "category_id": groceries }),
        ),
        (
            "2026-06-10",
            h.acct(&h.alex_credit),
            store,
            "20.00",
            json!({ "purpose_id": h.alex }),
        ),
        (
            "2026-06-15",
            store,
            h.acct(&h.alex_checking),
            "3000.00",
            json!({ "purpose_id": household }),
        ),
        (
            "2026-06-20",
            h.acct(&h.alex_checking),
            h.acct(&h.alex_credit),
            "100.00",
            json!({}),
        ),
        (
            "2026-07-01",
            h.acct(&h.joint_savings),
            store,
            "40.00",
            json!({ "purpose_id": household, "category_id": groceries }),
        ),
    ] {
        let r = h.tx(pool, date, from, to, qty, extra).await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    }
    // Tag the first purchase.
    let r = h
        .send(
            pool,
            "GET",
            "/api/v1/transactions?from=2026-06-03&to=2026-06-03",
            None,
        )
        .await;
    let tx = &r.body["items"][0];
    let path = format!("/api/v1/transactions/{}", tx["id"].as_str().unwrap());
    let etag = h.send(pool, "GET", &path, None).await.etag();
    let mut body = json!({ "date": tx["date"], "description": "groceries",
        "tags": ["food"], "splits": tx["splits"] });
    for s in body["splits"].as_array_mut().unwrap() {
        let s = s.as_object_mut().unwrap();
        s.remove("kind");
    }
    let r = common::call(pool, "PUT", &path)
        .token(&h.t)
        .if_match(&etag)
        .json(body)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    groceries
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn flows(pool: PgPool) {
    let h = H::new(&pool).await;
    let groceries = june_and_july(&h, &pool).await;

    // Spending by purpose and month.
    let rows = query(
        &h,
        &pool,
        json!({ "from": "2026-06-01", "to": "2026-07-31", "bucket": "month",
                "kinds": ["expense"], "group_by": ["purpose"] }),
    )
    .await;
    let found: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| {
            (
                r["bucket"].as_str().unwrap(),
                r["purpose_id"].as_str().unwrap(),
                r["amount"].as_str().unwrap(),
            )
        })
        .collect();
    let mut expected = vec![
        ("2026-06-01", h.household.as_str(), "-80.00"),
        ("2026-06-01", h.alex.as_str(), "-20.00"),
        ("2026-07-01", h.household.as_str(), "-40.00"),
    ];
    expected.sort_unstable();
    let mut found_sorted = found.clone();
    found_sorted.sort_unstable();
    assert_eq!(found_sorted, expected);

    // By category over the whole range: a missing category is null.
    let rows = query(
        &h,
        &pool,
        json!({ "kinds": ["expense"], "group_by": ["category"] }),
    )
    .await;
    assert_eq!(rows.len(), 2);
    let by: Vec<(Value, &str)> = rows
        .iter()
        .map(|r| (r["category_id"].clone(), r["amount"].as_str().unwrap()))
        .collect();
    assert!(by.contains(&(json!(groceries), "-120.00")));
    assert!(by.contains(&(Value::Null, "-20.00")));
    assert!(rows[0].get("purpose_id").is_none(), "not grouped: absent");

    // Income vs. expense, one row each; transfers net to zero.
    let rows = query(
        &h,
        &pool,
        json!({ "group_by": ["kind"], "to": "2026-06-30" }),
    )
    .await;
    let kinds: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r["kind"].as_str().unwrap(), r["amount"].as_str().unwrap()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("expense", "-100.00"),
            ("income", "3000.00"),
            ("transfer", "0.00")
        ]
    );

    // Filters: tags, text, account.
    let rows = query(&h, &pool, json!({ "tags": ["food"] })).await;
    assert_eq!(rows[0]["amount"], "-80.00");
    let rows = query(&h, &pool, json!({ "text": "GROCERIES" })).await;
    assert_eq!(rows[0]["amount"], "-80.00");
    let rows = query(
        &h,
        &pool,
        json!({ "account_ids": [h.alex_credit], "group_by": ["account"] }),
    )
    .await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["amount"], "0.00", "-80 - 20 + 100");

    // Bad queries.
    for bad in [
        json!({ "measure": "balance" }),
        json!({ "measure": "balance", "to": "2026-06-30", "group_by": ["category"] }),
        json!({ "measure": "balance", "to": "2026-06-30", "kinds": ["expense"] }),
        json!({ "group_by": ["kind", "kind"] }),
        json!({ "from": "2026-07-01", "to": "2026-06-01" }),
        json!({ "group_by": ["payee"] }),
    ] {
        let r = h
            .send(&pool, "POST", "/api/v1/query", Some(bad.clone()))
            .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.body
        );
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn balances(pool: PgPool) {
    let h = H::new(&pool).await;
    june_and_july(&h, &pool).await;

    // Balance history per account at each month end, from before any data.
    let rows = query(
        &h,
        &pool,
        json!({ "measure": "balance", "from": "2026-05-01", "to": "2026-07-31",
                "bucket": "month", "group_by": ["account"], "account_ids": [h.alex_checking] }),
    )
    .await;
    let found: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r["bucket"].as_str().unwrap(), r["amount"].as_str().unwrap()))
        .collect();
    assert_eq!(
        found,
        [
            ("2026-05-01", "0.00"),
            ("2026-06-01", "2900.00"),
            ("2026-07-01", "2900.00")
        ]
    );

    // Balances on a date, opening history included; zero balances are left
    // out (Alex Credit was paid off).
    let r = h
        .send(
            &pool,
            "GET",
            "/api/v1/reports/balances?date=2026-07-31",
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let mut b: Vec<(&str, &str)> = r.body["balances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["account_id"].as_str().unwrap(),
                x["balance"].as_str().unwrap(),
            )
        })
        .collect();
    b.sort_unstable();
    let mut expected = vec![
        (h.alex_checking.as_str(), "2900.00"),
        (h.joint_savings.as_str(), "-40.00"),
    ];
    expected.sort_unstable();
    assert_eq!(b, expected);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn household_pool(pool: PgPool) {
    let h = H::new(&pool).await;
    let groceries = june_and_july(&h, &pool).await;
    // Money out of an investment into Joint Savings (income), and funding
    // for Studio.
    h.tx(
        &pool,
        "2026-06-25",
        h.acct(&h.alex_brokerage),
        h.acct(&h.joint_savings),
        "500.00",
        json!({ "purpose_id": h.household }),
    )
    .await;
    h.put_params(
        &pool,
        "2026-06",
        json!({ "stipends": [{ "party_id": h.alex, "amount": "650.00" }] }),
    )
    .await;

    let r = h
        .send(&pool, "GET", "/api/v1/reports/household-pool/2026-06", None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let p = &r.body;
    assert_eq!(
        p["income"],
        json!([{ "category_id": null, "amount": "3500.00" }])
    );
    assert_eq!(
        p["expenses"],
        json!([{ "category_id": groceries, "amount": "-80.00" }])
    );
    assert!(p.get("liquidations").is_none());
    assert_eq!(
        p["stipends"],
        json!([{ "party_id": h.alex, "amount": "650.00" }])
    );
    assert_eq!(
        p["business_funding"],
        json!([{ "party_id": h.studio, "amount": "0.00" }])
    );
    assert_eq!(p["shared_savings_change"], "500.00");
}
