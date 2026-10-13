//! Remote AI in the stack (design §8, docs/ai.md), with a stand-in for the
//! decision model: what it's asked on upload, what its answers become, and
//! the backtest with it. Needs `just db-up`.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use common::household::H;
use common::{Response, call};
use outbound::decision::{Answer, Decide, Decision, Error, Question};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Picks a fixed option per question when offered, at 0.95, and keeps what
/// it was asked.
#[derive(Default)]
struct Fake {
    asked: Mutex<Vec<(Value, Vec<Question>)>>,
}

impl Decide for Fake {
    fn deployment(&self) -> &str {
        "test-deployment"
    }

    fn decide(&self, state: &Value, questions: &[Question]) -> Result<Decision, Error> {
        self.asked
            .lock()
            .unwrap()
            .push((state.clone(), questions.to_vec()));
        let mut answers = BTreeMap::new();
        for q in questions {
            let want = match q.name.as_str() {
                "counterparty" => "Store",
                "category" => "(no category)",
                "purpose" => "Household",
                _ => "Alex · Alex Credit",
            };
            if q.options.iter().any(|o| o.0 == want) {
                answers.insert(
                    q.name.clone(),
                    Answer {
                        choice: want.into(),
                        probabilities: [(want.to_owned(), 950)].into(),
                    },
                );
            }
        }
        Ok(Decision {
            model: "decision-model".into(),
            answers,
        })
    }
}

fn with_remote(fake: &Arc<Fake>) -> api::Suggestions {
    api::Suggestions::default().with_remote(Some(Arc::new(remote::Remote::new(fake.clone(), 2))))
}

async fn spend(pool: &PgPool, h: &H, date: &str, description: &str) {
    let r = h
        .send(
            pool,
            "POST",
            "/api/v1/transactions",
            Some(
                json!({ "date": date, "description": description, "splits": [{
                "from": h.leg(h.acct(&h.alex_credit), "4.50"),
                "to": h.leg(("counterparty_id", &h.store), "4.50"),
            }] }),
            ),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn remote_ai_answers_what_local_ai_leaves_unsure(pool: PgPool) {
    let h = H::new(&pool).await;
    spend(&pool, &h, "2026-02-01", "CORNER CAFE 1111").await;
    spend(&pool, &h, "2026-02-08", "CORNER CAFE 2222").await;
    let fake = Arc::new(Fake::default());

    // A card number in the bank's text: never sent.
    let csv = "Date,Description,Amount\n03/01/2026,CORNER CAFE 4111111111111111,-4.50\n";
    let r = call(&pool, "POST", "/api/v1/imports?file_name=card.csv")
        .token(&h.t)
        .bytes(csv.as_bytes().to_vec())
        .suggestions(with_remote(&fake))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let s = &r.body["statements"][0];
    let by = &s["suggested_account"]["suggester"];
    assert_eq!(
        (
            &s["account_id"],
            &by["kind"],
            &by["name"],
            &by["version"],
            &s["suggested_account"]["reason"]
        ),
        (
            &json!(h.alex_credit),
            &json!("remote"),
            &json!("decision-model"),
            &json!("q1"),
            &json!("decision-model: 0.95")
        ),
        "{s}"
    );

    let rows: Response = h
        .send(
            &pool,
            "GET",
            &format!("/api/v1/imports/{}/rows", r.id()),
            None,
        )
        .await;
    let row = &rows.body["items"][0];
    assert_eq!(
        (
            &row["counterparty_id"],
            &row["category_id"],
            &row["purpose_id"],
            &row["missing"]
        ),
        (
            &json!(h.store),
            &Value::Null,
            &json!(h.household),
            &json!([])
        ),
        "{row}"
    );
    let kinds: Vec<&Value> = row["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["suggester"]["kind"])
        .collect();
    assert_eq!(kinds, [&json!("remote"); 3]);

    let asked = fake.asked.lock().unwrap();
    // The account, then the row's three labels.
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0].1[0].name, "account");
    let (state, questions) = &asked[1];
    assert_eq!(state["account"]["name"], "Alex · Alex Credit");
    assert_eq!(
        state["similar_past_transactions"][0]["counterparty"],
        "Store"
    );
    assert_eq!(questions.len(), 3);
    let sent = format!("{asked:?}");
    assert!(!sent.contains("41111"), "{sent}");
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_backtest_can_ask_remote_ai_too(pool: PgPool) {
    let h = H::new(&pool).await;
    spend(&pool, &h, "2026-02-01", "CORNER CAFE").await;
    spend(&pool, &h, "2026-03-01", "CORNER CAFE").await;
    let fake = Arc::new(Fake::default());
    let backtest = |query: &str, s: api::Suggestions| {
        let path = format!("/api/v1/suggesters/backtest?{query}");
        let (pool, t) = (pool.clone(), h.t.clone());
        async move {
            call(&pool, "GET", path)
                .token(&t)
                .suggestions(s)
                .send()
                .await
        }
    };

    let r = backtest("from=2026-03&remote=true", with_remote(&fake)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let m = &r.body["months"][0];
    let levels: Vec<(&Value, &Value)> = m["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (&l["kind"], &l["name"]))
        .collect();
    assert_eq!(
        levels,
        [
            (&json!("local"), &json!("lexical-knn")),
            (&json!("remote"), &json!("decision-model"))
        ]
    );
    // Remote AI made the final counterparty suggestion, and it was right.
    let cp = |scores: &Value| -> (Value, Value) {
        let s = scores
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["label"] == "counterparty")
            .unwrap();
        (s["guessed"].clone(), s["right"].clone())
    };
    assert_eq!(cp(&m["levels"][1]["labels"]), (json!(1), json!(1)));
    assert_eq!(cp(&m["labels"]), (json!(1), json!(1)));
    assert!(!fake.asked.lock().unwrap().is_empty());

    // Without remote AI configured, or over too many months: refused.
    let r = backtest("from=2026-03&remote=true", api::Suggestions::default()).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = backtest("from=2026-01&to=2026-06&remote=true", with_remote(&fake)).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    // Local only, as before.
    let r = backtest("from=2026-03", api::Suggestions::default()).await;
    assert_eq!(r.body["months"][0]["levels"].as_array().unwrap().len(), 1);
}
