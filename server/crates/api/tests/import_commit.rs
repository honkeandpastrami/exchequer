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
    let store_cp = ("counterparty_id", h.store.as_str());
    let savings = h.acct(&h.joint_savings);
    // Pay recorded from the savings side as leaving to the outside.
    let pay = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(
                json!({ "date": "2026-03-04", "description": "To checking", "splits": [{
                "from": h.leg(savings, "1500.00"), "to": h.leg(store_cp, "1500.00"),
                "category_id": null }] }),
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
            (json!("transfer_match"), json!("match")),
        ]
    );

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
    let r = set_row(&pool, &h, &id, 1, json!({ "purpose_id": null })).await;
    assert_eq!(r.body.get("purpose_id"), None);
    assert_eq!(r.body["counterparty_id"], json!(h.store));

    let r = commit(&pool, &h, &id, &read(&pool, &h, &id).await.etag()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (&r.body["created"], &r.body["transfers"], &r.body["skipped"]),
        (&json!(1), &json!(1), &json!(1))
    );
    // The pay is now a transfer from savings into checking.
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
            "to": h.leg(h.acct(&h.alex_checking), "1500.00") }] }),
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

    // A closed month refuses the whole commit.
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
