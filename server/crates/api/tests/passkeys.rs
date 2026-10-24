//! Users, invites, passkey sign-in and the CLI's device sign-in, end to end
//! (design §10). Needs the dev database: `just db-up`.

mod common;

use axum::http::StatusCode;
use common::authenticator::Authenticator;
use common::{BOOTSTRAP, Response, call, new_token};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Adds a user with the bootstrap token; returns their ID and invite code.
async fn add_user(pool: &PgPool, name: &str) -> (String, String) {
    let r = call(pool, "POST", "/api/v1/users")
        .token(BOOTSTRAP)
        .json(json!({ "name": name }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["user"]["passkeys"], json!([]));
    (
        r.body["user"]["id"].as_str().unwrap().to_owned(),
        r.body["invite"]["code"].as_str().unwrap().to_owned(),
    )
}

/// Registers the authenticator's passkey with the invite.
async fn register(pool: &PgPool, key: &mut Authenticator, invite: &str) -> Response {
    let start = call(pool, "POST", "/api/v1/auth/register")
        .json(json!({ "invite": invite }))
        .send()
        .await;
    if start.status != StatusCode::OK {
        return start;
    }
    let credential = key.create(&start.body["options"]);
    call(
        pool,
        "POST",
        format!("/api/v1/auth/register/{}", start.id()),
    )
    .json(json!({ "credential": credential, "name": "Test key" }))
    .send()
    .await
}

/// Signs in with the authenticator's passkey.
async fn sign_in(pool: &PgPool, key: &mut Authenticator) -> Response {
    let start = call(pool, "POST", "/api/v1/auth/sign-in").send().await;
    assert_eq!(start.status, StatusCode::OK, "{}", start.body);
    let credential = key.get(&start.body["options"]);
    call(pool, "POST", format!("/api/v1/auth/sign-in/{}", start.id()))
        .json(json!({ "credential": credential }))
        .send()
        .await
}

async fn me(pool: &PgPool, token: &str) -> Response {
    call(pool, "GET", "/api/v1/me").token(token).send().await
}

fn token(r: &Response) -> String {
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    r.body["token"].as_str().unwrap().to_owned()
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn an_invite_registers_a_passkey_that_signs_in(pool: PgPool) {
    let (alex, invite) = add_user(&pool, "Alex").await;
    let mut key = Authenticator::new(1);
    let r = register(&pool, &mut key, &invite).await;
    let session = token(&r);
    assert_eq!(r.body["user"]["name"], "Alex");
    assert_eq!(r.body["user"]["passkeys"][0]["name"], "Test key");

    let r = me(&pool, &session).await;
    assert_eq!(r.body["user"]["id"], alex.as_str());
    assert!(
        r.body["token"]["expires_at"].is_string(),
        "a session expires"
    );

    // Signing in again, on the same passkey: a new session.
    let r = sign_in(&pool, &mut key).await;
    let again = token(&r);
    assert_ne!(again, session);
    assert_eq!(me(&pool, &again).await.body["user"]["name"], "Alex");
    let r = call(&pool, "GET", "/api/v1/users")
        .token(&again)
        .send()
        .await;
    assert!(r.body[0]["passkeys"][0]["last_used_at"].is_string());

    // The invite worked once.
    let r = register(&pool, &mut Authenticator::new(2), &invite).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.body);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_ceremony_is_used_once(pool: PgPool) {
    let (_, invite) = add_user(&pool, "Alex").await;
    let mut key = Authenticator::new(1);
    token(&register(&pool, &mut key, &invite).await);

    let start = call(&pool, "POST", "/api/v1/auth/sign-in").send().await;
    let credential = key.get(&start.body["options"]);
    let finish = |c: Value| {
        call(
            &pool,
            "POST",
            format!("/api/v1/auth/sign-in/{}", start.id()),
        )
        .json(json!({ "credential": c }))
        .send()
    };
    token(&finish(credential.clone()).await);
    assert_eq!(finish(credential).await.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn passkeys_from_elsewhere_are_refused(pool: PgPool) {
    let (_, invite) = add_user(&pool, "Alex").await;
    let mut key = Authenticator::new(1);
    token(&register(&pool, &mut key, &invite).await);

    // Another site's page asking this passkey: the origin gives it away.
    key.origin = "https://books.example.net".into();
    assert_eq!(
        sign_in(&pool, &mut key).await.status,
        StatusCode::UNAUTHORIZED
    );

    // A passkey nobody registered.
    let r = sign_in(&pool, &mut Authenticator::new(9)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    // An invite from elsewhere, or a mangled one.
    let (_, other) = add_user(&pool, "Sam").await;
    let forged = format!("{}{}", &other[..other.len() - 4], "0000");
    for bad in [forged.as_str(), "exi_nope", ""] {
        let r = register(&pool, &mut Authenticator::new(3), bad).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{bad}: {}", r.body);
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_new_invite_adds_a_passkey(pool: PgPool) {
    let (alex, invite) = add_user(&pool, "Alex").await;
    let session = token(&register(&pool, &mut Authenticator::new(1), &invite).await);

    let r = call(&pool, "POST", format!("/api/v1/users/{alex}/invites"))
        .token(&session)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let mut phone = Authenticator::new(2);
    let invite = r.body["code"].as_str().unwrap();
    assert_eq!(
        register(&pool, &mut phone, invite).await.body["user"]["passkeys"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    token(&sign_in(&pool, &mut phone).await);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn signing_out_ends_the_session_only(pool: PgPool) {
    let (_, invite) = add_user(&pool, "Alex").await;
    let session = token(&register(&pool, &mut Authenticator::new(1), &invite).await);
    let r = call(&pool, "DELETE", "/api/v1/me/session")
        .token(&session)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(me(&pool, &session).await.status, StatusCode::UNAUTHORIZED);

    // A script's token isn't a session: signing out leaves it working.
    let (_, script) = new_token(&pool, "script").await;
    let r = call(&pool, "DELETE", "/api/v1/me/session")
        .token(&script)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(me(&pool, &script).await.status, StatusCode::OK);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn expired_sessions_are_refused(pool: PgPool) {
    let (_, invite) = add_user(&pool, "Alex").await;
    let session = token(&register(&pool, &mut Authenticator::new(1), &invite).await);
    sqlx::query("UPDATE api_token SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(me(&pool, &session).await.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn removing_a_user_ends_their_access(pool: PgPool) {
    let (alex, invite) = add_user(&pool, "Alex").await;
    let mut key = Authenticator::new(1);
    let session = token(&register(&pool, &mut key, &invite).await);
    let (sam, sam_invite) = add_user(&pool, "Sam").await;
    let sams = token(&register(&pool, &mut Authenticator::new(2), &sam_invite).await);

    let r = call(&pool, "DELETE", format!("/api/v1/users/{alex}"))
        .token(&sams)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(me(&pool, &session).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        sign_in(&pool, &mut key).await.status,
        StatusCode::UNAUTHORIZED
    );
    let r = call(&pool, "GET", "/api/v1/users")
        .token(&sams)
        .send()
        .await;
    assert_eq!(r.body.as_array().unwrap().len(), 1);
    assert_eq!(r.body[0]["id"], sam.as_str());

    // Their name stays taken: the audit log still refers to them.
    let r = call(&pool, "POST", "/api/v1/users")
        .token(&sams)
        .json(json!({ "name": "Alex" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_deleted_passkey_stops_working(pool: PgPool) {
    let (_, invite) = add_user(&pool, "Alex").await;
    let mut key = Authenticator::new(1);
    let r = register(&pool, &mut key, &invite).await;
    let session = token(&r);
    let passkey = r.body["user"]["passkeys"][0]["id"].as_str().unwrap();
    let r = call(&pool, "DELETE", format!("/api/v1/passkeys/{passkey}"))
        .token(&session)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        sign_in(&pool, &mut key).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn the_cli_signs_in_when_a_user_approves(pool: PgPool) {
    let (alex, invite) = add_user(&pool, "Alex").await;
    let session = token(&register(&pool, &mut Authenticator::new(1), &invite).await);

    let r = call(&pool, "POST", "/api/v1/auth/device")
        .json(json!({ "client": "cli on laptop" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let device_code = r.body["device_code"].as_str().unwrap().to_owned();
    let user_code = r.body["user_code"].as_str().unwrap().to_owned();
    let poll = || {
        call(&pool, "POST", "/api/v1/auth/device/token")
            .json(json!({ "device_code": device_code }))
            .send()
    };
    assert_eq!(poll().await.body["token"], Value::Null, "waiting");

    // Typed loosely by the approver.
    let typed = user_code.to_lowercase().replace('-', "");
    let r = call(&pool, "GET", format!("/api/v1/devices/{typed}"))
        .token(&session)
        .send()
        .await;
    assert_eq!(r.body["client"], "cli on laptop");

    // Only a user can approve: not a script, not the bootstrap token.
    let (_, script) = new_token(&pool, "script").await;
    for t in [script.as_str(), BOOTSTRAP] {
        let r = call(
            &pool,
            "POST",
            format!("/api/v1/devices/{user_code}/approve"),
        )
        .token(t)
        .send()
        .await;
        assert!(r.status == StatusCode::FORBIDDEN, "{}", r.body);
    }
    let r = call(
        &pool,
        "POST",
        format!("/api/v1/devices/{user_code}/approve"),
    )
    .token(&session)
    .send()
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.body);

    let cli = token(&poll().await);
    let r = me(&pool, &cli).await;
    assert_eq!(r.body["user"]["id"], alex.as_str());
    assert_eq!(
        r.body["token"]["expires_at"],
        Value::Null,
        "the CLI's token lasts"
    );
    // Handed out once.
    assert_eq!(poll().await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn the_bootstrap_token_manages_users_and_nothing_else(pool: PgPool) {
    add_user(&pool, "Alex").await;
    let r = call(&pool, "GET", "/api/v1/users")
        .token(BOOTSTRAP)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = call(&pool, "GET", "/api/v1/parties")
        .token(BOOTSTRAP)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = call(&pool, "GET", "/api/v1/usersx")
        .token(BOOTSTRAP)
        .send()
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn without_an_origin_only_tokens_work(pool: PgPool) {
    let r = call(&pool, "GET", "/api/v1/auth")
        .passkeys_off()
        .send()
        .await;
    assert_eq!(r.body, json!({ "passkeys": false }));
    let r = call(&pool, "POST", "/api/v1/auth/sign-in")
        .passkeys_off()
        .send()
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    let r = call(&pool, "GET", "/api/v1/auth").send().await;
    assert_eq!(r.body, json!({ "passkeys": true }));
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn the_audit_log_keeps_no_secrets(pool: PgPool) {
    let (_, invite) = add_user(&pool, "Alex").await;
    let session = token(&register(&pool, &mut Authenticator::new(1), &invite).await);
    let rows: Vec<(String, String, Option<Value>)> =
        sqlx::query_as("SELECT entity, action, after FROM audit_log ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let entities: Vec<_> = rows.iter().map(|(e, a, _)| format!("{e} {a}")).collect();
    assert_eq!(
        entities,
        [
            "user create",
            "invite create",
            "token create",
            "passkey create"
        ]
    );
    let all = format!("{rows:?}");
    assert!(!all.contains(&invite[37..]), "the invite's secret");
    assert!(!all.contains(&session[37..]), "the session's secret");
}
