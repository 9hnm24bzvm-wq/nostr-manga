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
