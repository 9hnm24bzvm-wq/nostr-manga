//! RECONSTRUCTED, UNVERIFIED safe public errors. Reconcile existing internal ApiError bridge.
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub type PublicApiResult<T> = Result<T, PublicApiError>;
#[derive(Debug)]
pub struct PublicApiError {
    pub status: StatusCode,
    pub title: &'static str,
    pub detail: String,
}
impl PublicApiError {
    pub fn new(status: StatusCode, title: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status,
            title,
            detail: detail.into(),
        }
    }
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "Bad Request", detail)
    }
    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Unauthorized", detail)
    }
    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "Forbidden", detail)
    }
    pub fn not_found() -> Self {
        Self::not_found_detail("Resource not found")
    }
    pub fn not_found_detail(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "Not Found", detail)
    }
    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "Conflict", detail)
    }
    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Service Unavailable",
            "Resource projection is unavailable",
        )
    }
    pub fn unsupported() -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            "Not Implemented",
            "This operation is not implemented",
        )
    }
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "The request could not be completed",
        )
    }
}
impl std::fmt::Display for PublicApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}
impl std::error::Error for PublicApiError {}
#[derive(Serialize)]
struct ErrorEnvelope {
    result: &'static str,
    errors: Vec<ErrorObject>,
}
#[derive(Serialize)]
struct ErrorObject {
    id: String,
    status: u16,
    title: &'static str,
    detail: String,
}
impl IntoResponse for PublicApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorEnvelope {
                result: "error",
                errors: vec![ErrorObject {
                    id: error_id(),
                    status: self.status.as_u16(),
                    title: self.title,
                    detail: self.detail,
                }],
            }),
        )
            .into_response();
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        response
    }
}

impl From<crate::error::ApiError> for PublicApiError {
    fn from(error: crate::error::ApiError) -> Self {
        match error.status.as_u16() {
            500..=599 if error.status != StatusCode::NOT_IMPLEMENTED => Self::unavailable(),
            _ => Self::new(
                error.status,
                error.status.canonical_reason().unwrap_or("Request Failed"),
                error.message,
            ),
        }
    }
}
impl From<tokio_postgres::Error> for PublicApiError {
    fn from(error: tokio_postgres::Error) -> Self {
        crate::error::ApiError::from(error).into()
    }
}
impl From<deadpool_postgres::PoolError> for PublicApiError {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        crate::error::ApiError::from(error).into()
    }
}

impl From<super::id::InvalidPublicUuid> for PublicApiError {
    fn from(_: super::id::InvalidPublicUuid) -> Self {
        Self::bad_request("Expected a public UUID")
    }
}
fn error_id() -> String {
    let mut b = rand::random::<[u8; 16]>();
    b[6] = (b[6] & 15) | 64;
    b[8] = (b[8] & 63) | 128;
    let s = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &s[..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..]
    )
}
