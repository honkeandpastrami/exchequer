//! Transfers in staging (design §6.4, docs/ai.md): a row's other side may
//! be one of your accounts; two staged rows, in different files, that are
//! the two sides of one transfer become a transfer match and are committed
//! as one transfer; a transfer already in the ledger makes its other side a
//! transfer duplicate. Needs `just db-up`.

mod common;

use axum::http::StatusCode;
use common::household::H;
use common::{Response, call};
use serde_json::{Value, json};
use sqlx::PgPool;

const CARD: &str = "Date,Description,Amount\n\
                    03/12/2026,TRANSFER FROM CHECKING,642.18\n";
const CHECKING: &str = "Date,Description,Amount\n\
                        03/13/2026,ONLINE PAYMENT TO CARD,-642.18\n\
                        03/14/2026,COFFEE,-4.50\n";

async fn upload(pool: &PgPool, h: &H, account: &str, file: &str) -> String {
    let r = call(
        pool,
        "POST",
        format!("/api/v1/imports?account_id={account}"),
    )
    .token(&h.t)
    .bytes(file.as_bytes().to_vec())
    .send()
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.id()
}

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

async fn commit(pool: &PgPool, h: &H, id: &str) -> Response {
    let etag = read(pool, h, id).await.etag();
    call(pool, "POST", format!("/api/v1/imports/{id}/commit"))
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await
}

