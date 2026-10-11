//! `/api/v1/transactions`: the design's §3.3 examples through HTTP, the
//! ledger rules as 422s, filters, pagination, replace and delete.
//! Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::{Response, call, new_token};
use serde_json::{Value, json};
use sqlx::PgPool;

/// The fictional household, created through the API.
struct Household {
    t: String,
    household: String,
    alex: String,
    usd: String,
    eur: String,
    acme: String,
    alex_checking: String,
    alex_credit: String,
    alex_brokerage: String,
    joint_savings: String,
    grocer: String,
    store: String,
    groceries: String,
}

async fn create(pool: &PgPool, t: &str, path: &str, body: Value) -> String {
    let r = call(pool, "POST", path).token(t).json(body).send().await;
    assert_eq!(r.status, StatusCode::CREATED, "{path}: {}", r.body);
    r.id()
}

impl Household {
    async fn new(pool: &PgPool) -> Self {
        let (_, t) = new_token(pool, "cli").await;
        let party = |name: &str, kind: &str| json!({ "name": name, "kind": kind });
        let household = create(pool, &t, "/api/v1/parties", party("Household", "household")).await;
        let alex = create(pool, &t, "/api/v1/parties", party("Alex", "person")).await;
        let commodity = |code: &str, kind: &str, precision: u32| json!({ "code": code, "kind": kind, "precision": precision });
        let usd = create(pool, &t, "/api/v1/commodities", commodity("USD", "fiat", 2)).await;
        let eur = create(pool, &t, "/api/v1/commodities", commodity("EUR", "fiat", 2)).await;
        let acme = create(
            pool,
            &t,
            "/api/v1/commodities",
            commodity("ACME", "security", 0),
        )
        .await;
        let account = |name: &str, kind: &str, owner: &str| json!({ "name": name, "kind": kind, "owner_id": owner });
        let alex_checking = create(
            pool,
            &t,
            "/api/v1/accounts",
            account("Alex Checking", "checking", &alex),
        )
        .await;
        let alex_credit = create(
            pool,
            &t,
            "/api/v1/accounts",
            account("Alex Credit", "credit_card", &alex),
        )
        .await;
        let alex_brokerage = create(
            pool,
            &t,
            "/api/v1/accounts",
            account("Alex Brokerage", "brokerage", &alex),
        )
        .await;
        let joint_savings = create(
            pool,
            &t,
            "/api/v1/accounts",
            account("Joint Savings", "savings", &household),
        )
        .await;
        let grocer = create(
            pool,
            &t,
            "/api/v1/counterparties",
            json!({ "name": "Grocer" }),
        )
        .await;
        let store = create(
            pool,
            &t,
            "/api/v1/counterparties",
            json!({ "name": "Store" }),
        )
        .await;
        let groceries = create(
            pool,
            &t,
            "/api/v1/categories",
            json!({ "name": "Groceries" }),
        )
        .await;
        Self {
            t,
            household,
            alex,
            usd,
            eur,
            acme,
            alex_checking,
            alex_credit,
            alex_brokerage,
            joint_savings,
            grocer,
            store,
            groceries,
        }
    }

    async fn post(&self, pool: &PgPool, body: Value) -> Response {
        call(pool, "POST", "/api/v1/transactions")
            .token(&self.t)
            .json(body)
            .send()
            .await
    }
}

fn account(id: &str, commodity: &str, qty: &str) -> Value {
    json!({ "account_id": id, "commodity_id": commodity, "quantity": qty })
}

fn external(id: &str, commodity: &str, qty: &str) -> Value {
    json!({ "counterparty_id": id, "commodity_id": commodity, "quantity": qty })
}

