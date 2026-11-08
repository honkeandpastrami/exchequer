//! Migrations over data, not just an empty database: production has rows
//! the newest migration must carry along. Needs the dev database:
//! `just db-up`.

use sqlx::PgPool;

/// Applies the migrations up to `version`, as the server would have.
async fn migrate_to(pool: &PgPool, version: i64) {
    for m in store::MIGRATOR.iter().filter(|m| m.version <= version) {
        sqlx::raw_sql(m.sql.clone())
            .execute(pool)
            .await
            .unwrap_or_else(|e| {
                panic!("{} {}: {e}", m.version, m.description);
            });
    }
}

async fn migrate_from(pool: &PgPool, version: i64) {
    for m in store::MIGRATOR.iter().filter(|m| m.version > version) {
        sqlx::raw_sql(m.sql.clone())
            .execute(pool)
            .await
            .unwrap_or_else(|e| {
                panic!("{} {}: {e}", m.version, m.description);
            });
    }
}

/// Every kind of token there was before sessions (0026) keeps working, as
/// what it is now.
#[sqlx::test(migrations = false)]
async fn tokens_become_sessions(pool: PgPool) {
    migrate_to(&pool, 25).await;
    sqlx::raw_sql(
        "INSERT INTO app_user (id, name) VALUES ('00000000-0000-0000-0000-00000000000a', 'Alex');
         INSERT INTO api_token (id, name, kind, secret_hash, user_id, expires_at, revoked_at)
         VALUES
           (gen_random_uuid(), 'Alex · web · a1b2c3', 'session', 'x',
            '00000000-0000-0000-0000-00000000000a', now() + interval '1 day', NULL),
           (gen_random_uuid(), 'Alex · cli on laptop · d4e5f6', 'cli', 'x',
            '00000000-0000-0000-0000-00000000000a', now() + interval '1 day', NULL),
           (gen_random_uuid(), 'backup', 'script', 'x',
            '00000000-0000-0000-0000-00000000000a', now() + interval '1 day', NULL),
           (gen_random_uuid(), 'Alex · web · 0f0f0f', 'session', 'x',
            '00000000-0000-0000-0000-00000000000a', now() + interval '1 day', now());
         INSERT INTO invite (id, user_id, secret_hash, expires_at, replace)
         VALUES (gen_random_uuid(), '00000000-0000-0000-0000-00000000000a', 'x',
                 now() + interval '1 hour', true);",
    )
    .execute(&pool)
    .await
    .unwrap();

    migrate_from(&pool, 25).await;

    let tokens: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT kind, name FROM api_token ORDER BY kind, name")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        tokens,
        [
            ("browser".into(), None),
            ("browser".into(), None),
            ("cli".into(), None),
            ("script".into(), Some("backup".into())),
        ]
    );
    let sessions = store::tokens::sessions(&pool).await.unwrap();
    assert_eq!(sessions.len(), 2, "the browser and the CLI signed in now");
    assert!(sessions.iter().all(|s| s.agent.is_none()));
}
