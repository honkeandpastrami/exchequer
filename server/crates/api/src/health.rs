//! Liveness and version endpoints (design §14). Neither reveals configuration.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(healthz))
        .routes(routes!(version))
}

#[derive(Serialize, ToSchema)]
pub struct Health {
    /// `ok`, or `database unavailable`.
    status: &'static str,
}

/// Whether the server is up and can reach its database.
#[utoipa::path(
    get,
    path = "/healthz",
    tag = "operations",
    responses(
        (status = OK, body = Health),
        (status = SERVICE_UNAVAILABLE, body = Health),
    ),
)]
async fn healthz(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => (StatusCode::OK, Json(Health { status: "ok" })),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Health {
                status: "database unavailable",
            }),
        ),
    }
}

#[derive(Serialize, ToSchema)]
pub struct Version {
    /// Server version.
    version: &'static str,
    /// Newest schema migration built into this server. Backups record it so
    /// a restore can check compatibility (design §14.2).
    schema: i64,
}

/// Server and schema versions.
#[utoipa::path(
    get,
    path = "/version",
    tag = "operations",
    responses((status = OK, body = Version)),
)]
async fn version() -> Json<Version> {
    Json(Version {
        version: env!("CARGO_PKG_VERSION"),
        schema: store::schema_version(),
    })
}
