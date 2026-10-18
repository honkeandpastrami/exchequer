//! Committing staged imports: decisions, new transactions, transfers,
//! recorded rows, balance checks, and refusals. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::H;
use common::{Response, call};
use serde_json::{Value, json};
use sqlx::PgPool;

const CARD: &str = "4111000011112222";

fn ofx() -> String {
    format!(
        "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\nSECURITY:NONE\r\n\
ENCODING:USASCII\r\nCHARSET:1252\r\nCOMPRESSION:NONE\r\nOLDFILEUID:NONE\r\nNEWFILEUID:NONE\r\n\r\n\
<OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<DTSERVER>20260331120000<LANGUAGE>ENG</SONRS></SIGNONMSGSRSV1>\
<CREDITCARDMSGSRSV1><CCSTMTTRNRS><TRNUID>1<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<CCSTMTRS><CURDEF>USD<CCACCTFROM><ACCTID>{CARD}</CCACCTFROM>\
<BANKTRANLIST><DTSTART>20260301<DTEND>20260331\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260302<TRNAMT>-80.00<FITID>F1<NAME>GROCER</STMTTRN>\
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260303<TRNAMT>30.00<FITID>F2<MEMO>Refund</STMTTRN>\
</BANKTRANLIST><LEDGERBAL><BALAMT>-50.00<DTASOF>20260331</LEDGERBAL>\
</CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>"
    )
}

const CSV: &str = "Date,Description,Amount\n\
03/01/2026,COFFEE,-4.50\n\
03/02/2026,COFFEE,-4.50\n\
03/03/2026,PAYROLL,\"1,500.00\"\n";

async fn upload(pool: &PgPool, h: &H, query: &str, file: &str) -> Response {
    let r = call(pool, "POST", format!("/api/v1/imports{query}"))
        .token(&h.t)
        .bytes(file.as_bytes().to_vec())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r
}

async fn read(pool: &PgPool, h: &H, id: &str) -> Response {
    h.send(pool, "GET", &format!("/api/v1/imports/{id}"), None)
        .await
}

async fn commit(pool: &PgPool, h: &H, id: &str, etag: &str) -> Response {
    call(pool, "POST", format!("/api/v1/imports/{id}/commit"))
        .token(&h.t)
        .if_match(etag)
        .send()
        .await
}

async fn rows(pool: &PgPool, h: &H, id: &str) -> Vec<Value> {
    let r = h
        .send(pool, "GET", &format!("/api/v1/imports/{id}/rows"), None)
        .await;
    r.body["items"].as_array().unwrap().clone()
}

async fn set_row(pool: &PgPool, h: &H, id: &str, position: i32, body: Value) -> Response {
    let etag = read(pool, h, id).await.etag();
    call(
        pool,
        "PATCH",
        format!("/api/v1/imports/{id}/rows/0/{position}"),
    )
    .token(&h.t)
    .if_match(&etag)
    .json(body)
    .send()
    .await
}

