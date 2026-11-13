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
