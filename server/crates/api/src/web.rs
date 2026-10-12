//! The web app's static build, served at `/` next to the API (design §13).
//! The app uses hash routing and relative paths, so every page is
//! `index.html` and nothing here knows the proxy prefix.

use std::path::Path;

use axum::Router;
use axum::extract::Request;
use axum::handler::HandlerWithoutStateExt;
use axum::http::HeaderValue;
use axum::http::header::{CACHE_CONTROL, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS};
use axum::middleware::Next;
use axum::response::Response;
use tower_http::services::ServeDir;

/// Build output whose names carry a content hash: cacheable forever.
const IMMUTABLE: &str = "/_app/immutable/";

pub fn service(dir: &Path) -> Router {
    let files = ServeDir::new(dir).not_found_service(super::not_found.into_service());
    Router::new()
        .fallback_service(files)
        .layer(axum::middleware::from_fn(headers))
}

async fn headers(req: Request, next: Next) -> Response {
    let immutable = req.uri().path().starts_with(IMMUTABLE);
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        CACHE_CONTROL,
        HeaderValue::from_static(if immutable {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    h.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    res
}
