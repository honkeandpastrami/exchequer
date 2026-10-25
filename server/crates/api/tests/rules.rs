//! The rules script (design §8, docs/ai.md): versions, validation, its
//! suggestions in staging, and dry runs. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::{H, create};
use common::{Response, call};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn put(pool: &PgPool, h: &H, etag: &str, source: &str, note: Option<&str>) -> Response {
    call(pool, "PUT", "/api/v1/rules")
        .token(&h.t)
        .if_match(etag)
        .json(json!({ "source": source, "note": note }))
        .send()
        .await
}

const SCRIPT: &str = r#"// Coffee is the household's.
fn classify(row, s) {
    if row.description.contains("CORNER CAFE") && row.amount < 0 {
        s.counterparty("Store");
        s.category("Dining");
        s.purpose("Household");
    }
}

fn account(file, s) {
    if file.name == "alex-card.csv" {
        s.account("Alex · Alex Credit");
    }
}
"#;

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn versions_are_validated_and_kept(pool: PgPool) {
    let h = H::new(&pool).await;
    create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Dining" }),
    )
    .await;
    // None yet: version 0, no rules.
    let r = h.send(&pool, "GET", "/api/v1/rules", None).await;
    assert_eq!(
        (&r.body["version"], &r.body["source"]),
        (&json!(0), &json!(""))
    );
    let etag = r.etag();

    // Invalid: refused, with each error's line; nothing stored.
    let r = put(
        &pool,
        &h,
        &etag,
        "fn classify(row, s) {\n  s.category(\"Dinning\");\n  s.purpose(\"Nobody\");\n}",
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    assert_eq!(r.body["type"], json!("urn:exchequer:problem:rules-invalid"));
    let lines: Vec<&Value> = r.body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["line"])
        .collect();
    assert_eq!(lines, [&json!(2), &json!(3)], "{}", r.body);
    let r = put(&pool, &h, &etag, "fn classify(row, s) {", None).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Valid: version 1, active at once.
    let r = put(&pool, &h, &etag, SCRIPT, Some("coffee")).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(
        (&r.body["version"], &r.body["note"], &r.body["author"]),
        (&json!(1), &json!("coffee"), &json!(common::TESTER))
    );
    // Against a stale version: refused.
    let r = put(&pool, &h, &etag, "", None).await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED);
    let r = h.send(&pool, "GET", "/api/v1/rules", None).await;
    let r = put(&pool, &h, &r.etag(), "", Some("no rules")).await;
    assert_eq!(r.body["version"], json!(2));

    let r = h.send(&pool, "GET", "/api/v1/rules/versions", None).await;
    let versions: Vec<(&Value, &Value)> = r.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (&v["version"], &v["lines"]))
        .collect();
    assert_eq!(versions, [(&json!(2), &json!(0)), (&json!(1), &json!(14))]);
    let r = h.send(&pool, "GET", "/api/v1/rules/versions/1", None).await;
    assert_eq!(r.body["source"], json!(SCRIPT));
    let r = h.send(&pool, "GET", "/api/v1/rules/versions/9", None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn rules_suggest_first_and_confidently(pool: PgPool) {
    let h = H::new(&pool).await;
    let dining = create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Dining" }),
    )
    .await;
    let etag = h.send(&pool, "GET", "/api/v1/rules", None).await.etag();
    let r = put(&pool, &h, &etag, SCRIPT, None).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);

    let csv = "Date,Description,Amount\n\
               03/01/2026,SQ *CORNER CAFE #12,-4.50\n\
               03/02/2026,HARDWARE,-20.00\n";
    let r = call(&pool, "POST", "/api/v1/imports?file_name=alex-card.csv")
        .token(&h.t)
        .bytes(csv.as_bytes().to_vec())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let s = &r.body["statements"][0];
    assert_eq!(
        (
            &s["account_id"],
            &s["suggested_account"]["suggester"]["kind"],
            &s["suggested_account"]["reason"],
            &s["suggested_account"]["confidence"],
        ),
        (
            &json!(h.alex_credit),
            &json!("rule"),
            &json!("rule v1:12"),
            &json!(1000)
        ),
        "{s}"
    );
    let id = r.id();
    let rows = h
        .send(&pool, "GET", &format!("/api/v1/imports/{id}/rows"), None)
        .await;
    let coffee = &rows.body["items"][0];
    assert_eq!(
        (
            &coffee["counterparty_id"],
            &coffee["category_id"],
            &coffee["purpose_id"],
            &coffee["missing"]
        ),
        (
            &json!(h.store),
            &json!(dining),
            &json!(h.household),
            &json!([])
        ),
        "{coffee}"
    );
    let reasons: Vec<&Value> = coffee["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["reason"])
        .collect();
    assert_eq!(
        reasons,
        [
            &json!("rule v1:5"),
            &json!("rule v1:4"),
            &json!("rule v1:6")
        ]
    );
    assert_eq!(coffee["suggestions"][0]["suggester"]["version"], json!("1"));
    // A row no rule covers: nothing from rules.
    assert_eq!(
        rows.body["items"][1]["missing"],
        json!(["counterparty", "category", "purpose"])
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_dry_run_shows_what_a_script_would_change(pool: PgPool) {
    let h = H::new(&pool).await;
    let dining = create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Dining" }),
    )
    .await;
    create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Treats" }),
    )
    .await;
    for (date, description) in [
        ("2026-03-01", "CORNER CAFE"),
        ("2026-03-02", "CORNER CAFE"),
        ("2026-03-03", "HARDWARE"),
        ("2026-05-01", "CORNER CAFE"),
    ] {
        let r = h
            .send(
                &pool,
                "POST",
                "/api/v1/transactions",
                Some(
                    json!({ "date": date, "description": description, "splits": [{
                    "from": h.leg(h.acct(&h.alex_credit), "4.50"),
                    "to": h.leg(("counterparty_id", &h.store), "4.50"),
                    "category_id": dining, "purpose_id": h.alex }] }),
                ),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    }
    let dry = |body: Value, range: &str| {
        let (pool, h) = (pool.clone(), &h);
        let path = format!("/api/v1/rules/dry-run?{range}");
        async move { h.send(&pool, "POST", &path, Some(body)).await }
    };
    // The candidate moves coffee to Treats, and fails on hardware.
    let candidate = "fn classify(row, s) {\n\
                     if row.description == \"CORNER CAFE\" { s.category(\"Treats\"); s.counterparty(\"Store\"); }\n\
                     else { throw \"unexpected\"; }\n\
                     }";
    let r = dry(
        json!({ "source": candidate }),
        "from=2026-03-01&to=2026-03-31",
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (
            r.body.get("version"),
            &r.body["rows"],
            &r.body["labeled"],
            &r.body["changed"],
            &r.body["failed"]
        ),
        (None, &json!(3), &json!(2), &json!(2), &json!(1)),
        "{}",
        r.body
    );
    let first = &r.body["items"][0];
    assert_eq!(
        (
            &first["changes"][0]["label"],
            &first["changes"][0]["current"],
            &first["changes"][0]["line"]
        ),
        (&json!("category"), &json!(dining), &json!(2)),
        "{first}"
    );
    assert_eq!(first["changes"].as_array().unwrap().len(), 1);
    assert!(
        r.body["items"][2]["error"]
            .as_str()
            .unwrap()
            .contains("unexpected")
    );

    // The active script: none yet, so nothing changes.
    let r = dry(json!({}), "from=2026-01-01&to=2026-12-31").await;
    assert_eq!(
        (&r.body["version"], &r.body["rows"], &r.body["changed"]),
        (&json!(0), &json!(4), &json!(0))
    );
    for (body, range) in [
        (
            json!({ "source": "fn classify(row, s) { s.category(\"Nope\"); }" }),
            "from=2026-01-01&to=2026-01-31",
        ),
        (json!({}), "from=2026-02-01&to=2026-01-01"),
        (json!({}), "from=2020-01-01&to=2026-01-01"),
    ] {
        let r = dry(body, range).await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{range}: {}",
            r.body
        );
    }
}