/// Gives every new row's missing labels an explicit value (none for the
/// counterparty; Alex and Shopping for the purpose and category, which are
/// never none), as commit needs (design §8).
async fn label_none(pool: &PgPool, h: &H, id: &str) {
    for r in rows(pool, h, id).await {
        if r["missing"].as_array().unwrap().is_empty() {
            continue;
        }
        let position = i32::try_from(r["position"].as_i64().unwrap()).unwrap();
        let body: serde_json::Map<String, Value> = r["missing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                let v = match l.as_str().unwrap() {
                    "purpose" => json!(h.alex),
                    "category" => json!(h.shopping),
                    _ => Value::Null,
                };
                (format!("{}_id", l.as_str().unwrap()), v)
            })
            .collect();
        let r = set_row(pool, h, id, position, Value::Object(body)).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.body);
        assert_eq!(r.body["missing"], json!([]));
    }
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_card_statement_becomes_transactions_and_a_balance_check(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = upload(&pool, &h, "?file_name=march.ofx", &ofx()).await;
    let id = r.id();
    let r = commit(&pool, &h, &id, &r.etag()).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert!(r.body["detail"].as_str().unwrap().contains("no account"));

    let etag = read(&pool, &h, &id).await.etag();
    call(&pool, "PATCH", format!("/api/v1/imports/{id}/statements/0"))
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "account_id": h.alex_credit, "remember_number": true }))
        .send()
        .await;
    assert_eq!(
        commit(&pool, &h, &id, &etag).await.status,
        StatusCode::PRECONDITION_FAILED
    );
    // New rows need their labels: yours or confidently suggested.
    let r = commit(&pool, &h, &id, &read(&pool, &h, &id).await.etag()).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert_eq!(
        r.body["type"],
        json!("urn:exchequer:problem:labels-missing")
    );
    assert!(
        r.body["detail"]
            .as_str()
            .unwrap()
            .contains("statement 0 row 0 (counterparty, category, purpose)"),
        "{}",
        r.body
    );
    label_none(&pool, &h, &id).await;
    let r = commit(&pool, &h, &id, &read(&pool, &h, &id).await.etag()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (
            &r.body["created"],
            &r.body["transfers"],
            &r.body["matched"],
            &r.body["skipped"]
        ),
        (&json!(2), &json!(0), &json!(0), &json!(0))
    );
    let check = &r.body["balance_checks"][0];
    assert_eq!(
        (&check["source"], &check["matches"], &check["date"]),
        (&json!("ofx"), &json!(true), &json!("2026-03-31"))
    );
    assert_eq!(read(&pool, &h, &id).await.status, StatusCode::NOT_FOUND);

    // The transactions: in a file batch named after the file.
    let batch = r.body["import_batch_id"].as_str().unwrap().to_owned();
    let b = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/import-batches/{batch}"),
            None,
        )
        .await;
    assert_eq!(
        (
            &b.body["kind"],
            &b.body["label"],
            &b.body["transaction_count"]
        ),
        (&json!("file"), &json!("march.ofx"), &json!(2))
    );
    let list = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/transactions?import_batch_id={batch}"),
            None,
        )
        .await;
    let mut got: Vec<(String, String, Value, Value)> = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            let s = &t["splits"][0];
            (
                t["description"].as_str().unwrap().to_owned(),
                s["kind"].as_str().unwrap().to_owned(),
                s["from"]["account_id"].clone(),
                s["to"]["account_id"].clone(),
            )
        })
        .collect();
    got.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        got,
        [
            (
                "GROCER".into(),
                "expense".into(),
                json!(h.alex_credit),
                Value::Null
            ),
            (
                "Refund".into(),
                "income".into(),
                Value::Null,
                json!(h.alex_credit)
            ),
        ]
    );
    // What the file said about each row is kept with its transaction.
    let grocer = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["description"] == json!("GROCER"))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let seen = |pool: PgPool, t: String, token: String| async move {
        call(
            &pool,
            "GET",
            format!("/api/v1/transactions/{t}/imported-rows"),
        )
        .token(&token)
        .send()
        .await
        .body
    };
    let first = seen(pool.clone(), grocer.clone(), h.t.clone()).await;
    assert_eq!(first.as_array().unwrap().len(), 1, "{first}");
    assert_eq!(
        (
            &first[0]["fingerprint"],
            &first[0]["account_id"],
            &first[0]["raw"]["FITID"]
        ),
        (&json!("F1"), &json!(h.alex_credit), &json!("F1"))
    );
    let sha = first[0]["file_sha256"].as_str().unwrap().to_owned();
    assert_eq!(sha.len(), 64);
    // The same file again, under another name: all duplicates, nothing new,
    // and nothing more to say about the rows.
    let r = upload(&pool, &h, "?file_name=copy.ofx", &ofx()).await;
    assert_eq!(r.body["file_sha256"], json!(sha));
    assert_eq!(r.body["statements"][0]["counts"]["duplicate"], json!(2));
    let r = commit(&pool, &h, &r.id(), &r.etag()).await;
    assert_eq!(
        (
            &r.body["created"],
            &r.body["matched"],
            r.body.get("import_batch_id")
        ),
        (&json!(0), &json!(2), None)
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM imported_row").await, 2);
    // Another file with the same rows adds what it says to the same
    // transactions.
    let other = ofx().replace("<NAME>GROCER", "<NAME>GROCER<MEMO>store 12");
    let r = upload(&pool, &h, "", &other).await;
    assert_ne!(r.body["file_sha256"], json!(sha));
    commit(&pool, &h, &r.id(), &r.etag()).await;
    let now = seen(pool.clone(), grocer.clone(), h.t.clone()).await;
    let memos: Vec<_> = now
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["raw"]["MEMO"].clone())
        .collect();
    assert_eq!(memos, [Value::Null, json!("store 12")]);

    // Undoing the batch forgets the rows: the file is new again.
    let etag = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/import-batches/{batch}"),
            None,
        )
        .await
        .etag();
    let r = call(&pool, "DELETE", format!("/api/v1/import-batches/{batch}"))
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = upload(&pool, &h, "", &ofx()).await;
    assert_eq!(r.body["statements"][0]["counts"]["new"], json!(2));
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn decisions_labels_and_transfers(pool: PgPool) {
    let h = H::new(&pool).await;
    let savings = h.acct(&h.joint_savings);
    // A transfer from savings into checking, already recorded (from the
    // savings side): checking's row of it is a transfer duplicate.
    let pay = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(
                json!({ "date": "2026-03-07", "description": "To checking", "splits": [{
                "from": h.leg(savings, "1500.00"),
                "to": h.leg(h.acct(&h.alex_checking), "1500.00") }] }),
            ),
        )
        .await
        .id();
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let id = r.id();
    let all = rows(&pool, &h, &id).await;
    let decisions: Vec<_> = all
        .iter()
        .map(|r| (r["status"].clone(), r["decision"].clone()))
        .collect();
    assert_eq!(
        decisions,
        [
            (json!("new"), json!("new")),
            (json!("new"), json!("new")),
            (json!("transfer_duplicate"), json!("match")),
        ]
    );
    assert_eq!(all[2]["match_transaction_id"], json!(pay));
    // Each decision is matching's suggestion: sure of a new row, less of a
    // transfer recorded four days off.
    let suggested = |r: &Value| {
        let s = &r["suggested_decision"];
        (
            s["decision"].clone(),
            s["confidence"].clone(),
            s["confident"].clone(),
            s["reason"].clone(),
            s["suggester"]["kind"].clone(),
        )
    };
    assert_eq!(
        suggested(&all[0]),
        (
            json!("new"),
            json!(1000),
            json!(true),
            json!("nothing like it in the ledger"),
            json!("match")
        )
    );
    assert_eq!(
        suggested(&all[2]),
        (
            json!("match"),
            json!(880),
            json!(false),
            json!("the transfer is already recorded, 4 days apart"),
            json!("match")
        )
    );
    set_row(&pool, &h, &id, 2, json!({ "decision": "match" })).await;

    // Can't match a row with nothing to match; skip one coffee, label the other.
    let r = set_row(&pool, &h, &id, 0, json!({ "decision": "match" })).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = set_row(&pool, &h, &id, 0, json!({ "decision": "skip" })).await;
    assert_eq!(
        (r.body["decision"].clone(), r.body["decided"].clone()),
        (json!("skip"), json!(true))
    );
    let r = set_row(
        &pool,
        &h,
        &id,
        1,
        json!({ "set_description": " Morning coffee ", "counterparty_id": h.store,
                "purpose_id": h.household }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["set_description"], json!("Morning coffee"));
    // A purpose names someone: `null` isn't one.
    let r = set_row(&pool, &h, &id, 1, json!({ "purpose_id": null })).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    let r = set_row(&pool, &h, &id, 1, json!({ "purpose_id": h.alex })).await;
    assert_eq!(r.body["purpose_id"], json!(h.alex));
    assert_eq!(r.body["counterparty_id"], json!(h.store));
    // Only the category is open.
    assert_eq!(
        (&r.body["chosen"], &r.body["missing"]),
        (&json!(["counterparty", "purpose"]), &json!(["category"]))
    );
    // A category names something: `null` isn't one either.
    let r = set_row(&pool, &h, &id, 1, json!({ "category_id": null })).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    let r = set_row(&pool, &h, &id, 1, json!({ "category_id": h.shopping })).await;
    assert_eq!(r.body["missing"], json!([]));
    // Reverting gives the label back to suggestions; there are none.
    let r = set_row(&pool, &h, &id, 1, json!({ "revert": ["counterparty"] })).await;
    assert_eq!(
        (r.body.get("counterparty_id"), &r.body["missing"]),
        (None, &json!(["counterparty"]))
    );
    let r = set_row(
        &pool,
        &h,
        &id,
        1,
        json!({ "revert": ["counterparty"], "counterparty_id": h.store }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    set_row(&pool, &h, &id, 1, json!({ "counterparty_id": h.store })).await;

    let r = commit(&pool, &h, &id, &read(&pool, &h, &id).await.etag()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (
            &r.body["created"],
            &r.body["transfers"],
            &r.body["matched"],
            &r.body["skipped"]
        ),
        (&json!(1), &json!(0), &json!(1), &json!(1))
    );
    // The transfer stays as it was, now holding checking's bank row too.
    let t = h
        .send(&pool, "GET", &format!("/api/v1/transactions/{pay}"), None)
        .await;
    let s = &t.body["splits"][0];
    assert_eq!(
        (
            &s["kind"],
            &s["from"]["account_id"],
            &s["to"]["account_id"],
            s["to"].get("counterparty_id")
        ),
        (
            &json!("transfer"),
            &json!(h.joint_savings),
            &json!(h.alex_checking),
            None
        )
    );
    let batch = r.body["import_batch_id"].as_str().unwrap();
    let list = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/transactions?import_batch_id={batch}"),
            None,
        )
        .await;
    let coffee = &list.body["items"][0];
    assert_eq!(coffee["description"], json!("Morning coffee"));
    assert_eq!(coffee["splits"][0]["to"]["counterparty_id"], json!(h.store));
    // Skipped rows aren't recorded; the other two are.
    assert_eq!(
        count(&pool, "SELECT count(*) FROM import_fingerprint").await,
        2
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn commit_rechecks_and_is_all_or_nothing(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let (id, etag) = (r.id(), r.etag());
    // Meanwhile, the pay was entered by hand.
    let salary = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(
                json!({ "date": "2026-03-03", "description": "Salary", "splits": [{
            "from": h.leg(("counterparty_id", &h.store), "1500.00"),
            "to": h.leg(h.acct(&h.alex_checking), "1500.00"), "purpose_id": h.alex, "category_id": h.shopping }] }),
            ),
        )
        .await
        .id();
    let r = commit(&pool, &h, &id, &etag).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert_eq!(count(&pool, "SELECT count(*) FROM transaction").await, 1);
    let now = read(&pool, &h, &id).await;
    assert_ne!(now.etag(), etag);
    assert_eq!(
        now.body["statements"][0]["counts"]["near_duplicate"],
        json!(1)
    );

    // A near duplicate is a guess: commit waits for your decision, even
    // the default one.
    label_none(&pool, &h, &id).await;
    let r = commit(&pool, &h, &id, &read(&pool, &h, &id).await.etag()).await;
    assert_eq!(
        (r.status, &r.body["type"]),
        (
            StatusCode::CONFLICT,
            &json!("urn:exchequer:problem:decisions-missing")
        ),
        "{}",
        r.body
    );
    assert_eq!(
        read(&pool, &h, &id).await.body["readiness"]["undecided"],
        json!(1)
    );
    let r = set_row(&pool, &h, &id, 2, json!({ "decision": "match" })).await;
    assert_eq!(r.body["decided"], json!(true), "{}", r.body);

    // A closed month refuses the whole commit.
    let now = read(&pool, &h, &id).await;
    h.put_params(
        &pool,
        "2026-03",
        json!({ "stipends": [{ "party_id": h.alex, "amount": "0" }],
                "funders": [{ "business_id": h.studio, "funder_id": h.household }] }),
    )
    .await;
    assert_eq!(
        h.post_month(&pool, "2026-03", "close").await.status,
        StatusCode::OK
    );
    let r = commit(&pool, &h, &id, &now.etag()).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert!(
        r.body["detail"].as_str().unwrap().contains("row 0"),
        "{}",
        r.body
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM transaction").await, 1);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM import_fingerprint").await,
        0
    );

    h.post_month(&pool, "2026-03", "reopen").await;
    let r = commit(&pool, &h, &id, &now.etag()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (&r.body["created"], &r.body["matched"]),
        (&json!(2), &json!(1))
    );
    // The bank's row is kept with the transaction entered by hand.
    let rows = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/transactions/{salary}/imported-rows"),
            None,
        )
        .await;
    assert_eq!(
        rows.body[0]["raw"]["Description"],
        json!("PAYROLL"),
        "{}",
        rows.body
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_row_can_replace_the_transaction_it_matches(pool: PgPool) {
    let h = H::new(&pool).await;
    // The pay, typed by hand two days off, with notes and a tag.
    let salary = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(
                json!({ "date": "2026-03-01", "description": "Salary", "notes": "March",
                        "tags": ["work"], "splits": [{
            "from": h.leg(("counterparty_id", &h.store), "1500.00"),
            "to": h.leg(h.acct(&h.alex_checking), "1500.00"), "purpose_id": h.alex, "category_id": h.shopping }] }),
            ),
        )
        .await
        .id();
    // A coffee typed as two splits can't be replaced by one bank row.
    h.send(
        &pool,
        "POST",
        "/api/v1/transactions",
        Some(
            json!({ "date": "2026-03-01", "description": "Coffees", "splits": [
            { "from": h.leg(h.acct(&h.alex_checking), "4.50"),
              "to": h.leg(("counterparty_id", &h.store), "4.50"), "purpose_id": h.alex, "category_id": h.shopping },
            { "from": h.leg(h.acct(&h.alex_checking), "4.50"),
              "to": h.leg(("counterparty_id", &h.store), "4.50"), "purpose_id": h.alex, "category_id": h.shopping }] }),
        ),
    )
    .await;
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let id = r.id();
    let all = rows(&pool, &h, &id).await;
    let statuses: Vec<_> = all.iter().map(|r| r["status"].clone()).collect();
    assert_eq!(
        statuses,
        [
            json!("near_duplicate"),
            json!("new"),
            json!("near_duplicate")
        ]
    );

    let r = set_row(&pool, &h, &id, 0, json!({ "decision": "replace" })).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    assert!(r.body["detail"].as_str().unwrap().contains("one split"));
    // A new row has nothing to replace.
    let r = set_row(&pool, &h, &id, 1, json!({ "decision": "replace" })).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let wages = common::household::create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Wages" }),
    )
    .await;
    let r = set_row(
        &pool,
        &h,
        &id,
        2,
        json!({ "decision": "replace", "category_id": wages }),
    )
    .await;
    assert_eq!(
        (&r.body["decision"], &r.body["missing"]),
        (&json!("replace"), &json!([])),
        "{}",
        r.body
    );
    label_none(&pool, &h, &id).await;
    set_row(&pool, &h, &id, 0, json!({ "decision": "match" })).await;
    let r = commit(&pool, &h, &id, &read(&pool, &h, &id).await.etag()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (&r.body["replaced"], &r.body["matched"], &r.body["created"]),
        (&json!(1), &json!(1), &json!(1))
    );
    // Same transaction: the bank's date and text, its own counterparty,
    // notes and tags, the category chosen in review.
    let t = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/transactions/{salary}"),
            None,
        )
        .await;
    let s = &t.body["splits"][0];
    assert_eq!(
        (
            &t.body["date"],
            &t.body["description"],
            &t.body["notes"],
            &t.body["tags"],
            &s["from"]["counterparty_id"],
            &s["category_id"],
            &s["to"]["quantity"]
        ),
        (
            &json!("2026-03-03"),
            &json!("PAYROLL"),
            &json!("March"),
            &json!(["work"]),
            &json!(h.store),
            &json!(wages),
            &json!("1500.00")
        )
    );
    // And the row is recorded with it: next time it's a duplicate.
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    assert_eq!(r.body["statements"][0]["counts"]["duplicate"], json!(3));
}

