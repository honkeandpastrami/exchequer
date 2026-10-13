//! Suggestions in staging (design §8): a statement's suggested account and
//! rows' suggested labels, how review shows them, what commit takes and what
//! it records. The suggesters themselves come later; these tests store
//! suggestions directly. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::{H, create};
use common::{Response, call};
use serde_json::{Value, json};
use sqlx::PgPool;
use store::imports::{AccountSuggestion, SuggestionRow};
use uuid::Uuid;

const CSV: &str = "Date,Description,Amount\n\
03/01/2026,COFFEE,-4.50\n\
03/02/2026,GROCER,-80.00\n";

async fn read(pool: &PgPool, h: &H, id: &str) -> Response {
    h.send(pool, "GET", &format!("/api/v1/imports/{id}"), None)
        .await
}

async fn rows(pool: &PgPool, h: &H, id: &str) -> Vec<Value> {
    let r = h
        .send(pool, "GET", &format!("/api/v1/imports/{id}/rows"), None)
        .await;
    r.body["items"].as_array().unwrap().clone()
}

async fn patch(pool: &PgPool, h: &H, path: String, body: Value) -> Response {
    let id = path.split('/').nth(4).unwrap().to_owned();
    let etag = read(pool, h, &id).await.etag();
    let r = call(pool, "PATCH", path)
        .token(&h.t)
        .if_match(&etag)
        .json(body)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    r
}

async fn commit(pool: &PgPool, h: &H, id: &str) -> Response {
    let etag = read(pool, h, id).await.etag();
    call(pool, "POST", format!("/api/v1/imports/{id}/commit"))
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await
}

async fn suggester(pool: &PgPool, kind: &str, name: &str) -> Uuid {
    let mut db = pool.acquire().await.unwrap();
    store::imports::suggester(&mut db, kind, name, "1", &json!({ "snapshot": "test" }))
        .await
        .unwrap()
}

fn uuid(s: &str) -> Uuid {
    s.parse().unwrap()
}

