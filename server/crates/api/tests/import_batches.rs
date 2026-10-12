//! `/api/v1/import-batches`: every transaction's batch, migration batches,
//! undoing a batch, and the backfill of transactions stored before batches
//! existed. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::H;
use common::{Response, call};
use serde_json::{Value, json};
use sqlx::PgPool;

fn tx(h: &H, date: &str, batch: Option<&str>) -> Value {
    let mut body = json!({ "date": date, "description": "x", "splits": [
        { "from": h.leg(h.acct(&h.alex_credit), "10.00"), "to": h.leg(("counterparty_id", &h.store), "10.00") }] });
    if let Some(b) = batch {
        body["import_batch_id"] = json!(b);
    }
    body
}

async fn batch(pool: &PgPool, h: &H, id: &str) -> Response {
    h.send(pool, "GET", &format!("/api/v1/import-batches/{id}"), None)
        .await
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn transactions_entered_by_hand_get_a_batch_each(pool: PgPool) {
    let h = H::new(&pool).await;
    let a = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(tx(&h, "2026-06-01", None)),
        )
        .await;
    let b = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(tx(&h, "2026-06-02", None)),
        )
        .await;
    let (ba, bb) = (
        a.body["import_batch_id"].as_str().unwrap().to_owned(),
        b.body["import_batch_id"].as_str().unwrap().to_owned(),
    );
    assert_ne!(ba, bb);
    let r = batch(&pool, &h, &ba).await;
    assert_eq!(
        (
            r.body["kind"].as_str(),
            r.body["transaction_count"].as_i64()
        ),
        (Some("manual"), Some(1))
    );

    // Edits never move a transaction to another batch.
    let path = format!("/api/v1/transactions/{}", a.id());
    let r = call(&pool, "PUT", &path)
        .token(&h.t)
        .if_match(&a.etag())
        .json(tx(&h, "2026-06-03", Some(&bb)))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    let r = call(&pool, "PUT", &path)
        .token(&h.t)
        .if_match(&a.etag())
        .json(tx(&h, "2026-06-03", None))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["import_batch_id"], json!(ba));
    let r = call(&pool, "PUT", &path)
        .token(&h.t)
        .if_match(&r.etag())
        .json(tx(&h, "2026-06-04", Some(&ba)))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    // Only migration batches take new transactions.
    let r = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(tx(&h, "2026-06-05", Some(&ba))),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    let r = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(tx(
                &h,
                "2026-06-05",
                Some("00000000-0000-0000-0000-000000000000"),
            )),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_migration_batch_is_found_and_undone_together(pool: PgPool) {
    let h = H::new(&pool).await;
    for bad in [
        json!({ "kind": "manual" }),
        json!({ "kind": "file" }),
        json!({ "kind": "migration", "label": " " }),
    ] {
        let r = h
            .send(&pool, "POST", "/api/v1/import-batches", Some(bad.clone()))
            .await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    let r = h
        .send(
            &pool,
            "POST",
            "/api/v1/import-batches",
            Some(json!({ "kind": "migration", "label": "From the old app" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let id = r.id();
    let mut created = Vec::new();
    for date in ["2026-05-30", "2026-06-01", "2026-06-02"] {
        let r = h
            .send(
                &pool,
                "POST",
                "/api/v1/transactions",
                Some(tx(&h, date, Some(&id))),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
        created.push(r.id());
    }
    let other = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(tx(&h, "2026-06-01", None)),
        )
        .await
        .id();

    let r = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/transactions?import_batch_id={id}"),
            None,
        )
        .await;
    let mut found: Vec<String> = r.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect();
    found.sort();
    created.sort();
    assert_eq!(found, created);

    let r = h
        .send(&pool, "GET", "/api/v1/import-batches?kind=migration", None)
        .await;
    let items = r.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        (
            items[0]["label"].as_str(),
            items[0]["transaction_count"].as_i64()
        ),
        (Some("From the old app"), Some(3))
    );

    // Undoing acts on what was read: a change since then is a 412.
    let seen = batch(&pool, &h, &id).await.etag();
    let path = format!("/api/v1/transactions/{}", created[2]);
    let etag = h.send(&pool, "GET", &path, None).await.etag();
    let r = call(&pool, "DELETE", &path)
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let undo = |etag: String| {
        let pool = pool.clone();
        let (t, id) = (h.t.clone(), id.clone());
        async move {
            call(&pool, "DELETE", format!("/api/v1/import-batches/{id}"))
                .token(&t)
                .if_match(&etag)
                .send()
                .await
        }
    };
    assert_eq!(undo(seen).await.status, StatusCode::PRECONDITION_FAILED);

    // A transaction in a closed month blocks the whole undo.
    h.put_params(
        &pool,
        "2026-05",
        json!({ "stipends": [{ "party_id": h.alex, "amount": "0" }],
                "funders": [{ "business_id": h.studio, "funder_id": h.household }] }),
    )
    .await;
    assert_eq!(
        h.post_month(&pool, "2026-05", "close").await.status,
        StatusCode::OK
    );
    let r = undo(batch(&pool, &h, &id).await.etag()).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert_eq!(
        batch(&pool, &h, &id).await.body["transaction_count"],
        json!(2)
    );

    h.post_month(&pool, "2026-05", "reopen").await;
    let r = undo(batch(&pool, &h, &id).await.etag()).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.body);
    assert_eq!(batch(&pool, &h, &id).await.status, StatusCode::NOT_FOUND);
    for t in &created {
        let r = h
            .send(&pool, "GET", &format!("/api/v1/transactions/{t}"), None)
            .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND);
    }
    let r = h
        .send(&pool, "GET", &format!("/api/v1/transactions/{other}"), None)
        .await;
    assert_eq!(r.status, StatusCode::OK);
}

/// Transactions stored before batches existed: those a token created with
/// outside references are one migration batch, the rest one batch each.
#[sqlx::test(migrations = false)]
async fn backfill_groups_what_each_token_migrated(pool: PgPool) {
    let (before, after): (Vec<&sqlx::migrate::Migration>, Vec<_>) =
        store::MIGRATOR.iter().partition(|m| m.version < 8);
    for m in before {
        sqlx::raw_sql(m.sql.clone()).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql(
        "INSERT INTO api_token (id, name, secret_hash) VALUES
             ('00000000-0000-0000-0000-0000000000a1', 'migration', 'x'),
             ('00000000-0000-0000-0000-0000000000a2', 'web', 'x');
         INSERT INTO transaction (id, date, description) VALUES
             ('00000000-0000-0000-0000-000000000001', '2026-01-01', 'migrated'),
             ('00000000-0000-0000-0000-000000000002', '2026-01-01', 'opening balance'),
             ('00000000-0000-0000-0000-000000000003', '2026-02-01', 'by hand'),
             ('00000000-0000-0000-0000-000000000004', '2026-02-02', 'by hand');
         INSERT INTO transaction_external_ref VALUES
             ('00000000-0000-0000-0000-000000000001', 'oldapp', '7');
         INSERT INTO audit_log (id, token_id, action, entity, entity_id) VALUES
             (gen_random_uuid(), '00000000-0000-0000-0000-0000000000a1', 'create', 'transaction',
              '00000000-0000-0000-0000-000000000001'),
             (gen_random_uuid(), '00000000-0000-0000-0000-0000000000a1', 'create', 'transaction',
              '00000000-0000-0000-0000-000000000002'),
             (gen_random_uuid(), '00000000-0000-0000-0000-0000000000a2', 'create', 'transaction',
              '00000000-0000-0000-0000-000000000003'),
             (gen_random_uuid(), '00000000-0000-0000-0000-0000000000a2', 'create', 'transaction',
              '00000000-0000-0000-0000-000000000004');",
    )
    .execute(&pool)
    .await
    .unwrap();
    for m in after {
        sqlx::raw_sql(m.sql.clone()).execute(&pool).await.unwrap();
    }

    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT t.description, b.kind, b.label FROM transaction t
         JOIN import_batch b ON b.id = t.import_batch_id ORDER BY t.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let label = Some("Migrated from oldapp".to_owned());
    assert_eq!(
        rows,
        [
            ("migrated".into(), "migration".into(), label.clone()),
            ("opening balance".into(), "migration".into(), label),
            ("by hand".into(), "manual".into(), None),
            ("by hand".into(), "manual".into(), None),
        ]
    );
    let batches: i64 = sqlx::query_scalar("SELECT count(*) FROM import_batch")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(batches, 3);
}
