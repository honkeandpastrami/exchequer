//! Users, invites, passkey sign-in and the CLI's device sign-in, end to end
//! (design §10). Needs the dev database: `just db-up`.

mod common;

use axum::http::StatusCode;
use common::authenticator::Authenticator;
use common::{Response, call, new_token, script_token};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Invites a user as `exchequer-server invite` does (adding them if need
/// be); returns their ID and the invite's code.
async fn invite(pool: &PgPool, name: &str) -> (String, String) {
    let invited = invited(pool, name).await;
    (invited.user.id.to_string(), invited.invite.code)
}

async fn invited(pool: &PgPool, name: &str) -> api::Invited {
    let mut tx = pool.begin().await.unwrap();
    let invited = api::invite(&mut tx, None, name).await.unwrap();
    tx.commit().await.unwrap();
    invited
}

/// Adds a user; returns their ID and invite code.
async fn add_user(pool: &PgPool, name: &str) -> (String, String) {
    let invited = invited(pool, name).await;
    assert!(invited.added);
    assert!(invited.user.passkeys.is_empty());
    (invited.user.id.to_string(), invited.invite.code)
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
    assert_eq!(r.body["session"]["kind"], "browser");
    assert_eq!(r.body["session"]["signed_in_with"], "Test key");
    assert!(
        r.body["session"]["expires_at"].is_string(),
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
    let (_, invite) = add_user(&pool, "Alex").await;
    let session = token(&register(&pool, &mut Authenticator::new(1), &invite).await);

    let r = call(&pool, "POST", "/api/v1/invites")
        .token(&session)
        .json(json!({ "name": "Alex" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let mut phone = Authenticator::new(2);
    let invite = r.body["invite"]["code"].as_str().unwrap();
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
    let (_, script) = script_token(&pool, "script").await;
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

    // Their name is free again: inviting it adds someone new.
    let r = call(&pool, "POST", "/api/v1/invites")
        .token(&sams)
        .json(json!({ "name": "Alex" }))
        .send()
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["added"], true);
    assert_ne!(r.body["user"]["id"], alex.as_str());
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn a_user_is_renamed(pool: PgPool) {
    let (alex, _) = add_user(&pool, "Alex").await;
    add_user(&pool, "Sam").await;
    let (_, session) = new_token(&pool).await;
    let path = format!("/api/v1/users/{alex}");
    let r = call(&pool, "GET", &path).token(&session).send().await;
    assert_eq!(r.status, StatusCode::OK);
    let tag = r.etag();
    let rename = |name: &str, tag: Option<&str>| {
        let mut c = call(&pool, "PATCH", &path)
            .token(&session)
            .json(json!({ "name": name }));
        if let Some(t) = tag {
            c = c.header("If-Match", t);
        }
        c.send()
    };
    assert_eq!(
        rename("Alexandra", None).await.status,
        StatusCode::PRECONDITION_REQUIRED
    );
    assert_eq!(
        rename("Sam", Some(&tag)).await.status,
        StatusCode::CONFLICT,
        "unique"
    );
    let r = rename(" Alexandra ", Some(&tag)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["name"], "Alexandra");
    assert_eq!(
        rename("Al", Some(&tag)).await.status,
        StatusCode::PRECONDITION_FAILED,
        "stale"
    );
    // The old name is free.
    assert!(invited(&pool, "Alex").await.added);
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
        .json(json!({ "client": "cli on laptop", "ttl_days": 90 }))
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
    assert_eq!(
        r.body["ttl_days"], 90,
        "the approver sees how long it lasts"
    );

    // Only a person can approve: not a script.
    let (_, script) = script_token(&pool, "script").await;
    for t in [script.as_str()] {
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
    assert_eq!(r.body["session"]["kind"], "cli");
    assert_eq!(
        r.body["session"]["signed_in_with"], "Test key",
        "the passkey its approver signed in with"
    );
    let expires = time::OffsetDateTime::parse(
        r.body["session"]["expires_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    let left = expires - time::OffsetDateTime::now_utc();
    assert!(left > time::Duration::days(89), "the TTL the CLI asked for");
    // Handed out once.
    assert_eq!(poll().await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn one_invite_adds_or_finds_the_user(pool: PgPool) {
    let (_, session) = new_token(&pool).await;
    let post = |body: Value| {
        call(&pool, "POST", "/api/v1/invites")
            .token(&session)
            .json(body)
            .send()
    };
    let r = post(json!({ "name": "Sam" })).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["added"], true);
    let sam = r.body["user"]["id"].clone();
    let left = time::OffsetDateTime::parse(
        r.body["invite"]["expires_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
        - time::OffsetDateTime::now_utc();
    assert!(
        left <= time::Duration::hours(3)
            && left > time::Duration::hours(3) - time::Duration::minutes(1)
    );

    // The same name again: the same user, another link.
    let r = post(json!({ "name": " Sam " })).await;
    assert_eq!(r.body["added"], false);
    assert_eq!(r.body["user"]["id"], sam);

    // A removed user's name is free again.
    let r = call(
        &pool,
        "DELETE",
        format!("/api/v1/users/{}", sam.as_str().unwrap()),
    )
    .token(&session)
    .send()
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = post(json!({ "name": "Sam" })).await;
    assert_eq!(r.body["added"], true);
    assert_ne!(r.body["user"]["id"], sam);
    assert_eq!(
        post(json!({ "name": "" })).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn an_invite_only_adds_a_passkey(pool: PgPool) {
    let (_, first) = add_user(&pool, "Alex").await;
    let mut lost = Authenticator::new(1);
    let old = token(&register(&pool, &mut lost, &first).await);

    // A lost passkey: a new invite adds another, and leaves the rest alone.
    let (_, code) = invite(&pool, "Alex").await;
    let mut found = Authenticator::new(2);
    let r = register(&pool, &mut found, &code).await;
    let new = token(&r);
    let passkeys = r.body["user"]["passkeys"].as_array().unwrap().clone();
    assert_eq!(passkeys.len(), 2);
    assert_eq!(me(&pool, &old).await.status, StatusCode::OK);
    token(&sign_in(&pool, &mut lost).await);

    // Signed in with the new one, they delete the old one and sign out what
    // it signed in.
    let r = call(
        &pool,
        "DELETE",
        format!("/api/v1/passkeys/{}", passkeys[0]["id"].as_str().unwrap()),
    )
    .token(&new)
    .send()
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let sessions = call(&pool, "GET", "/api/v1/sessions")
        .token(&new)
        .send()
        .await;
    let olds: Vec<_> = sessions
        .body
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["signed_in_with"].is_null())
        .collect();
    assert_eq!(olds.len(), 2, "the passkey's gone: {}", sessions.body);
    for s in olds {
        let r = call(
            &pool,
            "DELETE",
            format!("/api/v1/sessions/{}", s["id"].as_str().unwrap()),
        )
        .token(&new)
        .send()
        .await;
        assert_eq!(r.status, StatusCode::NO_CONTENT);
    }
    assert_eq!(me(&pool, &old).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(me(&pool, &new).await.status, StatusCode::OK);
    assert_eq!(
        sign_in(&pool, &mut lost).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn there_is_no_bootstrap_token(pool: PgPool) {
    let r = call(&pool, "GET", "/api/v1/users")
        .token("bootstrap-token-for-tests-0123456789abcdef")
        .send()
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
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