fn label(
    position: i32,
    label: &str,
    value: Option<&str>,
    by: Uuid,
    confidence: i16,
) -> SuggestionRow {
    SuggestionRow {
        statement: 0,
        position,
        label: label.to_owned(),
        value: value.map(uuid),
        account_id: None,
        suggester_id: by,
        confidence,
        reason: format!("like a past {label}"),
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_suggested_account_counts_only_when_confident(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = call(&pool, "POST", "/api/v1/imports?file_name=march.csv")
        .token(&h.t)
        .bytes(CSV.as_bytes().to_vec())
        .send()
        .await;
    let id = r.id();
    let local = suggester(&pool, "local", "lexical-knn").await;
    // The same identity is the same suggester.
    assert_eq!(suggester(&pool, "local", "lexical-knn").await, local);

    let suggest = |confidence: i16| {
        let pool = pool.clone();
        let (id, account) = (uuid(&id), uuid(&h.alex_checking));
        async move {
            store::imports::set_account_suggestion(
                &pool,
                id,
                0,
                Some(&AccountSuggestion {
                    account_id: account,
                    suggester_id: local,
                    confidence,
                    reason: "its rows look like this account's".to_owned(),
                }),
            )
            .await
            .unwrap();
        }
    };
    // Not confident: shown, but the rows still wait for an account.
    suggest(600).await;
    let r = patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/statements/0"),
        json!({ "account_id": null }),
    )
    .await;
    let s = &r.body["statements"][0];
    assert_eq!(s.get("account_id"), Some(&Value::Null));
    assert_eq!(
        (
            &s["suggested_account"]["account_id"],
            &s["suggested_account"]["confident"],
            &s["suggested_account"]["suggester"]["kind"],
            &s["counts"]["needs_account"]
        ),
        (
            &json!(h.alex_checking),
            &json!(false),
            &json!("local"),
            &json!(2)
        )
    );
    let r = commit(&pool, &h, &id).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);

    // Confident: the rows are worked out against it.
    suggest(950).await;
    let r = patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/statements/0"),
        json!({ "account_id": null }),
    )
    .await;
    let s = &r.body["statements"][0];
    assert_eq!(
        (
            &s["account_id"],
            &s["account_suggested"],
            &s["counts"]["new"]
        ),
        (&json!(h.alex_checking), &json!(true), &json!(2))
    );
    // Picking it makes it the statement's own.
    let r = patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/statements/0"),
        json!({ "account_id": h.alex_checking }),
    )
    .await;
    assert_eq!(r.body["statements"][0]["account_suggested"], json!(false));
    // Taken as shown on commit, and recorded.
    patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/statements/0"),
        json!({ "account_id": null }),
    )
    .await;
    for position in [0, 1] {
        patch(
            &pool,
            &h,
            format!("/api/v1/imports/{id}/rows/0/{position}"),
            json!({ "counterparty_id": null, "category_id": null, "purpose_id": null }),
        )
        .await;
    }
    let r = commit(&pool, &h, &id).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let (label, suggested, committed, confidence): (String, Uuid, Uuid, i16) =
        sqlx::query_as("SELECT label, suggested, committed, confidence FROM suggestion_outcome")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (label.as_str(), suggested, committed, confidence),
        (
            "account",
            uuid(&h.alex_checking),
            uuid(&h.alex_checking),
            950
        )
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn confident_labels_are_committed_unless_you_choose(pool: PgPool) {
    let h = H::new(&pool).await;
    let groceries = create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Groceries" }),
    )
    .await;
    let dining = create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Dining" }),
    )
    .await;
    let r = call(
        &pool,
        "POST",
        format!("/api/v1/imports?account_id={}", h.alex_checking),
    )
    .token(&h.t)
    .bytes(CSV.as_bytes().to_vec())
    .send()
    .await;
    let id = r.id();
    let rule = suggester(&pool, "rule", "rules").await;
    let remote = suggester(&pool, "remote", "a-deployment/a-model").await;
    let mut db = pool.acquire().await.unwrap();
    store::imports::set_suggestions(
        &mut db,
        uuid(&id),
        0,
        &[
            // The coffee: all three confident, one of them "none".
            label(0, "counterparty", Some(&h.store), rule, 1000),
            label(0, "category", Some(&dining), rule, 1000),
            label(0, "purpose", None, rule, 1000),
            // The grocer: a confident counterparty, an unsure category, no
            // purpose at all.
            label(1, "counterparty", Some(&h.store), remote, 930),
            label(1, "category", Some(&groceries), remote, 610),
        ],
    )
    .await
    .unwrap();
    drop(db);

    let all = rows(&pool, &h, &id).await;
    assert_eq!(
        (
            &all[0]["counterparty_id"],
            &all[0]["category_id"],
            all[0].get("purpose_id"),
            &all[0]["missing"],
            &all[0]["chosen"]
        ),
        (
            &json!(h.store),
            &json!(dining),
            None,
            &json!([]),
            &json!([])
        )
    );
    let s = &all[1]["suggestions"];
    assert_eq!(
        (
            &s[0]["label"],
            &s[0]["confidence"],
            &s[0]["confident"],
            &s[0]["suggester"]["name"]
        ),
        (
            &json!("category"),
            &json!(610),
            &json!(false),
            &json!("a-deployment/a-model")
        )
    );
    assert_eq!(
        (
            &all[1]["counterparty_id"],
            all[1].get("category_id"),
            &all[1]["missing"]
        ),
        (&json!(h.store), None, &json!(["category", "purpose"]))
    );
    let r = commit(&pool, &h, &id).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert!(
        r.body["detail"]
            .as_str()
            .unwrap()
            .contains("statement 0 row 1 (category, purpose)"),
        "{}",
        r.body
    );
    // A skipped row needs nothing.
    let r = patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/rows/0/1"),
        json!({ "decision": "skip" }),
    )
    .await;
    assert_eq!(r.body["missing"], json!([]));
    patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/rows/0/1"),
        json!({ "decision": null }),
    )
    .await;
    // Accept the unsure category, choose the purpose; override the coffee's
    // category.
    patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/rows/0/1"),
        json!({ "category_id": groceries, "purpose_id": h.household }),
    )
    .await;
    let r = patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/rows/0/0"),
        json!({ "category_id": groceries }),
    )
    .await;
    assert_eq!(
        (&r.body["category_id"], &r.body["chosen"]),
        (&json!(groceries), &json!(["category"]))
    );
    let r = commit(&pool, &h, &id).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    let batch = r.body["import_batch_id"].as_str().unwrap();
    let list = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/transactions?import_batch_id={batch}"),
            None,
        )
        .await;
    let got: Vec<(Value, Value, Value)> = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            let s = &t["splits"][0];
            (
                s["to"]["counterparty_id"].clone(),
                s["category_id"].clone(),
                s.get("purpose_id").cloned().unwrap_or(Value::Null),
            )
        })
        .collect();
    assert!(
        got.contains(&(json!(h.store), json!(groceries), json!(h.household))),
        "{got:?}"
    );
    // Each suggestion is recorded with what was committed.
    let mut outcomes: Vec<(String, i16, bool)> = sqlx::query_as(
        "SELECT label, confidence, suggested IS NOT DISTINCT FROM committed
         FROM suggestion_outcome ORDER BY label, confidence",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    outcomes.sort();
    assert_eq!(
        outcomes,
        [
            ("category".into(), 610, true),
            ("category".into(), 1000, false),
            ("counterparty".into(), 930, true),
            ("counterparty".into(), 1000, true),
            ("purpose".into(), 1000, true),
        ]
    );
}

