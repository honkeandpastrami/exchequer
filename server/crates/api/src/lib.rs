//! The HTTP API (design §5). Every route is registered together with its
//! OpenAPI entry, so the spec can't drift from the handlers.

use std::path::Path;
use std::sync::Arc;
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

mod accounts;
pub mod auth;
mod balance_checks;
mod categories;
mod commodities;
mod common;
pub mod conventions;
mod counterparties;
mod health;
mod import_batches;
mod imports;
mod ledger;
mod parties;
pub mod problem;
mod query;
mod reports;
mod settlements;
mod tags;
mod tokens;
mod transactions;
mod web;

/// Shared by every handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Accepted only for managing tokens (design §10).
    pub bootstrap_token: Option<Arc<str>>,
    /// The server's time zone: what "today" and a bank file's dates mean.
    pub zone: jiff::tz::TimeZone,
}

impl AppState {
    /// Today's date in the server's time zone.
    #[must_use]
    pub fn today(&self) -> time::Date {
        let d = jiff::Zoned::now().with_time_zone(self.zone.clone()).date();
        let month = u8::try_from(d.month())
            .ok()
            .and_then(|m| time::Month::try_from(m).ok())
            .expect("jiff months are 1-12");
        let day = u8::try_from(d.day()).expect("jiff days are 1-31");
        time::Date::from_calendar_date(i32::from(d.year()), month, day)
            .expect("a jiff date is a valid calendar date")
    }
}

#[derive(OpenApi)]
#[openapi(modifiers(&BearerAuth), info(
    title = "Exchequer",
    description = "Self-hosted household accounting. Amounts are decimal strings, never JSON numbers.",
    license(name = "BSD-2-Clause", identifier = "BSD-2-Clause"),
))]
struct ApiDoc;

struct BearerAuth;

impl utoipa::Modify for BearerAuth {
    fn modify(&self, doc: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
        doc.components.get_or_insert_default().add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some("`exq_<id>_<secret>`, from POST /api/v1/tokens"))
                    .build(),
            ),
        );
    }
}

/// Everything under `/api/v1`; all of it requires a token.
fn v1() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(tokens::routes())
        .merge(parties::routes())
        .merge(commodities::routes())
        .merge(accounts::routes())
        .merge(counterparties::routes())
        .merge(categories::routes())
        .merge(tags::routes())
        .merge(transactions::routes())
        .merge(import_batches::routes())
        .merge(imports::routes())
        .merge(settlements::routes())
        .merge(query::routes())
        .merge(reports::routes())
        .merge(balance_checks::routes())
}

fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(health::routes())
        .nest("/api/v1", v1())
}

/// The OpenAPI document, without needing a database.
#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = routes().split_for_parts().1;
    describe_responses(&mut doc);
    qualify_operation_ids(&mut doc);
    doc
}

/// Handlers share names across resources (`list`, `get`…), but operation IDs
/// must be unique: prefix each with its tag (`parties_list`).
fn qualify_operation_ids(doc: &mut utoipa::openapi::OpenApi) {
    for item in doc.paths.paths.values_mut() {
        let operations = [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ];
        for op in operations.into_iter().flatten() {
            if let (Some(tag), Some(id)) =
                (op.tags.as_ref().and_then(|t| t.first()), &op.operation_id)
            {
                op.operation_id = Some(format!("{}_{id}", tag.replace('-', "_")));
            }
        }
    }
}

/// OpenAPI requires a description on every response; where a handler gives
/// none, use the status's reason phrase ("Not Found").
fn describe_responses(doc: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::RefOr;
    for item in doc.paths.paths.values_mut() {
        let operations = [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ];
        for op in operations.into_iter().flatten() {
            for (status, response) in &mut op.responses.responses {
                if let RefOr::T(r) = response
                    && r.description.is_empty()
                {
                    r.description = status
                        .parse::<u16>()
                        .ok()
                        .and_then(|c| axum::http::StatusCode::from_u16(c).ok())
                        .and_then(|c| c.canonical_reason())
                        .unwrap_or("Response")
                        .to_owned();
                }
            }
        }
    }
}

async fn not_found() -> problem::Problem {
    problem::Problem::not_found()
}

/// The complete application, without the web app.
pub fn app(state: AppState) -> Router {
    build(state, None)
}

/// The complete application, also serving the web app's static build from
/// `web_dir` at `/`. Unknown paths outside `/api/v1` get its 404.
pub fn app_with_web(state: AppState, web_dir: &Path) -> Router {
    build(state, Some(web_dir))
}

fn build(state: AppState, web_dir: Option<&Path>) -> Router {
    let (operations, _) = health::routes().split_for_parts();
    let (v1, _) = v1().split_for_parts();
    // The fallback sits inside the auth layer: unknown /api/v1 paths need a
    // token too, so they don't reveal which routes exist.
    // Layers run outside-in in reverse order: auth first, then idempotency
    // (which needs to know the token).
    let v1 = v1
        .fallback(not_found)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            conventions::idempotency,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require,
        ));
    let router = operations.nest("/api/v1", v1);
    let router = match web_dir {
        Some(dir) => router.fallback_service(web::service(dir)),
        None => router.fallback(not_found),
    };
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