fn tx(date: &str, description: &str, splits: Value) -> Value {
    json!({ "date": date, "description": description, "splits": splits })
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn kinds_are_derived(pool: PgPool) {
    let h = Household::new(&pool).await;

    let r = h
        .post(
            &pool,
            json!({
                "date": "2026-03-14",
                "description": "GROCER #12",
                "tags": ["weekly", "food", "weekly"],
                "splits": [{
                    "from": account(&h.alex_credit, &h.usd, "80.00"),
                    "to": external(&h.grocer, &h.usd, "80.00"),
                    "purpose_id": h.household,
                    "category_id": h.groceries,
                }],
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let split = &r.body["splits"][0];
    assert_eq!(
        (split["kind"].as_str(), split["liquidation"].as_bool()),
        (Some("expense"), Some(false))
    );
    // Amounts come back as the exact strings that went in.
    assert_eq!(split["from"]["quantity"], "80.00");
    assert_eq!(r.body["tags"], json!(["food", "weekly"]));
    assert_eq!(r.body["date"], "2026-03-14");

    let cases = [
        // Card payment.
        (
            account(&h.alex_checking, &h.usd, "500"),
            account(&h.alex_credit, &h.usd, "500"),
            "transfer",
            false,
        ),
        // Refund.
        (
            external(&h.store, &h.usd, "30"),
            account(&h.alex_credit, &h.usd, "30"),
            "income",
            false,
        ),
        // Sell stock inside one account.
        (
            account(&h.alex_brokerage, &h.acme, "10"),
            account(&h.alex_brokerage, &h.usd, "1500"),
            "conversion",
            false,
        ),
        // FX exchange.
        (
            account(&h.alex_checking, &h.usd, "1100"),
            account(&h.alex_checking, &h.eur, "1000"),
            "conversion",
            false,
        ),
        // Liquidation into a shared account.
        (
            account(&h.alex_brokerage, &h.usd, "10000"),
            account(&h.joint_savings, &h.usd, "10000"),
            "transfer",
            true,
        ),
    ];
    for (from, to, kind, liquidation) in cases {
        let r = h
            .post(
                &pool,
                tx("2026-03-15", "Case", json!([{ "from": from, "to": to }])),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{kind}: {}", r.body);
        assert_eq!(r.body["splits"][0]["kind"], kind);
        assert_eq!(r.body["splits"][0]["liquidation"], liquidation);
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn ledger_rules_are_422s_naming_the_split(pool: PgPool) {
    let h = Household::new(&pool).await;
    let ok = json!({ "from": account(&h.alex_credit, &h.usd, "5"), "to": external(&h.grocer, &h.usd, "5") });

    let bad = [
        (
            json!({ "from": external(&h.store, &h.usd, "10"), "to": external(&h.grocer, &h.usd, "10") }),
            "doesn't involve",
        ),
        (
            json!({ "from": account(&h.alex_checking, &h.usd, "10"), "to": account(&h.alex_credit, &h.usd, "9.99") }),
            "same commodity",
        ),
        (
            json!({ "from": account(&h.alex_checking, &h.usd, "10"), "to": account(&h.alex_checking, &h.usd, "10") }),
            "same account",
        ),
        (
            json!({ "from": account(&h.alex_credit, &h.usd, "80.005"), "to": external(&h.grocer, &h.usd, "80.005") }),
            "decimal places",
        ),
        (
            json!({ "from": account(&h.alex_credit, &h.usd, "0"), "to": external(&h.grocer, &h.usd, "0") }),
            "positive",
        ),
        (
            json!({ "from": account(&h.alex_checking, &h.usd, "5"), "to": account(&h.alex_credit, &h.usd, "5"), "purpose_id": h.alex }),
            "purpose",
        ),
        (
            json!({ "from": account(&h.alex_checking, &h.usd, "5"), "to": account(&h.alex_credit, &h.usd, "5"), "category_id": h.groceries }),
            "category",
        ),
        (
            json!({ "from": account("00000000-0000-0000-0000-000000000000", &h.usd, "5"), "to": external(&h.grocer, &h.usd, "5") }),
            "unknown account",
        ),
    ];
    for (split, expect) in bad {
        let r = h
            .post(&pool, tx("2026-03-14", "Bad", json!([ok.clone(), split])))
            .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{expect}: {}",
            r.body
        );
        assert_eq!(
            r.body["type"], "urn:exchequer:problem:ledger-rule",
            "{expect}"
        );
        let detail = r.body["detail"].as_str().unwrap();
        assert!(
            detail.starts_with("split 1: ") && detail.contains(expect),
            "{detail}"
        );
    }

    // Malformed requests: a JSON number amount, a leg with both sides, no splits.
    let number = json!([{ "from": { "account_id": h.alex_credit, "commodity_id": h.usd, "quantity": 5 }, "to": external(&h.grocer, &h.usd, "5") }]);
    let both = json!([{ "from": { "account_id": h.alex_credit, "counterparty_id": h.grocer, "commodity_id": h.usd, "quantity": "5" }, "to": external(&h.grocer, &h.usd, "5") }]);
    for splits in [number, both, json!([])] {
        let r = h.post(&pool, tx("2026-03-14", "Bad", splits)).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
        assert_eq!(r.body["type"], "urn:exchequer:problem:invalid-request");
    }

    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM transaction")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "nothing invalid was stored");
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn list_filter_and_paginate(pool: PgPool) {
    let h = Household::new(&pool).await;
    for day in 1..=5 {
        let from = if day % 2 == 0 {
            &h.alex_checking
        } else {
            &h.alex_credit
        };
        let r = h.post(&pool, tx(&format!("2026-03-0{day}"), "Grocer", json!([{
            "from": account(from, &h.usd, "10"), "to": external(&h.grocer, &h.usd, "10"),
        }]))).await;
        assert_eq!(r.status, StatusCode::CREATED);
    }
    let dates = |r: &Response| -> Vec<String> {
        r.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["date"].as_str().unwrap().to_owned())
            .collect()
    };

    let get = |q: String| {
        call(&pool, "GET", format!("/api/v1/transactions?{q}"))
            .token(&h.t)
            .send()
    };
    let r = get(String::new()).await;
    assert_eq!(
        dates(&r),
        [
            "2026-03-05",
            "2026-03-04",
            "2026-03-03",
            "2026-03-02",
            "2026-03-01"
        ]
    );
    let r = get(format!("account_id={}", h.alex_checking)).await;
    assert_eq!(dates(&r), ["2026-03-04", "2026-03-02"]);
    let r = get("from=2026-03-02&to=2026-03-03".to_owned()).await;
    assert_eq!(dates(&r), ["2026-03-03", "2026-03-02"]);

    let first = get("limit=2".to_owned()).await;
    assert_eq!(dates(&first), ["2026-03-05", "2026-03-04"]);
    let cursor = first.body["next_cursor"].as_str().unwrap();
    let second = get(format!("limit=2&cursor={cursor}")).await;
    assert_eq!(dates(&second), ["2026-03-03", "2026-03-02"]);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn replace_and_delete(pool: PgPool) {
    let h = Household::new(&pool).await;
    let r = h.post(&pool, tx("2026-03-14", "Warehouse", json!([{
        "from": account(&h.alex_credit, &h.usd, "165"), "to": external(&h.store, &h.usd, "165"),
    }]))).await;
    let path = format!("/api/v1/transactions/{}", r.id());
    let v1 = r.etag();

    // Split it in two (groceries and home supplies) by replacing it whole.
    let split_up = json!({
        "date": "2026-03-14",
        "description": "Warehouse",
        "splits": [
            { "from": account(&h.alex_credit, &h.usd, "120"), "to": external(&h.store, &h.usd, "120"), "category_id": h.groceries },
            { "from": account(&h.alex_credit, &h.usd, "45"), "to": external(&h.store, &h.usd, "45") },
        ],
    });
    let r = call(&pool, "PUT", &path)
        .token(&h.t)
        .json(split_up.clone())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_REQUIRED);
    let r = call(&pool, "PUT", &path)
        .token(&h.t)
        .if_match(&v1)
        .json(split_up.clone())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["splits"].as_array().unwrap().len(), 2);
    let v2 = r.etag();
    let r = call(&pool, "PUT", &path)
        .token(&h.t)
        .if_match(&v1)
        .json(split_up)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED);

    let r = call(&pool, "GET", &path).token(&h.t).send().await;
    assert_eq!(
        (r.etag(), r.body["splits"][0]["category_id"].as_str()),
        (v2.clone(), Some(h.groceries.as_str()))
    );

    // An account with splits can't be deleted.
    let acct = call(&pool, "GET", format!("/api/v1/accounts/{}", h.alex_credit))
        .token(&h.t)
        .send()
        .await;
    let r = call(
        &pool,
        "DELETE",
        format!("/api/v1/accounts/{}", h.alex_credit),
    )
    .token(&h.t)
    .if_match(&acct.etag())
    .send()
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    let r = call(&pool, "DELETE", &path)
        .token(&h.t)
        .if_match(&v2)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        call(&pool, "GET", &path).token(&h.t).send().await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn external_refs_are_unique_and_findable(pool: PgPool) {
    let h = Household::new(&pool).await;
    let body = |refs: Value| {
        json!({
            "date": "2026-03-14",
            "description": "GROCER #12",
            "external_refs": refs,
            "splits": [{
                "from": account(&h.alex_credit, &h.usd, "80.00"),
                "to": external(&h.grocer, &h.usd, "80.00"),
            }],
        })
    };

    let r = h
        .post(
            &pool,
            body(json!([{ "source": "firefly", "ref": "41" }, { "source": "bank", "ref": "X-1" }])),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    // Sorted by source, then ref.
    assert_eq!(
        r.body["external_refs"],
        json!([{ "source": "bank", "ref": "X-1" }, { "source": "firefly", "ref": "41" }])
    );
    let id = r.id();

    // The same reference can't be stored twice.
    let r = h
        .post(&pool, body(json!([{ "source": "firefly", "ref": "41" }])))
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    // Same ref, other source: fine.
    let r = h
        .post(&pool, body(json!([{ "source": "other", "ref": "41" }])))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);

    for bad in [
        json!([{ "source": "Firefly", "ref": "1" }]),
        json!([{ "source": "firefly", "ref": "" }]),
        json!([{ "source": "firefly" }]),
    ] {
        let r = h.post(&pool, body(bad.clone())).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }

    let find = |source: &str, reference: &str| {
        call(
            &pool,
            "GET",
            format!("/api/v1/transactions?external_source={source}&external_ref={reference}"),
        )
        .token(&h.t)
        .send()
    };
    let r = find("firefly", "41").await;
    assert_eq!(r.status, StatusCode::OK);
    let items = r.body["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| i["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [id.as_str()]
    );
    assert!(
        find("firefly", "42").await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let r = call(&pool, "GET", "/api/v1/transactions?external_source=firefly")
        .token(&h.t)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Replacing a transaction replaces its references too.
    let path = format!("/api/v1/transactions/{id}");
    let current = call(&pool, "GET", &path).token(&h.t).send().await;
    let r = call(&pool, "PUT", &path)
        .token(&h.t)
        .if_match(&current.etag())
        .json(body(json!([{ "source": "firefly", "ref": "41" }])))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        r.body["external_refs"],
        json!([{ "source": "firefly", "ref": "41" }])
    );
    assert!(
        find("bank", "X-1").await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
