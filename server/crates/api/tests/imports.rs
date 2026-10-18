//! `/api/v1/imports`: uploading bank files, resolving their accounts, and
//! what each row would become. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::H;
use common::{Response, call};
use serde_json::{Value, json};
use sqlx::PgPool;

const CARD: &str = "4111000011112222";

fn ofx(number: &str) -> String {
    format!(
        "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\nSECURITY:NONE\r\n\
ENCODING:USASCII\r\nCHARSET:1252\r\nCOMPRESSION:NONE\r\nOLDFILEUID:NONE\r\nNEWFILEUID:NONE\r\n\r\n\
<OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<DTSERVER>20260331120000<LANGUAGE>ENG</SONRS></SIGNONMSGSRSV1>\
<CREDITCARDMSGSRSV1><CCSTMTTRNRS><TRNUID>1<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<CCSTMTRS><CURDEF>USD<CCACCTFROM><ACCTID>{number}</CCACCTFROM>\
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
03/03/2026,PAYROLL,\"1,500.00\"\n\
13/04/2026,BAD DATE,-1.00\n";

async fn upload(pool: &PgPool, h: &H, query: &str, file: &str) -> Response {
    call(pool, "POST", format!("/api/v1/imports{query}"))
        .token(&h.t)
        .bytes(file.as_bytes().to_vec())
        .send()
        .await
}

async fn rows(pool: &PgPool, h: &H, id: &str, query: &str) -> Vec<Value> {
    let r = h
        .send(
            pool,
            "GET",
            &format!("/api/v1/imports/{id}/rows{query}"),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    r.body["items"].as_array().unwrap().clone()
}

fn statuses(rows: &[Value]) -> Vec<&str> {
    rows.iter().map(|r| r["status"].as_str().unwrap()).collect()
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn files_that_cant_be_staged(pool: PgPool) {
    let h = H::new(&pool).await;
    for (query, file) in [
        ("", ""),
        ("", "just some text\n"),
        ("?account_id=00000000-0000-0000-0000-000000000000", CSV),
        ("?file_name=%20", CSV),
    ] {
        let r = upload(&pool, &h, query, file).await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{query} {}",
            r.body
        );
    }
    let r = upload(
        &pool,
        &h,
        &format!("?account_id={}", h.alex_checking),
        &ofx(CARD),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    // The file's number, once known, must agree with the account given.
    let id = r.id();
    let path = format!("/api/v1/imports/{id}/statements/0");
    let r = call(&pool, "PATCH", &path)
        .token(&h.t)
        .if_match(&r.etag())
        .json(json!({ "account_id": h.alex_checking, "remember_number": true }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let r = upload(
        &pool,
        &h,
        &format!("?account_id={}", h.alex_credit),
        &ofx(CARD),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
    assert!(r.body["detail"].as_str().unwrap().contains("…2222"));
    assert!(!r.body.to_string().contains(CARD));
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn an_unknown_number_waits_for_its_account(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = upload(&pool, &h, "?file_name=march.ofx", &ofx(CARD)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert!(!r.body.to_string().contains(CARD));
    let s = &r.body["statements"][0];
    assert_eq!(
        (
            r.body["format"].as_str(),
            r.body["file_name"].as_str(),
            s["number_last4"].as_str(),
            s["kind"].as_str(),
            &s["account_id"],
        ),
        (
            Some("ofx"),
            Some("march.ofx"),
            Some("2222"),
            Some("credit_card"),
            &Value::Null
        )
    );
    assert_eq!(
        s["ledger"],
        json!({ "amount": "-50.00", "date": "2026-03-31" })
    );
    assert_eq!(s["counts"]["needs_account"], json!(2));
    let id = r.id();
    let all = rows(&pool, &h, &id, "").await;
    assert_eq!(statuses(&all), ["needs_account", "needs_account"]);
    assert_eq!(
        (
            all[0]["amount"].as_str(),
            all[0]["description"].as_str(),
            all[0]["native_id"].as_str()
        ),
        (Some("-80.00"), Some("GROCER"), Some("F1"))
    );
    assert_eq!(all[0]["raw"]["FITID"], json!("F1"));

    // Setting the account, and remembering the number for next time.
    let path = format!("/api/v1/imports/{id}/statements/0");
    let r = call(&pool, "PATCH", &path)
        .token(&h.t)
        .if_match("\"999\"")
        .json(json!({ "account_id": h.alex_credit }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED);
    let etag = h
        .send(&pool, "GET", &format!("/api/v1/imports/{id}"), None)
        .await
        .etag();
    let r = call(&pool, "PATCH", &path)
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "account_id": h.alex_credit, "remember_number": true }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["statements"][0]["counts"]["new"], json!(2));
    assert_ne!(r.etag(), etag);
    let card = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/accounts/{}", h.alex_credit),
            None,
        )
        .await;
    assert_eq!(card.body["numbers"][0]["last4"], json!("2222"));

    // The next file from that card finds its account by itself.
    let r = upload(&pool, &h, "", &ofx(CARD)).await;
    assert_eq!(r.body["statements"][0]["account_id"], json!(h.alex_credit));

    // Discarding.
    let r = call(&pool, "DELETE", format!("/api/v1/imports/{id}"))
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::PRECONDITION_FAILED);
    let etag = h
        .send(&pool, "GET", &format!("/api/v1/imports/{id}"), None)
        .await
        .etag();
    let r = call(&pool, "DELETE", format!("/api/v1/imports/{id}"))
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = h
        .send(&pool, "GET", &format!("/api/v1/imports/{id}"), None)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let left = h.send(&pool, "GET", "/api/v1/imports", None).await;
    assert_eq!(left.body["items"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_csv_without_an_account_waits_for_one(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = upload(&pool, &h, "", CSV).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let s = &r.body["statements"][0];
    assert_eq!(
        (&s["account_id"], s.get("number_last4")),
        (&Value::Null, None)
    );
    assert_eq!(s["counts"]["needs_account"], json!(3));
    let path = format!("/api/v1/imports/{}/statements/0", r.id());
    // There's no number to remember.
    let bad = call(&pool, "PATCH", &path)
        .token(&h.t)
        .if_match(&r.etag())
        .json(json!({ "account_id": h.alex_checking, "remember_number": true }))
        .send()
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = call(&pool, "PATCH", &path)
        .token(&h.t)
        .if_match(&r.etag())
        .json(json!({ "account_id": h.alex_checking }))
        .send()
        .await;
    assert_eq!(r.body["statements"][0]["counts"]["new"], json!(3));
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn rows_already_imported_are_duplicates(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let s = &r.body["statements"][0];
    assert_eq!(s["counts"]["new"], json!(3));
    assert_eq!(
        s["skipped"],
        json!([{ "line": 5, "reason": "unreadable date" }])
    );
    let first = r.id();

    // Pretend the second coffee was imported before.
    let t = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(
                json!({ "date": "2026-03-02", "description": "COFFEE", "splits": [{
                "from": h.leg(h.acct(&h.alex_checking), "4.50"),
                "to": h.leg(("counterparty_id", &h.store), "4.50"), "purpose_id": h.alex }] }),
            ),
        )
        .await
        .id();
    let coffee = rows(&pool, &h, &first, "").await[1]["raw"].clone();
    assert_eq!(coffee["Date"], json!("03/02/2026"));
    let fingerprint: String = sqlx::query_scalar(
        "SELECT fingerprint FROM staged_row WHERE import_id = $1::uuid AND position = 1",
    )
    .bind(&first)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO import_fingerprint VALUES ($1::uuid, $2, $3::uuid)")
        .bind(&h.alex_checking)
        .bind(&fingerprint)
        .bind(&t)
        .execute(&pool)
        .await
        .unwrap();

    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let id = r.id();
    assert_eq!(r.body["statements"][0]["counts"]["duplicate"], json!(1));
    let all = rows(&pool, &h, &id, "").await;
    assert_eq!(statuses(&all), ["new", "duplicate", "new"]);
    assert_eq!(all[1]["match_transaction_id"], json!(t));
    // The same rows in another account aren't duplicates.
    let r = upload(&pool, &h, &format!("?account_id={}", h.joint_savings), CSV).await;
    assert_eq!(r.body["statements"][0]["counts"]["new"], json!(3));

    // Paging and filtering.
    let page = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/imports/{id}/rows?limit=2"),
            None,
        )
        .await;
    let cursor = page.body["next_cursor"].as_str().unwrap();
    let rest = rows(&pool, &h, &id, &format!("?limit=2&cursor={cursor}")).await;
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0]["description"], json!("PAYROLL"));
    let dups = rows(&pool, &h, &id, "?status=duplicate").await;
    assert_eq!(dups.len(), 1);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn rows_like_recorded_transactions_are_near_duplicates(pool: PgPool) {
    let h = H::new(&pool).await;
    // With someone outside, it's income or an expense: for the Household.
    let tx = |date: &str, desc: &str, from: Value, to: Value| {
        let outside = from.get("counterparty_id").is_some() || to.get("counterparty_id").is_some();
        let mut split = json!({ "from": from, "to": to });
        if outside {
            split["purpose_id"] = json!(h.household);
        }
        json!({ "date": date, "description": desc, "splits": [split] })
    };
    let store = ("counterparty_id", h.store.as_str());
    let mut made = Vec::new();
    for body in [
        // One coffee entered by hand, a day after the bank's.
        tx(
            "2026-03-02",
            "Coffee",
            h.leg(h.acct(&h.alex_checking), "4.50"),
            h.leg(store, "4.50"),
        ),
        // Pay in the right direction, two days later.
        tx(
            "2026-03-05",
            "Salary",
            h.leg(store, "1500.00"),
            h.leg(h.acct(&h.alex_checking), "1500.00"),
        ),
        // Not near: the wrong direction, another account, too far apart.
        tx(
            "2026-03-03",
            "PAYROLL",
            h.leg(h.acct(&h.alex_checking), "1500.00"),
            h.leg(store, "1500.00"),
        ),
        tx(
            "2026-03-01",
            "COFFEE",
            h.leg(h.acct(&h.joint_savings), "4.50"),
            h.leg(store, "4.50"),
        ),
        tx(
            "2026-03-07",
            "PAYROLL",
            h.leg(store, "1500.00"),
            h.leg(h.acct(&h.alex_checking), "1500.00"),
        ),
    ] {
        let r = h
            .send(&pool, "POST", "/api/v1/transactions", Some(body))
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
        made.push(r.id());
    }
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let all = rows(&pool, &h, &r.id(), "").await;
    // Two identical coffees, one recorded: only the one on the same day is
    // flagged.
    assert_eq!(statuses(&all), ["new", "near_duplicate", "near_duplicate"]);
    assert_eq!(all[1]["match_transaction_id"], json!(made[0]));
    assert_eq!(all[2]["match_transaction_id"], json!(made[1]));
    assert_eq!(
        r.body["statements"][0]["counts"]["near_duplicate"],
        json!(2)
    );
    // A near duplicate is never sure: the same text on the same day is 89%.
    let s = &all[1]["suggested_decision"];
    assert_eq!(
        (
            &s["decision"],
            &s["confidence"],
            &s["confident"],
            &s["reason"]
        ),
        (
            &json!("match"),
            &json!(890),
            &json!(false),
            &json!("the same amount, the same day, descriptions 100% alike")
        )
    );
    // Whether they're the same is yours to say; the new row needs labels.
    let ready = &r.body["readiness"];
    assert_eq!(
        (&ready["state"], &ready["undecided"], &ready["unlabeled"]),
        (&json!("needs_work"), &json!(2), &json!(1))
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn rows_whose_transfer_is_already_recorded(pool: PgPool) {
    let h = H::new(&pool).await;
    // With someone outside, it's income or an expense: for the Household.
    let one = |date: &str, from: Value, to: Value| {
        let outside = from.get("counterparty_id").is_some() || to.get("counterparty_id").is_some();
        let mut split = json!({ "from": from, "to": to });
        if outside {
            split["purpose_id"] = json!(h.household);
        }
        json!({ "date": date, "description": "x", "splits": [split] })
    };
    let store = ("counterparty_id", h.store.as_str());
    let savings = h.acct(&h.joint_savings);
    let checking = h.acct(&h.alex_checking);
    let mut made = Vec::new();
    for body in [
        // A transfer from savings into checking, three days after the pay
        // row: that row is its checking side.
        one(
            "2026-03-06",
            h.leg(savings, "1500.00"),
            h.leg(checking, "1500.00"),
        ),
        // Coffee money moved from checking to savings: the other side of a
        // coffee row leaving checking.
        one(
            "2026-03-01",
            h.leg(checking, "4.50"),
            h.leg(savings, "4.50"),
        ),
        // Not: one-sided (no longer matched), the other direction, too far
        // apart.
        one("2026-03-02", h.leg(savings, "4.50"), h.leg(store, "4.50")),
        one(
            "2026-03-02",
            h.leg(savings, "4.50"),
            h.leg(checking, "4.50"),
        ),
        one(
            "2026-03-10",
            h.leg(checking, "4.50"),
            h.leg(savings, "4.50"),
        ),
    ] {
        let r = h
            .send(&pool, "POST", "/api/v1/transactions", Some(body))
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
        made.push(r.id());
    }
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let all = rows(&pool, &h, &r.id(), "").await;
    assert_eq!(
        statuses(&all),
        ["transfer_duplicate", "new", "transfer_duplicate"]
    );
    assert_eq!(all[0]["match_transaction_id"], json!(made[1]));
    assert_eq!(all[2]["match_transaction_id"], json!(made[0]));
    assert_eq!(all[0]["decision"], json!("match"));
    assert_eq!(
        r.body["statements"][0]["counts"]["transfer_duplicate"],
        json!(2)
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn fingerprints_recorded_for_existing_transactions(pool: PgPool) {
    let h = H::new(&pool).await;
    let coffee = |date: &str| {
        json!({ "date": date, "description": "Coffee", "splits": [{
            "from": h.leg(h.acct(&h.alex_checking), "4.50"),
            "to": h.leg(("counterparty_id", &h.store), "4.50"), "purpose_id": h.alex }] })
    };
    let a = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(coffee("2026-02-01")),
        )
        .await
        .id();
    let b = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(coffee("2026-02-02")),
        )
        .await
        .id();
    // The CSV's first coffee, as the old tool fingerprinted it.
    let first = upload(&pool, &h, &format!("?account_id={}", h.joint_savings), CSV).await;
    let fp: String = sqlx::query_scalar(
        "SELECT fingerprint FROM staged_row WHERE import_id = $1::uuid AND position = 0",
    )
    .bind(first.id())
    .fetch_one(&pool)
    .await
    .unwrap();

    let post = |items: Value| {
        let pool = pool.clone();
        let t = h.t.clone();
        async move {
            call(&pool, "POST", "/api/v1/import-fingerprints")
                .token(&t)
                .json(json!({ "items": items }))
                .send()
                .await
        }
    };
    let item = |account: &str, fp: &str, t: &str| json!({ "account_id": account, "fingerprint": fp, "transaction_id": t });
    for bad in [
        json!([]),
        json!([item(&h.alex_checking, "", &a)]),
        json!([item(&h.joint_savings, "X1", &a)]), // no split on that account
        json!([
            item(&h.alex_checking, "X1", &a),
            item(&h.alex_checking, "X1", &b)
        ]),
    ] {
        let r = post(bad.clone()).await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad} {}",
            r.body
        );
    }
    let r = post(json!([
        item(&h.alex_checking, &fp, &a),
        item(&h.alex_checking, "X1", &a)
    ]))
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        r.body,
        json!({ "added": 2, "unchanged": 0, "conflicts": [] })
    );
    let r = post(json!([
        item(&h.alex_checking, &fp, &a),
        item(&h.alex_checking, "X1", &b)
    ]))
    .await;
    assert_eq!(
        r.body,
        json!({ "added": 0, "unchanged": 1,
                "conflicts": [item(&h.alex_checking, "X1", &b)] })
    );

    // Now the row is an exact duplicate of the transaction.
    let r = upload(&pool, &h, &format!("?account_id={}", h.alex_checking), CSV).await;
    let all = rows(&pool, &h, &r.id(), "").await;
    assert_eq!(all[0]["status"], json!("duplicate"));
    assert_eq!(all[0]["match_transaction_id"], json!(a));

    // Deleting the transaction forgets its fingerprints.
    let etag = h
        .send(&pool, "GET", &format!("/api/v1/transactions/{a}"), None)
        .await
        .etag();
    call(&pool, "DELETE", format!("/api/v1/transactions/{a}"))
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM import_fingerprint")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
