//! `/api/v1/prices` (design §3.4): manual prices, and valuations that
//! prefer one price to a chain. Needs `just db-up`.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::StatusCode;
use common::household::{H, create};
use outbound::prices::{Error, Kind, Pair, PriceSource, Quote};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::Date;
use time::macros::format_description;

async fn fetched(pool: &PgPool, base: &str, quote: &str, date: &str, rate: &str, source: &str) {
    sqlx::query(
        "INSERT INTO price (id, base_id, quote_id, date, rate, source)
         VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3::date, $4::numeric, $5)",
    )
    .bind(base)
    .bind(quote)
    .bind(date)
    .bind(rate)
    .bind(source)
    .execute(pool)
    .await
    .unwrap();
}

async fn fiat(h: &H, pool: &PgPool, code: &str) -> String {
    create(
        pool,
        &h.t,
        "/api/v1/commodities",
        json!({ "code": code, "kind": "fiat", "precision": 2 }),
    )
    .await
}

async fn value(h: &H, pool: &PgPool, query: &str) -> (StatusCode, Value) {
    let r = h
        .send(pool, "GET", &format!("/api/v1/prices/value?{query}"), None)
        .await;
    (r.status, r.body)
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn manual_prices(pool: PgPool) {
    let h = H::new(&pool).await;
    let eur = fiat(&h, &pool, "EUR").await;
    let new = json!({ "base_id": eur, "quote_id": h.usd, "date": "2026-06-01", "rate": "1.0842" });
    let r = h
        .send(&pool, "POST", "/api/v1/prices", Some(new.clone()))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["source"], "manual");
    assert_eq!(r.body["rate"], "1.0842");
    let (id, etag) = (r.id(), r.etag());

    let again = h.send(&pool, "POST", "/api/v1/prices", Some(new)).await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.body);
    for bad in [
        json!({ "base_id": eur, "quote_id": h.usd, "date": "2026-06-02", "rate": "0" }),
        json!({ "base_id": eur, "quote_id": h.usd, "date": "2026-06-02", "rate": 1.1 }),
        json!({ "base_id": eur, "quote_id": eur, "date": "2026-06-02", "rate": "1" }),
    ] {
        let r = h
            .send(&pool, "POST", "/api/v1/prices", Some(bad.clone()))
            .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.body
        );
    }

    let (status, v) = value(&h, &pool, "from=USD&to=EUR&date=2026-06-03").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["prices"][0]["id"], json!(id));
    assert_eq!(v["prices"][0]["inverted"], true);
    assert_eq!(v["derived"], false);
    assert_eq!(v["stale"], false);

    let r = common::call(&pool, "PATCH", format!("/api/v1/prices/{id}"))
        .token(&h.t)
        .if_match(&etag)
        .json(json!({ "rate": "1.25" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let (_, v) = value(&h, &pool, "from=USD&to=EUR&date=2026-06-03").await;
    assert_eq!(v["rate"], "0.8");

    let (status, _) = value(&h, &pool, "from=USD&to=EUR&date=2026-05-31").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = value(&h, &pool, "from=USD&to=XYZ").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let r = common::call(&pool, "DELETE", format!("/api/v1/prices/{id}"))
        .token(&h.t)
        .if_match(&r.etag())
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let (status, _) = value(&h, &pool, "from=USD&to=EUR&date=2026-06-03").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn one_source_before_a_chain(pool: PgPool) {
    let h = H::new(&pool).await;
    let eur = fiat(&h, &pool, "EUR").await;
    let gbp = fiat(&h, &pool, "GBP").await;
    fetched(&pool, &eur, &h.usd, "2026-06-01", "1.25", "provider:ecb").await;
    fetched(&pool, &eur, &gbp, "2026-06-01", "0.80", "provider:ecb").await;

    // Only the ECB's two rates connect GBP and USD: derived, one source.
    let (_, v) = value(&h, &pool, "from=GBP&to=USD&date=2026-06-01").await;
    assert_eq!(v["rate"], "1.5625");
    assert_eq!(v["derived"], true);
    assert_eq!(v["prices"].as_array().unwrap().len(), 2);

    // A direct quote wins, whatever its source.
    fetched(&pool, &gbp, &h.usd, "2026-06-01", "1.55", "provider:tiingo").await;
    let (_, v) = value(&h, &pool, "from=GBP&to=USD&date=2026-06-01").await;
    assert_eq!(v["rate"], "1.55");
    assert_eq!(v["derived"], false);
    assert_eq!(v["prices"][0]["source"], "provider:tiingo");

    // A fetched price can't be edited.
    let list = h
        .send(&pool, "GET", "/api/v1/prices?source=provider:tiingo", None)
        .await;
    let fetched_id = list.body["items"][0]["id"].as_str().unwrap().to_owned();
    let one = h
        .send(&pool, "GET", &format!("/api/v1/prices/{fetched_id}"), None)
        .await;
    let r = common::call(&pool, "PATCH", format!("/api/v1/prices/{fetched_id}"))
        .token(&h.t)
        .if_match(&one.etag())
        .json(json!({ "rate": "1.6" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
}

/// How a fake source fails, while it does.
type Failure = Arc<Mutex<Option<fn() -> Error>>>;

/// A price source that answers from a list, or fails while `fail` says
/// how, and counts what it's asked.
struct Fake {
    name: &'static str,
    /// `(base, quote, date, rate)`.
    quotes: Vec<(&'static str, &'static str, &'static str, &'static str)>,
    fail: Failure,
    asked: Arc<Mutex<Vec<String>>>,
}

impl PriceSource for Fake {
    fn name(&self) -> &str {
        self.name
    }

    fn plan(&self, base: &str, _: Kind, quote: &str, _: Kind) -> Option<Vec<Pair>> {
        Some(vec![Pair {
            base: base.into(),
            quote: quote.into(),
            kind: Kind::Currency,
        }])
    }

    fn fetch(&self, pairs: &[Pair], from: Date, to: Date) -> Result<Vec<Quote>, Error> {
        for p in pairs {
            self.asked
                .lock()
                .unwrap()
                .push(format!("{} {}{} {from} {to}", self.name, p.base, p.quote));
        }
        if let Some(fail) = *self.fail.lock().unwrap() {
            return Err(fail());
        }
        Ok(self
            .quotes
            .iter()
            .filter(|(b, q, _, _)| pairs.iter().any(|p| p.base == *b && p.quote == *q))
            .map(|(b, q, d, r)| Quote {
                base: (*b).into(),
                quote: (*q).into(),
                date: Date::parse(d, format_description!("[year]-[month]-[day]")).unwrap(),
                rate: (*r).into(),
            })
            .collect())
    }
}

fn fake(
    name: &'static str,
    fail: Option<fn() -> Error>,
    quotes: Vec<(&'static str, &'static str, &'static str, &'static str)>,
    asked: &Arc<Mutex<Vec<String>>>,
) -> Arc<dyn PriceSource> {
    Arc::new(Fake {
        name,
        quotes,
        fail: Arc::new(Mutex::new(fail)),
        asked: Arc::clone(asked),
    })
}

/// Asks for a valuation and, if prices are being fetched, waits for them.
async fn settled(
    pool: &PgPool,
    h: &H,
    pricing: &api::Pricing,
    q: &str,
) -> (StatusCode, Value, bool) {
    let r = common::call(pool, "GET", format!("/api/v1/prices/value?{q}"))
        .token(&h.t)
        .pricing(pricing.clone())
        .send()
        .await;
    let fetching = r.body["fetching"] == true
        || r.body["detail"]
            .as_str()
            .is_some_and(|d| d.contains("fetching"));
    pricing.settled().await;
    (r.status, r.body, fetching)
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn missing_prices_are_fetched_in_source_order(pool: PgPool) {
    let h = H::new(&pool).await;
    fiat(&h, &pool, "GBP").await;
    let asked = Arc::new(Mutex::new(Vec::new()));
    let pricing = api::Pricing::new(
        vec![
            fake("first", Some(|| Error::RateLimited), vec![], &asked),
            fake("second", None, vec![], &asked),
            fake(
                "third",
                None,
                vec![("GBP", "USD", "2026-06-01", "1.27")],
                &asked,
            ),
            fake(
                "fourth",
                None,
                vec![("GBP", "USD", "2026-06-01", "9")],
                &asked,
            ),
        ],
        5,
    );
    let get = |q: &'static str| settled(&pool, &h, &pricing, q);

    // Nothing stored: the answer comes at once, and the fetch after it.
    // The first fails, the second has nothing, the third answers: the
    // fourth isn't asked. The range goes back to cover a few fresh days.
    let (status, _, fetching) = get("from=GBP&to=USD&date=2026-06-02").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(fetching);
    assert_eq!(
        *asked.lock().unwrap(),
        [
            "first GBPUSD 2026-05-28 2026-06-02",
            "second GBPUSD 2026-05-28 2026-06-02",
            "third GBPUSD 2026-05-28 2026-06-02",
        ]
    );
    let (status, v, fetching) = get("from=GBP&to=USD&date=2026-06-02").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["rate"], "1.27");
    assert_eq!(v["prices"][0]["source"], "provider:third");
    assert!(!fetching);

    // Stored now: nobody is asked again.
    asked.lock().unwrap().clear();
    let (status, _, fetching) = get("from=USD&to=GBP&date=2026-06-03").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!fetching);
    assert!(asked.lock().unwrap().is_empty());

    // Stale for a later day: those days weren't asked for, so they are,
    // after the stale answer. The first is still rate-limited: passed over.
    let (_, v, fetching) = get("from=GBP&to=USD&date=2026-07-01").await;
    assert_eq!(v["stale"], true);
    assert!(fetching);
    assert_eq!(
        *asked.lock().unwrap(),
        [
            "second GBPUSD 2026-06-26 2026-07-01",
            "third GBPUSD 2026-06-26 2026-07-01",
        ]
    );

    // The same days within the hour, either way round: nobody again.
    asked.lock().unwrap().clear();
    let (_, _, fetching) = get("from=GBP&to=USD&date=2026-07-01").await;
    assert!(!fetching);
    get("from=USD&to=GBP&date=2026-07-01").await;
    assert!(asked.lock().unwrap().is_empty());

    // A refresh asks anyway, the first too, and answers before it's done.
    let refresh = |method: &'static str| {
        let call = common::call(&pool, method, "/api/v1/prices/refresh")
            .token(&h.t)
            .pricing(pricing.clone());
        if method == "POST" {
            call.json(json!({ "from": "2026-06-01", "to": "2026-06-05" }))
        } else {
            call
        }
        .send()
    };
    let r = refresh("GET").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "none yet");
    let r = refresh("POST").await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.body);
    assert_eq!(r.body["running"], true);
    pricing.settled().await;
    let r = refresh("GET").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["running"], false);
    assert_eq!(
        r.body["sources"],
        json!(["first", "second", "third", "fourth"])
    );
    assert_eq!(r.body["stored"], 1);
    assert_eq!(r.body["errors"], json!(["first: rate-limited"]));
    assert_eq!(asked.lock().unwrap().len(), 3);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn an_outage_is_asked_again_soon(pool: PgPool) {
    let h = H::new(&pool).await;
    fiat(&h, &pool, "GBP").await;
    fiat(&h, &pool, "EUR").await;
    let asked = Arc::new(Mutex::new(Vec::new()));
    let fail: Failure = Arc::new(Mutex::new(Some(|| Error::Transport("no network".into()))));
    let source = Arc::new(Fake {
        name: "net",
        quotes: vec![
            ("GBP", "USD", "2026-06-01", "1.27"),
            ("EUR", "USD", "2026-06-01", "1.1"),
        ],
        fail: Arc::clone(&fail),
        asked: Arc::clone(&asked),
    });
    let pricing = api::Pricing::new(vec![source], 5).outage(Duration::from_millis(200));
    let get = |q: &'static str| settled(&pool, &h, &pricing, q);

    // Down: asked once for both pairs, not once per pair.
    let r = common::call(&pool, "POST", "/api/v1/prices/refresh")
        .token(&h.t)
        .pricing(pricing.clone())
        .json(json!({ "from": "2026-06-01", "to": "2026-06-01" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.body);
    pricing.settled().await;
    let r = common::call(&pool, "GET", "/api/v1/prices/refresh")
        .token(&h.t)
        .pricing(pricing.clone())
        .send()
        .await;
    assert_eq!(r.body["errors"], json!(["net: unreachable: no network"]));
    assert_eq!(asked.lock().unwrap().len(), 1);

    // Passed over for a while: nothing to fetch, so not fetching.
    let (status, _, fetching) = get("from=GBP&to=USD&date=2026-06-01").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!fetching);
    assert_eq!(asked.lock().unwrap().len(), 1);

    // Back, and the wait is over: asked again, not an hour later.
    *fail.lock().unwrap() = None;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (_, _, fetching) = get("from=GBP&to=USD&date=2026-06-01").await;
    assert!(fetching);
    let (status, v, _) = get("from=GBP&to=USD&date=2026-06-01").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["rate"], "1.27");
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn settlement_and_balances_use_prices(pool: PgPool) {
    let h = H::new(&pool).await;
    let eur = fiat(&h, &pool, "EUR").await;
    let euro_account = create(
        &pool,
        &h.t,
        "/api/v1/accounts",
        json!({ "name": "Alex Euro", "kind": "checking", "owner_id": h.alex }),
    )
    .await;
    // Alex pays EUR 100 for the household.
    let leg =
        |side: (&str, &str)| json!({ side.0: side.1, "commodity_id": eur, "quantity": "100.00" });
    let r = h
        .send(
            &pool,
            "POST",
            "/api/v1/transactions",
            Some(
                json!({ "date": "2026-06-05", "description": "x", "splits": [{
                    "from": leg(("account_id", &euro_account)),
                    "to": leg(("counterparty_id", &h.store)),
                    "purpose_id": h.household,
                    "category_id": h.shopping,
                }]}),
            ),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);

    let statement = "/api/v1/settlements/2026-06/statement";
    let st = h.send(&pool, "GET", statement, None).await;
    assert_eq!(st.status, StatusCode::OK, "{}", st.body);
    assert_eq!(st.body["unpriced"].as_array().unwrap().len(), 1);
    let balances = "/api/v1/reports/balances?date=2026-06-30&value_in=USD";
    let b = h.send(&pool, "GET", balances, None).await;
    assert_eq!(b.status, StatusCode::OK, "{}", b.body);
    let euro = |b: &Value| {
        b["balances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["account_id"] == json!(euro_account))
            .unwrap()
            .clone()
    };
    assert_eq!(euro(&b.body)["balance"], "-100.00");
    assert!(euro(&b.body).get("value").is_none(), "no price yet");

    // A price on the day before: valued at it, and it says so.
    let r = h
        .send(
            &pool,
            "POST",
            "/api/v1/prices",
            Some(json!({ "base_id": eur, "quote_id": h.usd, "date": "2026-06-04", "rate": "1.1" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let st = h.send(&pool, "GET", statement, None).await;
    assert!(
        st.body["unpriced"].as_array().unwrap().is_empty(),
        "{}",
        st.body
    );
    let lines: Vec<Value> = st.body["edges"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| e["lines"].as_array().unwrap().clone())
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    // The household owes Alex: from parent to child.
    assert_eq!(lines[0]["amount"], "-110.00");
    assert_eq!(lines[0]["valuation"]["rate"], "1.1");
    assert_eq!(lines[0]["valuation"]["prices"][0]["source"], "manual");

    let b = h.send(&pool, "GET", balances, None).await;
    assert_eq!(b.body["value_in_id"], json!(h.usd));
    assert_eq!(euro(&b.body)["value"], "-110.00");
    assert_eq!(euro(&b.body)["valuation"]["stale"], true, "26 days old");
    let usd_row = b.body["balances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["commodity_id"] == json!(h.usd));
    if let Some(row) = usd_row {
        assert_eq!(row["value"], row["balance"]);
        assert!(row.get("valuation").is_none());
    }
}