fn status(row: &Value) -> (&Value, &Value) {
    (&row["status"], &row["decision"])
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn two_files_two_sides_one_transfer(pool: PgPool) {
    let h = H::new(&pool).await;
    let card = upload(&pool, &h, &h.alex_credit, CARD).await;
    let checking = upload(&pool, &h, &h.alex_checking, CHECKING).await;
    // Nothing settled: two new rows, each needing labels.
    let deposit = &rows(&pool, &h, &card).await[0];
    assert_eq!(status(deposit), (&json!("new"), &json!("new")));

    // The payment's other side is the card: the two rows are one transfer.
    let card_version = read(&pool, &h, &card).await.etag();
    let r = set_row(
        &pool,
        &h,
        &checking,
        0,
        json!({ "transfer_account_id": h.alex_credit }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (
            status(&r.body),
            &r.body["transfer_account_id"],
            &r.body["transfer_match"]["import_id"],
            &r.body["transfer_match"]["account_id"],
            &r.body["missing"],
        ),
        (
            (&json!("transfer_match"), &json!("match")),
            &json!(h.alex_credit),
            &json!(card),
            &json!(h.alex_credit),
            &json!([])
        ),
        "{}",
        r.body
    );
    assert_eq!(r.body.get("counterparty_id"), None);
    // The card's row too, in its own import, which changed.
    assert_ne!(read(&pool, &h, &card).await.etag(), card_version);
    let deposit = &rows(&pool, &h, &card).await[0];
    assert_eq!(
        (
            status(deposit),
            &deposit["transfer_account_id"],
            &deposit["transfer_match"]["import_id"],
            &deposit["missing"],
        ),
        (
            (&json!("transfer_match"), &json!("match")),
            &json!(h.alex_checking),
            &json!(checking),
            &json!([])
        ),
        "{deposit}"
    );
    let counts = &read(&pool, &h, &card).await.body["statements"][0]["counts"];
    assert_eq!(counts["transfer_match"], json!(1));

    // Committing either side records the one transfer, dated and described
    // as the side the money left.
    let r = commit(&pool, &h, &card).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (&r.body["created"], &r.body["transfers"]),
        (&json!(1), &json!(1))
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
    let t = &list.body["items"][0];
    let s = &t["splits"][0];
    assert_eq!(
        (
            &t["date"],
            &t["description"],
            &s["kind"],
            &s["from"]["account_id"],
            &s["to"]["account_id"],
            s.get("category_id"),
            s.get("purpose_id"),
        ),
        (
            &json!("2026-03-13"),
            &json!("ONLINE PAYMENT TO CARD"),
            &json!("transfer"),
            &json!(h.alex_checking),
            &json!(h.alex_credit),
            None,
            None
        ),
        "{t}"
    );
    // The payment is now a duplicate of it: nothing more to add.
    let payment = &rows(&pool, &h, &checking).await[0];
    assert_eq!(
        (status(payment), &payment["match_transaction_id"]),
        ((&json!("duplicate"), &json!("match")), &t["id"])
    );
    set_row(&pool, &h, &checking, 1, json!({ "decision": "skip" })).await;
    let r = commit(&pool, &h, &checking).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (&r.body["created"], &r.body["matched"]),
        (&json!(0), &json!(1))
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM transaction")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_pair_can_be_split_and_a_lone_transfer_committed(pool: PgPool) {
    let h = H::new(&pool).await;
    let card = upload(&pool, &h, &h.alex_credit, CARD).await;
    let checking = upload(&pool, &h, &h.alex_checking, CHECKING).await;
    set_row(
        &pool,
        &h,
        &checking,
        0,
        json!({ "transfer_account_id": h.alex_credit }),
    )
    .await;
    // Not the same transfer, you say: each row is new again.
    let r = set_row(&pool, &h, &checking, 0, json!({ "decision": "new" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (status(&r.body), &r.body["decided"]),
        ((&json!("new"), &json!("new")), &json!(true))
    );
    assert_eq!(
        status(&rows(&pool, &h, &card).await[0]),
        (&json!("new"), &json!("new"))
    );
    // Still a transfer to the card, alone: committed as one.
    set_row(&pool, &h, &checking, 1, json!({ "decision": "skip" })).await;
    let r = commit(&pool, &h, &checking).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (&r.body["created"], &r.body["transfers"]),
        (&json!(1), &json!(0))
    );
    // The card's row is now its other side, already recorded.
    let deposit = &rows(&pool, &h, &card).await[0];
    assert_eq!(
        status(deposit),
        (&json!("transfer_duplicate"), &json!("match"))
    );
    // You decide: it can still be added as new.
    let r = set_row(&pool, &h, &card, 0, json!({ "decision": "new" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_rule_settles_a_transfer_from_its_potential_matches(pool: PgPool) {
    let h = H::new(&pool).await;
    let etag = h.send(&pool, "GET", "/api/v1/rules", None).await.etag();
    let script = r#"
fn classify(row, s) {
    for c in row.transfer_candidates {
        if c.account == "Alex · Alex Credit" && row.description.contains("PAYMENT") {
            s.transfer("Alex · Alex Credit");
        }
    }
}
"#;
    let r = call(&pool, "PUT", "/api/v1/rules")
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "source": script }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let card = upload(&pool, &h, &h.alex_credit, CARD).await;
    // The card's row sees no payment staged yet; the payment sees the card's
    // row, and the rule settles it: they're matched on upload.
    let checking = upload(&pool, &h, &h.alex_checking, CHECKING).await;
    let payment = &rows(&pool, &h, &checking).await[0];
    assert_eq!(
        (
            status(payment),
            &payment["suggestions"][0]["transfer_account_id"],
            &payment["suggestions"][0]["reason"],
        ),
        (
            (&json!("transfer_match"), &json!("match")),
            &json!(h.alex_credit),
            &json!("rule v1:5")
        ),
        "{payment}"
    );
    assert_eq!(
        rows(&pool, &h, &card).await[0]["transfer_match"]["import_id"],
        json!(checking)
    );
    // Discarding one side ends the match.
    let etag = read(&pool, &h, &checking).await.etag();
    let r = call(&pool, "DELETE", format!("/api/v1/imports/{checking}"))
        .token(&h.t)
        .if_match(&etag)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        status(&rows(&pool, &h, &card).await[0]),
        (&json!("new"), &json!("new"))
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_liquidation_keeps_its_purpose(pool: PgPool) {
    let h = H::new(&pool).await;
    let file = "Date,Description,Amount\n03/10/2026,FROM BROKERAGE,10000.00\n";
    let checking = upload(&pool, &h, &h.alex_checking, file).await;
    // From an investment account into a liquid one: it still needs a
    // purpose and a category (design §4.1).
    let r = set_row(
        &pool,
        &h,
        &checking,
        0,
        json!({ "transfer_account_id": h.alex_brokerage }),
    )
    .await;
    assert_eq!(
        r.body["missing"],
        json!(["category", "purpose"]),
        "{}",
        r.body
    );
    // Between liquid accounts: neither.
    let r = set_row(
        &pool,
        &h,
        &checking,
        0,
        json!({ "transfer_account_id": h.joint_savings }),
    )
    .await;
    assert_eq!(r.body["missing"], json!([]), "{}", r.body);
    // Not to itself, and not both a counterparty and an account.
    let r = set_row(
        &pool,
        &h,
        &checking,
        0,
        json!({ "transfer_account_id": h.alex_checking }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = set_row(
        &pool,
        &h,
        &checking,
        0,
        json!({ "transfer_account_id": h.joint_savings, "counterparty_id": h.store }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

/// One file, two statements: checking and the card, each with its side of
/// the card payment.
const BOTH: &str = "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\nSECURITY:NONE\r\n\
ENCODING:USASCII\r\nCHARSET:1252\r\nCOMPRESSION:NONE\r\nOLDFILEUID:NONE\r\nNEWFILEUID:NONE\r\n\r\n\
<OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<DTSERVER>20260331120000<LANGUAGE>ENG</SONRS></SIGNONMSGSRSV1>\
<BANKMSGSRSV1><STMTTRNRS><TRNUID>1<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<STMTRS><CURDEF>USD<BANKACCTFROM><BANKID>000000000<ACCTID>11112222<ACCTTYPE>CHECKING</BANKACCTFROM>\
<BANKTRANLIST><DTSTART>20260301<DTEND>20260331\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260310<TRNAMT>-300.00<FITID>C1<NAME>PAYMENT TO CARD</STMTTRN>\
</BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1>\
<CREDITCARDMSGSRSV1><CCSTMTTRNRS><TRNUID>2<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<CCSTMTRS><CURDEF>USD<CCACCTFROM><ACCTID>33334444</CCACCTFROM>\
<BANKTRANLIST><DTSTART>20260301<DTEND>20260331\
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260309<TRNAMT>300.00<FITID>K1<NAME>PAYMENT THANK YOU</STMTTRN>\
</BANKTRANLIST></CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>";

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn both_sides_in_one_file(pool: PgPool) {
    let h = H::new(&pool).await;
    let r = call(&pool, "POST", "/api/v1/imports")
        .token(&h.t)
        .bytes(BOTH.as_bytes().to_vec())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let id = r.id();
    for (position, account) in [(0, &h.alex_checking), (1, &h.alex_credit)] {
        let etag = read(&pool, &h, &id).await.etag();
        let r = call(
            &pool,
            "PATCH",
            format!("/api/v1/imports/{id}/statements/{position}"),
        )
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "account_id": account }))
        .send()
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    }
    let r = set_row(
        &pool,
        &h,
        &id,
        0,
        json!({ "transfer_account_id": h.alex_credit }),
    )
    .await;
    assert_eq!(
        (&r.body["status"], &r.body["transfer_match"]["statement"]),
        (&json!("transfer_match"), &json!(1)),
        "{}",
        r.body
    );
    let r = commit(&pool, &h, &id).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (&r.body["created"], &r.body["transfers"]),
        (&json!(1), &json!(1))
    );
    // One transfer, with both bank rows.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM transaction")
        .fetch_one(&pool)
        .await
        .unwrap();
    let fps: i64 = sqlx::query_scalar("SELECT count(DISTINCT account_id) FROM import_fingerprint")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((n, fps), (1, 2));
}
