//! The fictional household (Alex, Studio) for settlement and report tests,
//! created through the API.

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Response, call, new_token};

pub struct H {
    pub t: String,
    pub household: String,
    pub alex: String,
    pub studio: String,
    pub usd: String,
    pub alex_checking: String,
    pub alex_credit: String,
    pub alex_brokerage: String,
    pub joint_savings: String,
    pub store: String,
    pub shopping: String,
}

pub async fn create(pool: &PgPool, t: &str, path: &str, body: Value) -> String {
    let r = call(pool, "POST", path).token(t).json(body).send().await;
    assert_eq!(r.status, StatusCode::CREATED, "{path}: {}", r.body);
    r.id()
}

impl H {
    pub async fn new(pool: &PgPool) -> Self {
        let (_, t) = new_token(pool).await;
        let party = |name: &str, kind: &str| json!({ "name": name, "kind": kind });
        let household = create(pool, &t, "/api/v1/parties", party("Household", "household")).await;
        let alex = create(pool, &t, "/api/v1/parties", party("Alex", "person")).await;
        let studio = create(pool, &t, "/api/v1/parties", party("Studio", "business")).await;
        let usd = create(
            pool,
            &t,
            "/api/v1/commodities",
            json!({ "code": "USD", "kind": "fiat", "precision": 2 }),
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
        let store = create(
            pool,
            &t,
            "/api/v1/counterparties",
            json!({ "name": "Store" }),
        )
        .await;
        let shopping = create(
            pool,
            &t,
            "/api/v1/categories",
            json!({ "name": "Shopping" }),
        )
        .await;
        Self {
            t,
            household,
            alex,
            studio,
            usd,
            alex_checking,
            alex_credit,
            alex_brokerage,
            joint_savings,
            store,
            shopping,
        }
    }

    pub async fn send(
        &self,
        pool: &PgPool,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Response {
        let mut c = call(pool, method, path).token(&self.t);
        if let Some(b) = body {
            c = c.json(b);
        }
        c.send().await
    }

    pub fn leg(&self, side: (&str, &str), qty: &str) -> Value {
        json!({ side.0: side.1, "commodity_id": self.usd, "quantity": qty })
    }

    pub fn acct<'a>(&self, id: &'a str) -> (&'static str, &'a str) {
        ("account_id", id)
    }

    /// One split; `extra` adds fields such as `purpose_id` or `settles`.
    pub async fn tx(
        &self,
        pool: &PgPool,
        date: &str,
        from: (&str, &str),
        to: (&str, &str),
        qty: &str,
        extra: Value,
    ) -> Response {
        let mut split = json!({ "from": self.leg(from, qty), "to": self.leg(to, qty) });
        for (k, v) in extra.as_object().unwrap() {
            split[k] = v.clone();
        }
        self.send(
            pool,
            "POST",
            "/api/v1/transactions",
            Some(json!({ "date": date, "description": "x", "splits": [split] })),
        )
        .await
    }

    pub async fn put_params(&self, pool: &PgPool, month: &str, body: Value) -> Response {
        let path = format!("/api/v1/settlements/{month}");
        let etag = self.send(pool, "GET", &path, None).await.etag();
        call(pool, "PUT", &path)
            .token(&self.t)
            .if_match(&etag)
            .json(body)
            .send()
            .await
    }

    pub async fn post_month(&self, pool: &PgPool, month: &str, action: &str) -> Response {
        let path = format!("/api/v1/settlements/{month}");
        let etag = self.send(pool, "GET", &path, None).await.etag();
        call(pool, "POST", format!("{path}/{action}"))
            .token(&self.t)
            .if_match(&etag)
            .send()
            .await
    }
}

pub fn edge<'a>(statement: &'a Value, child: &str) -> &'a Value {
    statement["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["child_id"] == child)
        .unwrap()
}
