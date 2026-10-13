//! A bounded producer keeps its byte lease until output is consumed, disconnected
//! or timed out. Copied 8KiB frames cannot retain the whole backing allocation.
use axum::{
    body::{Body, Bytes},
    http::{Response, header},
};
use std::{io, time::Duration};
use tokio::sync::mpsc;
fn channel(
    bytes: Vec<u8>,
    lease: impl Send + 'static,
    deadline: Duration,
) -> mpsc::Receiver<Result<Bytes, io::Error>> {
    let (tx, rx) = mpsc::channel(1);
    crate::node::spawn_owned_scope(async move {
        let _lease = lease;
        let expires = tokio::time::Instant::now() + deadline;
        for chunk in bytes.chunks(8192) {
            let frame = Bytes::copy_from_slice(chunk);
            match tokio::time::timeout_at(expires, tx.send(Ok(frame))).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => return,
                Err(_) => {
                    let _ = tx.try_send(Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "response deadline",
                    )));
                    return;
                }
            }
        }
    });
    rx
}
pub(crate) fn response(
    bytes: Vec<u8>,
    lease: impl Send + 'static,
    content_type: &str,
    private: bool,
    gzip: bool,
) -> Response<Body> {
    let mut bytes = bytes;
    bytes.shrink_to_fit();
    let length = bytes.len();
    let rx = channel(bytes, lease, Duration::from_secs(120));
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    let mut response = Response::new(Body::from_stream(stream));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, length.into());
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        if private {
            "private, no-store"
        } else {
            "no-store"
        }
        .parse()
        .unwrap(),
    );
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    if gzip {
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, "gzip".parse().unwrap());
        response
            .headers_mut()
            .insert(header::VARY, "accept-encoding".parse().unwrap());
    }
    response
}
#[cfg(test)]
mod public_contract_tests {
    use crate::bounded_response::response;
    use axum::{body::to_bytes, http::header};
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    #[tokio::test]
    async fn normal_response_streams_exact_bytes_and_releases_its_external_lease() {
        let data = vec![47; 100_000];
        let budget = Arc::new(Semaphore::new(100_000));
        let lease = budget.clone().try_acquire_many_owned(100_000).unwrap();
        let reply = response(data.clone(), lease, "application/cbor", false, false);
        assert_eq!(reply.headers()[header::CONTENT_TYPE], "application/cbor");
        assert_eq!(reply.headers()[header::CONTENT_LENGTH], "100000");
        assert_eq!(reply.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!reply.headers().contains_key(header::CONTENT_ENCODING));
        assert!(!reply.headers().contains_key(header::VARY));
        assert_eq!(
            to_bytes(reply.into_body(), 100_000).await.unwrap().as_ref(),
            data.as_slice()
        );
        let next = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            budget.acquire_many_owned(100_000),
        )
        .await
        .unwrap()
        .unwrap();
        drop(next);
    }

    #[tokio::test]
    async fn independently_encoded_gzip_bytes_keep_their_http_contract() {
        // Python gzip.compress(b"chapter one\n", mtime=0), fixed independent bytes.
        let compressed =
            hex::decode("1f8b08000000000002034bce482c28492d52c8cf4be5020014ab64af0c000000")
                .unwrap();
        let budget = Arc::new(Semaphore::new(32));
        let lease = budget.clone().try_acquire_many_owned(32).unwrap();
        let reply = response(
            compressed.clone(),
            lease,
            "application/octet-stream",
            true,
            true,
        );
        assert_eq!(reply.status(), 200);
        assert_eq!(reply.headers()[header::CONTENT_ENCODING], "gzip");
        assert_eq!(reply.headers()[header::VARY], "accept-encoding");
        assert_eq!(reply.headers()[header::CONTENT_LENGTH], "32");
        assert_eq!(
            reply.headers()[header::CONTENT_TYPE],
            "application/octet-stream"
        );
        assert_eq!(reply.headers()[header::CACHE_CONTROL], "private, no-store");
        assert_eq!(reply.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        let body = to_bytes(reply.into_body(), 32).await.unwrap();
        assert_eq!(body.as_ref(), compressed.as_slice());
        use std::io::Read;
        let mut plain = String::new();
        flate2::read::GzDecoder::new(body.as_ref())
            .read_to_string(&mut plain)
            .unwrap();
        assert_eq!(plain, "chapter one\n");
        let next = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            budget.acquire_many_owned(32),
        )
        .await
        .unwrap()
        .unwrap();
        drop(next);
    }
}
