//! HTTP shaping after policy checks and a fully verified, budgeted snapshot.
//! HEAD still performs the same verification as GET, but creates no body task.
use crate::download_budget::Snapshot;
use axum::{
    body::Body,
    http::{HeaderMap, Method, StatusCode, header},
    response::Response,
};

pub(super) fn response(
    snapshot: Snapshot,
    mime: &str,
    method: &Method,
    headers: &HeaderMap,
) -> Response {
    let length = snapshot.len();
    if *method == Method::HEAD {
        drop(snapshot);
        return empty(StatusCode::OK, mime, length);
    }
    // We emit no validators. An If-Range condition cannot be proven, so ignore
    // Range rather than returning an incorrectly validated partial response.
    let range = if headers.contains_key(header::IF_RANGE) {
        Ok(None)
    } else {
        crate::blossom::range(
            headers.get(header::RANGE).and_then(|h| h.to_str().ok()),
            length,
        )
    };
    match range {
        Ok(range) => {
            let mut reply = snapshot.response(range, mime, false);
            reply
                .headers_mut()
                .insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
            if let Some((start, end)) = range {
                *reply.status_mut() = StatusCode::PARTIAL_CONTENT;
                reply.headers_mut().insert(
                    header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{length}").parse().unwrap(),
                );
            }
            reply
        }
        Err(_) => {
            drop(snapshot);
            let mut reply = empty(StatusCode::RANGE_NOT_SATISFIABLE, mime, 0);
            reply.headers_mut().insert(
                header::CONTENT_RANGE,
                format!("bytes */{length}").parse().unwrap(),
            );
            reply
        }
    }
}
fn empty(status: StatusCode, mime: &str, length: usize) -> Response {
    let mut reply = Response::new(Body::empty());
    *reply.status_mut() = status;
    reply
        .headers_mut()
        .insert(header::CONTENT_TYPE, mime.parse().unwrap());
    reply
        .headers_mut()
        .insert(header::CONTENT_LENGTH, length.into());
    reply
        .headers_mut()
        .insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    reply
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    reply
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    reply
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download_budget::Budget;
    use axum::body::to_bytes;
    fn snapshot(budget: &Budget, bytes: &[u8]) -> Snapshot {
        Snapshot::verified(bytes.to_vec(), budget.acquire(bytes.len() as u64).unwrap())
    }
    #[tokio::test]
    async fn exact_ranges_and_large_decimal_clamping() {
        for (value, expected, status) in [
            ("bytes=1-3", &b"bcd"[..], StatusCode::PARTIAL_CONTENT),
            ("bytes=-2", &b"ef"[..], StatusCode::PARTIAL_CONTENT),
            ("bytes=4-", &b"ef"[..], StatusCode::PARTIAL_CONTENT),
            ("bytes=0-1,4-5", &b"abcdef"[..], StatusCode::OK),
            (
                "bytes=-999999999999999999999999999999999",
                &b"abcdef"[..],
                StatusCode::PARTIAL_CONTENT,
            ),
            (
                "bytes=1-999999999999999999999999999999999",
                &b"bcdef"[..],
                StatusCode::PARTIAL_CONTENT,
            ),
        ] {
            let budget = Budget::new(1, 65536).unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(header::RANGE, value.parse().unwrap());
            let reply = response(
                snapshot(&budget, b"abcdef"),
                "image/png",
                &Method::GET,
                &headers,
            );
            assert_eq!(reply.status(), status);
            assert_eq!(reply.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                to_bytes(reply.into_body(), 32).await.unwrap().as_ref(),
                expected
            );
        }
    }
    #[tokio::test]
    async fn head_ignores_range_and_immediately_releases_snapshot() {
        let budget = Budget::new(1, 65536).unwrap();
        for range in [None, Some("bytes=0-0"), Some("bytes=100-")] {
            let mut headers = HeaderMap::new();
            if let Some(range) = range {
                headers.insert(header::RANGE, range.parse().unwrap());
            }
            let reply = response(
                snapshot(&budget, b"abcdef"),
                "image/png",
                &Method::HEAD,
                &headers,
            );
            assert_eq!(reply.status(), StatusCode::OK);
            assert_eq!(reply.headers()[header::CONTENT_LENGTH], "6");
            assert!(!reply.headers().contains_key(header::CONTENT_RANGE));
            assert!(budget.acquire(6).is_ok());
            assert!(to_bytes(reply.into_body(), 32).await.unwrap().is_empty());
        }
    }
    #[tokio::test]
    async fn unsatisfiable_is_empty_and_if_range_falls_back() {
        let budget = Budget::new(1, 65536).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            "bytes=999999999999999999999999999999999-".parse().unwrap(),
        );
        let reply = response(
            snapshot(&budget, b"abcdef"),
            "image/png",
            &Method::GET,
            &headers,
        );
        assert_eq!(reply.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(reply.headers()[header::CONTENT_RANGE], "bytes */6");
        assert!(to_bytes(reply.into_body(), 32).await.unwrap().is_empty());
        headers.insert(header::IF_RANGE, "\"unknown\"".parse().unwrap());
        let reply = response(
            snapshot(&budget, b"abcdef"),
            "image/png",
            &Method::GET,
            &headers,
        );
        assert_eq!(reply.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(reply.into_body(), 32).await.unwrap().as_ref(),
            b"abcdef"
        );
    }
    #[tokio::test]
    async fn disconnected_partial_body_releases_snapshot_lease() {
        let budget = Budget::new(1, 100000).unwrap();
        let bytes = vec![1; 32000];
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=1-30000".parse().unwrap());
        let reply = response(
            snapshot(&budget, &bytes),
            "image/png",
            &Method::GET,
            &headers,
        );
        assert_eq!(reply.status(), StatusCode::PARTIAL_CONTENT);
        drop(reply);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if budget.acquire(32000).is_ok() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("disconnected response retained its lease");
    }
}
