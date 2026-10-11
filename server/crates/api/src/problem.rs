//! Errors as RFC 9457 Problem Details (design §5).

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// An error response (`application/problem+json`).
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    /// `urn:exchequer:problem:<slug>`; stable, for clients to match on.
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Problem {
    pub fn new(status: StatusCode, slug: &str, title: &str) -> Self {
        Self {
            kind: format!("urn:exchequer:problem:{slug}"),
            title: title.to_owned(),
            status: status.as_u16(),
            detail: None,
        }
    }

    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Missing or invalid token",
        )
    }

    pub fn forbidden(detail: &str) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", "Not allowed").detail(detail)
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not-found", "Not found")
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "conflict",
            "Conflicts with existing data",
        )
        .detail(detail)
    }

    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid-request",
            "Invalid request",
        )
        .detail(detail)
    }

    /// A change to a closed month (design §4.4).
    pub fn month_closed(month: domain::Month) -> Self {
        Self::new(StatusCode::CONFLICT, "month-closed", "The month is closed")
            .detail(format!("{month} is closed; reopen it to edit"))
    }

    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Internal server error",
        )
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut res = (status, axum::Json(&self)).into_response();
        let headers = res.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        if status == StatusCode::UNAUTHORIZED {
            headers.insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        res
    }
}

/// Database errors become a 500. Only the error code and constraint are
/// logged: a database message can quote row values.
impl From<sqlx::Error> for Problem {
    fn from(err: sqlx::Error) -> Self {
        match err.as_database_error() {
            Some(db) => tracing::error!(
                code = db.code().as_deref().unwrap_or("?"),
                constraint = db.constraint().unwrap_or("-"),
                "database error"
            ),
            None => tracing::error!(error = %err, "database error"),
        }
        Self::internal()
    }
}

impl From<JsonRejection> for Problem {
    fn from(rejection: JsonRejection) -> Self {
        Self::invalid(rejection.body_text())
    }
}

impl From<PathRejection> for Problem {
    fn from(rejection: PathRejection) -> Self {
        Self::invalid(rejection.body_text())
    }
}

impl From<QueryRejection> for Problem {
    fn from(rejection: QueryRejection) -> Self {
        Self::invalid(rejection.body_text())
    }
}

/// `Query`, with rejections reported as Problem Details.
#[derive(axum::extract::FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(Problem))]
pub struct Query<T>(pub T);

/// `Json`, with rejections reported as Problem Details.
#[derive(axum::extract::FromRequest)]
#[from_request(via(axum::Json), rejection(Problem))]
pub struct Json<T>(pub T);

/// `Path`, with rejections reported as Problem Details.
#[derive(axum::extract::FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(Problem))]
pub struct Path<T>(pub T);

/// Whether a database error is a unique-constraint violation.
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .and_then(|e| e.code())
        .is_some_and(|c| c == "23505")
}
