//! Token authentication, `/api/v1/tokens` and `/api/v1/sessions`, end to
//! end.
//! Needs the dev database: `just db-up`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tower::ServiceExt;

use common::{new_token as create_token, script_token};

struct Response {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: Value,
}

async fn call(
    pool: &PgPool,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Response {
    call_from(pool, method, path, token, body, &[]).await
}

/// `call`, from a client that sends these headers.
async fn call_from(
    pool: &PgPool,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> Response {
    let app = api::app(api::AppState {
        pool: pool.clone(),
        passkeys: None,
        zone: jiff::tz::TimeZone::get("America/New_York").unwrap(),
        suggestions: api::Suggestions::default(),
        pricing: api::Pricing::default(),
    });
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    Response {
        status,
        headers,
        body,
    }
}

fn at(v: &Value) -> OffsetDateTime {
    OffsetDateTime::parse(v.as_str().unwrap(), &Rfc3339).unwrap()
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn requests_without_a_valid_token_are_rejected(pool: PgPool) {
    let (_, token) = create_token(&pool).await;
    let wrong_secret = format!(
        "{}{}",
        &token[..token.len() - 1],
        if token.ends_with('0') { '1' } else { '0' }
    );

    for t in [
        None,
        Some("garbage"),
        Some(wrong_secret.as_str()),
        Some(
            "exq_0123456789abcdef0123456789abcdef_0000000000000000000000000000000000000000000000000000000000000000",
        ),
    ] {
        let r = call(&pool, "GET", "/api/v1/tokens", t, None).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{t:?}");
        assert_eq!(r.headers[header::WWW_AUTHENTICATE], "Bearer");
        assert_eq!(r.headers[header::CONTENT_TYPE], "application/problem+json");
        assert_eq!(r.body["type"], "urn:exchequer:problem:unauthorized");
    }
    // Unknown paths need a token too, so they don't reveal which routes exist.
    let r = call(&pool, "GET", "/api/v1/no-such-thing", None, None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let r = call(&pool, "GET", "/api/v1/no-such-thing", Some(&token), None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // Operations endpoints stay open.
    assert_eq!(
        call(&pool, "GET", "/healthz", None, None).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn token_lifecycle(pool: PgPool) {
    let (_, cli) = create_token(&pool).await;

    // A person's token makes a script's, theirs, for as long as asked; names
    // are unique.
    let r = call(
        &pool,
        "POST",
        "/api/v1/tokens",
        Some(&cli),
        Some(json!({ "name": "backup", "ttl_days": 7 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert!(r.body["last_used_at"].is_null());
    let left = at(&r.body["expires_at"]) - OffsetDateTime::now_utc();
    assert!(left > time::Duration::days(7) - time::Duration::minutes(1));
    assert!(left <= time::Duration::days(7));
    let (id, token) = (
        r.body["id"].as_str().unwrap().to_owned(),
        r.body["token"].as_str().unwrap().to_owned(),
    );
    assert!(token.starts_with("exq_"));
    let r = call(
        &pool,
        "POST",
        "/api/v1/tokens",
        Some(&cli),
        Some(json!({ "name": "backup", "ttl_days": 7 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(r.body["type"], "urn:exchequer:problem:conflict");

    // The token works; the list is scripts' tokens only, and never shows
    // secrets.
    let r = call(&pool, "GET", "/api/v1/tokens", Some(&token), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body.as_array().unwrap().len(), 1, "not the CLI's");
    assert_eq!(r.body[0]["name"], "backup");
    assert!(r.body[0]["revoked_at"].is_null());
    assert!(
        r.body[0]["last_used_at"].is_string(),
        "this request used it"
    );
    assert!(!r.body.to_string().contains(&token[37..]));
    assert!(r.body[0].get("secret_hash").is_none());

    // Revoking stops it immediately; revoking twice is harmless. Only a
    // script's token is revoked here; a person's session is signed out.
    let path = format!("/api/v1/tokens/{id}");
    assert_eq!(
        call(&pool, "DELETE", &path, Some(&cli), None).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&pool, "GET", "/api/v1/tokens", Some(&token), None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&pool, "DELETE", &path, Some(&cli), None).await.status,
        StatusCode::NO_CONTENT
    );
    let (cli_id, _) = create_token(&pool).await;
    for missing in [cli_id.as_str(), "00000000-0000-0000-0000-000000000000"] {
        let path = format!("/api/v1/tokens/{missing}");
        let r = call(&pool, "DELETE", &path, Some(&cli), None).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{missing}");
    }

    // The audit log has the create and the revoke, and never a secret.
    let rows: Vec<(String, Option<Value>)> =
        sqlx::query_as("SELECT action, after FROM audit_log ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let actions: Vec<&str> = rows.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(actions, ["create", "update"]);
    let all = format!("{rows:?}");
    assert!(!all.contains(&token[37..]));
    assert!(!all.contains("secret_hash"));
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn expired_tokens_are_refused(pool: PgPool) {
    let (id, token) = create_token(&pool).await;
    sqlx::query("UPDATE api_token SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let r = call(&pool, "GET", "/api/v1/tokens", Some(&token), None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn the_kill_switch_revokes_every_token(pool: PgPool) {
    let (_, mine) = create_token(&pool).await;
    let (_, script) = script_token(&pool, "script").await;
    let (gone, _) = script_token(&pool, "revoked already").await;
    let r = call(
        &pool,
        "DELETE",
        &format!("/api/v1/tokens/{gone}"),
        Some(&mine),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);

    let r = call(&pool, "POST", "/api/v1/revoke-all", Some(&mine), None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body, json!({ "revoked": 2 }));
    for t in [&mine, &script] {
        let r = call(&pool, "GET", "/api/v1/me", Some(t), None).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "this request's too");
    }
    let revokes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE entity = 'token' AND action = 'update'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revokes, 3, "one by one, then two at once");
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn sessions_are_clients_using_tokens(pool: PgPool) {
    let (_, cli) = create_token(&pool).await;
    let (script_id, script) = script_token(&pool, "backup").await;
    let laptop = [
        ("user-agent", "exchequer-cli/1.4.0 python-httpx/0.28.1"),
        ("exchequer-host", "laptop"),
        ("x-forwarded-for", "192.168.1.20"),
    ];
    let me = call_from(&pool, "GET", "/api/v1/me", Some(&cli), None, &laptop).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.body);
    assert_eq!(me.body["session"]["kind"], "cli");
    assert_eq!(me.body["session"]["client"], "exchequer-cli 1.4.0");
    assert_eq!(me.body["session"]["host"], "laptop");
    assert_eq!(me.body["session"]["ip"], "192.168.1.20");
    let cli_session = me.body["session"]["id"].clone();

    // A browser's or the CLI's token is one session, whatever it reports; a
    // script's has one per tool and host.
    let upgraded = [
        ("user-agent", "exchequer-cli/1.5.0"),
        ("exchequer-host", "laptop"),
    ];
    for (agent, host) in [
        ("backup/1.0", "nas"),
        ("backup/1.1", "nas"),
        ("backup/1.1", "desktop"),
        ("prices.py", "nas"),
    ] {
        let headers = [("user-agent", agent), ("exchequer-host", host)];
        let r = call_from(&pool, "GET", "/api/v1/me", Some(&script), None, &headers).await;
        assert_eq!(r.status, StatusCode::OK);
    }
    let r = call_from(
        &pool,
        "GET",
        "/api/v1/sessions",
        Some(&cli),
        None,
        &upgraded,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let sessions = r.body.as_array().unwrap();
    assert_eq!(sessions.len(), 4, "{}", r.body);
    let cli_row = sessions.iter().find(|s| s["id"] == cli_session).unwrap();
    assert_eq!(
        cli_row["client"], "exchequer-cli 1.5.0",
        "what it last used"
    );
    assert!(cli_row["ip"].is_null(), "no proxy, no peer: no address");
    // Sent nothing: nothing is known, and it works all the same.
    let r = call(&pool, "GET", "/api/v1/me", Some(&cli), None).await;
    assert!(r.body["session"]["client"].is_null());
    let scripts: Vec<_> = sessions.iter().filter(|s| s["kind"] == "script").collect();
    assert_eq!(scripts.len(), 3);
    assert!(scripts.iter().all(|s| s["signed_in_with"] == "backup"));

    // Signing a script's session out revokes its token: all three end.
    let path = format!("/api/v1/sessions/{}", scripts[0]["id"].as_str().unwrap());
    let r = call(&pool, "DELETE", &path, Some(&cli), None).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = call(&pool, "GET", "/api/v1/sessions", Some(&cli), None).await;
    assert_eq!(r.body.as_array().unwrap().len(), 1);
    let r = call(&pool, "GET", "/api/v1/tokens", Some(&cli), None).await;
    assert_eq!(r.body[0]["id"], script_id.as_str());
    assert!(r.body[0]["revoked_at"].is_string());
    let r = call(&pool, "DELETE", &path, Some(&cli), None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "it ended");
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_script_cannot_make_tokens(pool: PgPool) {
    let (_, script) = script_token(&pool, "script").await;
    let r = call(
        &pool,
        "POST",
        "/api/v1/tokens",
        Some(&script),
        Some(json!({ "name": "successor", "ttl_days": 30 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.body);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn bad_requests_are_problem_details(pool: PgPool) {
    let (_, token) = create_token(&pool).await;
    for body in [
        json!({}),
        json!({ "name": "no ttl" }),
        json!({ "name": "  ", "ttl_days": 30 }),
        json!({ "name": "x".repeat(65), "ttl_days": 30 }),
        json!({ "name": "forever", "ttl_days": 0 }),
        json!({ "name": "too long", "ttl_days": 366 }),
        json!({ "name": "negative", "ttl_days": -1 }),
    ] {
        let r = call(
            &pool,
            "POST",
            "/api/v1/tokens",
            Some(&token),
            Some(body.clone()),
        )
        .await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(r.headers[header::CONTENT_TYPE], "application/problem+json");
    }
    let r = call(
        &pool,
        "DELETE",
        "/api/v1/tokens/not-a-uuid",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(r.body["type"], "urn:exchequer:problem:invalid-request");
}
