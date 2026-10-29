use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub type ApiResult<T> = Result<T, ApiError>;

/// Public messages must describe the operation, never internal SQL or secrets.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    code: &'a str,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Sign in to continue",
        )
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "permission_denied", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }

    pub fn busy() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "capacity_busy",
            "Service is busy; retry shortly",
        )
    }

    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Service is temporarily unavailable",
        )
    }

    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_IMPLEMENTED, "not_implemented", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                error: &self.message,
                code: self.code,
            }),
        )
            .into_response();
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
        }
        response
    }
}

impl From<tokio_postgres::Error> for ApiError {
    fn from(error: tokio_postgres::Error) -> Self {
        match error.code().map(|c| c.code()) {
            Some("P6001") => return Self::new(StatusCode::CONFLICT, "last_administrator_protected", "Appoint another enabled, verified, unrestricted administrator with Manage staff access before disabling, restricting, demoting or changing this account's email. Nothing was changed."),
            Some("P6002") => return Self::conflict("Authority revision exhausted; contact the instance operator"),
            Some("P6003") => return Self::new(StatusCode::CONFLICT, "authority_busy", "Account authority is changing. Refresh and review again before retrying; nothing was changed."),
            _ => {},
        }
        // Error codes are safe to log; SQL, parameters and server detail are not.
        tracing::warn!(
            sqlstate = error.code().map(|c| c.code()),
            "platform database operation failed"
        );
        Self::unavailable()
    }
}

impl From<deadpool_postgres::PoolError> for ApiError {
    fn from(_: deadpool_postgres::PoolError) -> Self {
        Self::unavailable()
    }
}
