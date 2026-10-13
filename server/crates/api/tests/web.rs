//! The web app's static build at `/` (design §13): files, caching headers,
//! and the API still behind its token. Needs `just db-up`.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

struct Got {
    status: StatusCode,
    cache: Option<String>,
    body: String,
}

async fn get(app: &axum::Router, path: &str) -> Got {
    let res = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let cache = res
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = res.into_body().collect().await.unwrap().to_bytes();
    Got {
        status,
        cache,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn serves_the_build(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("exchequer-web-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("_app/immutable")).unwrap();
    std::fs::write(dir.join("index.html"), "<p>app</p>").unwrap();
    std::fs::write(dir.join("_app/immutable/start.js"), "// js").unwrap();
    let app = api::app_with_web(
        api::AppState {
            pool,
            bootstrap_token: None,
            zone: jiff::tz::TimeZone::get("America/New_York").unwrap(),
            suggestions: api::Suggestions::default(),
        },
        &dir,
    );

    let index = get(&app, "/").await;
    assert_eq!(index.status, StatusCode::OK);
    assert_eq!(index.body, "<p>app</p>");
    assert_eq!(index.cache.as_deref(), Some("no-cache"));

    let js = get(&app, "/_app/immutable/start.js").await;
    assert_eq!(js.status, StatusCode::OK);
    assert!(js.cache.unwrap().contains("immutable"));

    let missing = get(&app, "/nope.js").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert!(missing.body.contains("not-found"), "{}", missing.body);

    // Outside the build directory: refused.
    assert_eq!(
        get(&app, "/../Cargo.toml").await.status,
        StatusCode::NOT_FOUND
    );
    // The API keeps its routes and its token.
    assert_eq!(get(&app, "/healthz").await.status, StatusCode::OK);
    assert_eq!(
        get(&app, "/api/v1/accounts").await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&app, "/api/v1/index.html").await.status,
        StatusCode::UNAUTHORIZED
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
