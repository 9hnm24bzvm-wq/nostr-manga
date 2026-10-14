//! Fixture-only, closed observations at the actual feed database boundaries.
//! These categories are observations, not diagnoses: 57014 also covers an
//! explicit cancellation, so `query-canceled` does not assert a timeout cause.
use crate::error::ApiError;
use axum::{
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use deadpool_postgres::{PoolError, TimeoutType};
use tokio_postgres::error::SqlState;

pub(super) const HEADER: &str = "x-weebdex-fixture-feed-failure";

pub struct FeedError {
    error: ApiError,
    failure: Option<&'static str>,
}

impl From<ApiError> for FeedError {
    fn from(error: ApiError) -> Self {
        Self {
            error,
            failure: None,
        }
    }
}

fn pool_failure(error: &PoolError) -> &'static str {
    match error {
        PoolError::Timeout(TimeoutType::Wait) => "pool-wait-timeout",
        PoolError::Timeout(TimeoutType::Create) => "pool-create-timeout",
        PoolError::Timeout(TimeoutType::Recycle) => "pool-recycle-timeout",
        PoolError::Closed => "pool-closed",
        PoolError::Backend(_) => "pool-backend",
        PoolError::NoRuntimeSpecified | PoolError::PostCreateHook(_) => "pool-other",
    }
}

fn query_failure(code: Option<&SqlState>, closed: bool) -> &'static str {
    match code {
        Some(code) if code == &SqlState::QUERY_CANCELED => "query-canceled",
        Some(code) if code == &SqlState::LOCK_NOT_AVAILABLE => "query-lock",
        Some(code) if code.code().starts_with("08") => "query-connection",
        Some(_) => "query-database",
        None if closed => "query-connection",
        None => "query-client",
    }
}

impl FeedError {
    pub(super) fn pool(error: PoolError, enabled: bool) -> Self {
        let failure = enabled.then(|| pool_failure(&error));
        Self {
            error: error.into(),
            failure,
        }
    }

    pub(super) fn query(error: tokio_postgres::Error, enabled: bool) -> Self {
        let failure = enabled.then(|| query_failure(error.code(), error.is_closed()));
        Self {
            error: error.into(),
            failure,
        }
    }
}

impl IntoResponse for FeedError {
    fn into_response(self) -> Response {
        let mut response = self.error.into_response();
        if let Some(failure) = self.failure {
            response
                .headers_mut()
                .insert(HEADER, HeaderValue::from_static(failure));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::StatusCode};
    use deadpool_postgres::{Manager, Pool};
    use serde_json::json;
    use tokio_postgres::NoTls;

    #[test]
    fn closed_classification_never_includes_sqlstate_or_server_text() {
        for (error, expected) in [
            (PoolError::Timeout(TimeoutType::Wait), "pool-wait-timeout"),
            (
                PoolError::Timeout(TimeoutType::Create),
                "pool-create-timeout",
            ),
            (
                PoolError::Timeout(TimeoutType::Recycle),
                "pool-recycle-timeout",
            ),
            (PoolError::Closed, "pool-closed"),
            (PoolError::NoRuntimeSpecified, "pool-other"),
        ] {
            assert_eq!(pool_failure(&error), expected);
        }
        for (code, expected) in [
            ("57014", "query-canceled"),
            ("55P03", "query-lock"),
            ("08006", "query-connection"),
            ("42P01", "query-database"),
            ("42883", "query-database"),
            ("54001", "query-database"),
            ("private server data", "query-database"),
        ] {
            assert_eq!(
                query_failure(Some(&SqlState::from_code(code)), false),
                expected
            );
        }
        assert_eq!(query_failure(None, true), "query-connection");
        assert_eq!(query_failure(None, false), "query-client");
    }

    #[tokio::test]
    async fn actual_closed_deadpool_error_preserves_response_and_fixture_gate() {
        // A real deadpool failure, with no listener, connection or database.
        let manager = Manager::new("host=127.0.0.1".parse().unwrap(), NoTls);
        let pool = Pool::builder(manager).build().unwrap();
        pool.close();
        for enabled in [false, true] {
            let error = pool.get().await.unwrap_err();
            assert!(matches!(error, PoolError::Closed));
            let response = FeedError::pool(error, enabled).into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(
                response.headers().get(HEADER).map(|h| h.to_str().unwrap()),
                enabled.then_some("pool-closed")
            );
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                json!({"error":"Service is temporarily unavailable","code":"unavailable"})
            );
        }
        let response = FeedError::from(ApiError::bad_request("Invalid feed")).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!response.headers().contains_key(HEADER));
    }

    #[tokio::test]
    async fn actual_tokio_postgres_query_errors_are_closed_and_fixture_gated() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // In-memory protocol peer: real tokio-postgres query/error decoding,
        // explicitly no PostgreSQL server, SQL execution, listener or fixture.
        for (code, expected) in [
            ("57014", "query-canceled"),
            ("55P03", "query-lock"),
            ("08006", "query-connection"),
            ("42P01", "query-database"),
        ] {
            for enabled in [false, true] {
                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    let (stream, mut peer) = tokio::io::duplex(4096);
                    let server = tokio::spawn(async move {
                        let length = peer.read_u32().await.unwrap();
                        assert!((8..=4096).contains(&length));
                        let mut startup = vec![0; (length - 4) as usize];
                        peer.read_exact(&mut startup).await.unwrap();
                        peer.write_all(b"R\0\0\0\x08\0\0\0\0Z\0\0\0\x05I").await.unwrap();
                        // The actual query_one prepares first. Drain through
                        // Sync before returning the typed ErrorResponse.
                        for _ in 0..4 {
                            let tag = peer.read_u8().await.unwrap();
                            let length = peer.read_u32().await.unwrap();
                            assert!((4..=4096).contains(&length));
                            let mut payload = vec![0; (length - 4) as usize];
                            peer.read_exact(&mut payload).await.unwrap();
                            if tag == b'S' {
                                let fields = format!("SERROR\0VERROR\0C{code}\0Mprivate SQL URL\0Dprivate server detail\0\0");
                                peer.write_u8(b'E').await.unwrap();
                                peer.write_u32((fields.len() + 4) as u32).await.unwrap();
                                peer.write_all(fields.as_bytes()).await.unwrap();
                                peer.write_all(b"Z\0\0\0\x05I").await.unwrap();
                                let _ = peer.read_u8().await;
                                return;
                            }
                        }
                        panic!("Synthetic query did not send its bounded Sync");
                    });
                    let mut config = tokio_postgres::Config::new();
                    config.user("synthetic").ssl_mode(tokio_postgres::config::SslMode::Disable);
                    let (client, connection) = config.connect_raw(stream, NoTls).await.unwrap();
                    let connection = tokio::spawn(connection);
                    let error = client.query_one("SELECT synthetic", &[]).await.unwrap_err();
                    assert_eq!(error.code().unwrap().code(), code);
                    let response = FeedError::query(error, enabled).into_response();
                    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
                    assert_eq!(response.headers().get(HEADER).map(|h| h.to_str().unwrap()), enabled.then_some(expected));
                    let body = to_bytes(response.into_body(), 4096).await.unwrap();
                    assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap(), json!({"error":"Service is temporarily unavailable","code":"unavailable"}));
                    drop(client);
                    server.await.unwrap();
                    let _ = connection.await.unwrap();
                }).await.expect("Finite in-memory protocol seam deadline");
            }
        }
    }
}