async fn commit_ready(pool: &PgPool, h: &H, ids: &[&str]) -> Vec<Value> {
    let r = h
        .send(
            pool,
            "POST",
            "/api/v1/imports/commit-ready",
            Some(json!({ "ids": ids })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    r.body["results"].as_array().unwrap().clone()
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn ready_imports_are_committed_together(pool: PgPool) {
    let h = H::new(&pool).await;
    let card = upload(&pool, &h, "?file_name=march.ofx", &ofx()).await;
    let card_id = card.id();
    let ready = |r: &Response| r.body["readiness"].clone();
    assert_eq!(
        ready(&card),
        json!({ "state": "needs_work", "no_account": 1, "unlabeled": 0, "undecided": 0,
                "suggested_accounts": 0, "suggested_rows": 0 })
    );
    let r = call(
        &pool,
        "PATCH",
        format!("/api/v1/imports/{card_id}/statements/0"),
    )
    .token(&h.t)
    .if_match(&card.etag())
    .json(json!({ "account_id": h.alex_credit }))
    .send()
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let r = read(&pool, &h, &card_id).await;
    assert_eq!(
        (
            &ready(&r)["state"],
            &ready(&r)["no_account"],
            &ready(&r)["unlabeled"]
        ),
        (&json!("needs_work"), &json!(0), &json!(2))
    );
    label_none(&pool, &h, &card_id).await;
    assert_eq!(
        ready(&read(&pool, &h, &card_id).await)["state"],
        json!("ready")
    );

    // Another, still needing labels, isn't committed; one that's gone is
    // said to be.
    let cash = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let cash_id = cash.id();
    let gone = "00000000-0000-7000-8000-000000000000";
    let results = commit_ready(&pool, &h, &[&cash_id, &card_id, gone]).await;
    assert_eq!(results.len(), 3);
    let of = |id: &str| results.iter().find(|r| r["id"] == json!(id)).unwrap();
    assert_eq!(of(gone)["skipped"], json!("it's gone"));
    assert_eq!(of(&cash_id)["skipped"], json!("it isn't ready"));
    assert_eq!(
        (
            &of(&card_id)["committed"]["created"],
            &of(&card_id)["file_name"]
        ),
        (&json!(2), &json!("march.ofx"))
    );
    assert_eq!(
        read(&pool, &h, &card_id).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(read(&pool, &h, &cash_id).await.status, StatusCode::OK);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_split_row_becomes_one_transaction_of_its_parts(pool: PgPool) {
    let h = H::new(&pool).await;
    let id = upload(&pool, &h, "?file_name=march.ofx", &ofx()).await.id();
    let groceries = common::household::create(
        &pool,
        &h.t,
        "/api/v1/categories",
        json!({ "name": "Groceries" }),
    )
    .await;
    let split = |parts: Value| json!({ "splits": parts });
    // No account yet: nothing to split from.
    let two = json!([
        { "amount": "50.00", "counterparty_id": h.store, "category_id": groceries,
          "purpose_id": h.household, "description": "food" },
        { "amount": "30.00", "transfer_account_id": h.alex_checking }
    ]);
    let r = set_row(&pool, &h, &id, 0, split(two.clone())).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);

    let etag = read(&pool, &h, &id).await.etag();
    call(&pool, "PATCH", format!("/api/v1/imports/{id}/statements/0"))
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "account_id": h.alex_credit }))
        .send()
        .await;
    for (parts, says) in [
        (json!([{ "amount": "80.00" }]), "two or more"),
        (
            json!([{ "amount": "50.00" }, { "amount": "20.00" }]),
            "add up to 70.00, the row is 80.00",
        ),
        (
            json!([{ "amount": "80.00" }, { "amount": "0" }]),
            "split 2: the amount must be more than zero",
        ),
        (
            json!([{ "amount": "50.00" }, { "amount": "30.00", "transfer_account_id": h.alex_credit }]),
            "split 2: transfer_account_id is the statement's own account",
        ),
        (
            json!([{ "amount": "50.00" },
                   { "amount": "30.00", "transfer_account_id": h.alex_checking,
                     "category_id": groceries }]),
            "split 2: a transfer has no category or purpose",
        ),
        (
            json!([{ "amount": "50.00", "counterparty_id": h.store },
                   { "amount": "30.00", "transfer_account_id": h.alex_checking }]),
            "split 1: purpose_id: who it was for",
        ),
    ] {
        let r = set_row(&pool, &h, &id, 0, split(parts)).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
        assert!(
            r.body["detail"].as_str().unwrap().contains(says),
            "{says}: {}",
            r.body
        );
    }
    // Only a row added as new is split.
    let r = set_row(
        &pool,
        &h,
        &id,
        0,
        json!({ "decision": "skip", "splits": two }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);

    // Split: its labels are the parts', so it needs none of its own.
    let r = set_row(&pool, &h, &id, 0, split(two.clone())).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["missing"], json!([]));
    assert_eq!(r.body["splits"].as_array().unwrap().len(), 2);
    assert_eq!(r.body["splits"][0]["description"], json!("food"));
    let r = set_row(&pool, &h, &id, 0, json!({ "decision": "skip" })).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    assert!(
        r.body["detail"]
            .as_str()
            .unwrap()
            .contains("clear its splits first")
    );
    // Unsplit, its labels are needed again; split once more.
    let r = set_row(&pool, &h, &id, 0, json!({ "splits": null })).await;
    assert_eq!(
        (
            &r.body["splits"],
            r.body["missing"].as_array().unwrap().len()
        ),
        (&json!([]), 3)
    );
    let r = set_row(&pool, &h, &id, 0, split(two)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    label_none(&pool, &h, &id).await;
    let r = read(&pool, &h, &id).await;
    assert_eq!(r.body["readiness"]["state"], json!("ready"), "{}", r.body);
    let r = commit(&pool, &h, &id, &r.etag()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["created"], json!(2));
    let batch = r.body["import_batch_id"].as_str().unwrap().to_owned();
    let list = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/transactions?import_batch_id={batch}&q=GROCER"),
            None,
        )
        .await;
    let t = &list.body["items"][0];
    let got: Vec<(Value, Value, Value, Value, Value, Value)> = t["splits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["from"]["account_id"].clone(),
                s["to"]["counterparty_id"].clone(),
                s["to"]["account_id"].clone(),
                s["from"]["quantity"].clone(),
                s["category_id"].clone(),
                s["kind"].clone(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                json!(h.alex_credit),
                json!(h.store),
                Value::Null,
                json!("50.00"),
                json!(groceries),
                json!("expense")
            ),
            (
                json!(h.alex_credit),
                Value::Null,
                json!(h.alex_checking),
                json!("30.00"),
                Value::Null,
                json!("transfer")
            ),
        ]
    );
    // The bank row is recorded with it: the same file again is a duplicate.
    let again = upload(&pool, &h, "?file_name=march.ofx", &ofx()).await.id();
    let etag = read(&pool, &h, &again).await.etag();
    call(
        &pool,
        "PATCH",
        format!("/api/v1/imports/{again}/statements/0"),
    )
    .token(&h.t)
    .if_match(&etag)
    .json(json!({ "account_id": h.alex_credit }))
    .send()
    .await;
    assert_eq!(
        rows(&pool, &h, &again).await[0]["status"],
        json!("duplicate")
    );
}