/// Ten months of history: coffee on Alex's card for the household, pay into
/// Alex's checking.
async fn history(pool: &PgPool, h: &H) -> (String, String, String) {
    let dining = create(
        pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Dining" }),
    )
    .await;
    let salary = create(
        pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Salary" }),
    )
    .await;
    let employer = create(
        pool,
        &h.t,
        "/api/v1/counterparties",
        json!({ "name": "Acme" }),
    )
    .await;
    for month in 1..=10 {
        for day in [3, 12, 21] {
            let date = format!("2025-{month:02}-{day:02}");
            let split = |from: Value, to: Value, category: &str, purpose: Option<&str>| {
                let mut s = json!({ "from": from, "to": to, "category_id": category });
                if let Some(p) = purpose {
                    s["purpose_id"] = json!(p);
                }
                s
            };
            for (description, s) in [
                (
                    format!("SQ *CORNER CAFE #04{day} {month:02}/{day:02}"),
                    split(
                        h.leg(h.acct(&h.alex_credit), "4.75"),
                        h.leg(("counterparty_id", &h.store), "4.75"),
                        &dining,
                        Some(&h.household),
                    ),
                ),
                (
                    format!("ACME PAYROLL DEP {month}{day}0001"),
                    split(
                        h.leg(("counterparty_id", &employer), "1000.00"),
                        h.leg(h.acct(&h.alex_checking), "1000.00"),
                        &salary,
                        None,
                    ),
                ),
            ] {
                let r = h
                    .send(
                        pool,
                        "POST",
                        "/api/v1/transactions",
                        Some(json!({ "date": date, "description": description, "splits": [s] })),
                    )
                    .await;
                assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
            }
        }
    }
    (dining, salary, employer)
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn local_ai_suggests_the_account_and_the_labels(pool: PgPool) {
    let h = H::new(&pool).await;
    let (dining, _, _) = history(&pool, &h).await;
    let csv = "Date,Description,Amount\n\
               11/03/2025,SQ *CORNER CAFE #0403 11/03,-4.75\n\
               11/12/2025,SQ *CORNER CAFE #0412 11/12,-5.25\n\
               11/14/2025,UNHEARD OF HARDWARE,-19.99\n";
    let r = call(&pool, "POST", "/api/v1/imports?file_name=card.csv")
        .token(&h.t)
        .bytes(csv.as_bytes().to_vec())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let id = r.id();
    // The rows look like the card's: its account, confidently, by local AI.
    let s = &r.body["statements"][0];
    assert_eq!(
        (
            &s["account_id"],
            &s["account_suggested"],
            &s["suggested_account"]["confident"],
            &s["suggested_account"]["suggester"]["name"],
            &s["counts"]["new"],
        ),
        (
            &json!(h.alex_credit),
            &json!(true),
            &json!(true),
            &json!("lexical-knn"),
            &json!(3)
        ),
        "{s}"
    );
    let all = rows(&pool, &h, &id).await;
    // The coffee: all three labels, confidently; the household as purpose.
    assert_eq!(
        (
            &all[0]["counterparty_id"],
            &all[0]["category_id"],
            &all[0]["purpose_id"],
            &all[0]["missing"],
        ),
        (
            &json!(h.store),
            &json!(dining),
            &json!(h.household),
            &json!([])
        ),
        "{}",
        all[0]
    );
    let local = &all[0]["suggestions"][0]["suggester"];
    assert_eq!(
        (&local["kind"], &local["version"]),
        (&json!("local"), &json!("2"))
    );
    assert!(local["details"]["snapshot"].is_string(), "{local}");
    // A stranger: nothing confident.
    assert_eq!(
        all[2]["missing"],
        json!(["counterparty", "category", "purpose"]),
        "{}",
        all[2]
    );

    // A label you choose stays yours when the account changes and the rows
    // are suggested again.
    let mine = create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Treats" }),
    )
    .await;
    patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/rows/0/1"),
        json!({ "category_id": mine }),
    )
    .await;
    patch(
        &pool,
        &h,
        format!("/api/v1/imports/{id}/statements/0"),
        json!({ "account_id": h.alex_checking }),
    )
    .await;
    let all = rows(&pool, &h, &id).await;
    let row = &all[1];
    assert_eq!(
        (&row["category_id"], &row["chosen"], &row["counterparty_id"]),
        (&json!(mine), &json!(["category"]), &json!(h.store)),
        "{row}"
    );
    assert!(
        row["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["label"] != "category"),
        "{row}"
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_backtest_measures_local_ai_on_past_months(pool: PgPool) {
    let h = H::new(&pool).await;
    history(&pool, &h).await;
    let r = h
        .send(
            &pool,
            "GET",
            "/api/v1/suggesters/backtest?from=2025-09&to=2025-10",
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["method"], json!("lexical-knn-v2"));
    let october = &r.body["months"][1];
    assert_eq!(
        (&october["month"], &october["trained_on"]),
        (&json!("2025-10"), &json!(54))
    );
    let counterparty = &october["labels"][0];
    assert_eq!(
        (
            &counterparty["label"],
            &counterparty["asked"],
            &counterparty["right"]
        ),
        (&json!("counterparty"), &json!(6), &json!(6)),
        "{counterparty}"
    );
    assert_eq!(october["accounts"]["asked"], json!(2));
    assert_eq!(october["labels"][0]["bins"].as_array().unwrap().len(), 10);
    // Nothing was stored.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM suggester")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);

    for bad in [
        "from=2025-10&method=magic-v9",
        "from=2025-10&to=2025-09",
        "from=2023-01&to=2025-10",
    ] {
        let r = h
            .send(
                &pool,
                "GET",
                &format!("/api/v1/suggesters/backtest?{bad}"),
                None,
            )
            .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.body
        );
    }
}
