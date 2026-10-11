//! The HTTP API (design §5). Every route is registered together with its
//! OpenAPI entry, so the spec can't drift from the handlers.

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::Response;
use sqlx::PgPool;
use tower_http::trace::TraceLayer;
use tracing::Span;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

mod health;

/// Shared by every handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

#[derive(OpenApi)]
#[openapi(info(
    title = "Exchequer",
    description = "Self-hosted household accounting. Amounts are decimal strings, never JSON numbers.",
    license(name = "BSD-2-Clause", identifier = "BSD-2-Clause"),
))]
struct ApiDoc;

fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi()).merge(health::routes())
}

/// The OpenAPI document, without needing a database.
#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    routes().split_for_parts().1
}

/// The complete application.
pub fn app(state: AppState) -> Router {
    let (router, _) = routes().split_for_parts();
    // Logs method, path, status and latency. Never bodies, headers or query
    // strings: they can carry household data or tokens (design §3.6).
    let request_log = TraceLayer::new_for_http()
        .make_span_with(|req: &Request| {
            tracing::info_span!("request", method = %req.method(), path = %req.uri().path())
        })
        .on_request(())
        .on_response(|res: &Response<Body>, latency: Duration, _: &Span| {
            tracing::info!(status = res.status().as_u16(), ms = latency.as_millis(), "response");
        });
    router.layer(request_log).with_state(state)
}
