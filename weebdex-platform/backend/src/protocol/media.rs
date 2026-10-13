//! Reader-compatible URLs backed by verified, self-hosted protocol originals.
//! Source metadata is checked at every request; original bytes are obtained only
//! from the original module's bounded public download path or explicitly local
//! original-serving endpoint. Remote peer URLs are never used as a reader CDN.
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    body::{Body, Bytes, HttpBody},
    http::{HeaderMap, Method, StatusCode, header},
    response::Response,
};
use http_body::{Frame, SizeHint};
use manga_node::{auth, db::Id};
use serde_json::Value;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// Admission follows the actual streamed original body, including cancellation.
/// The original Snapshot independently retains its bounded download ownership.
struct AdmittedOriginal {
    inner: Body,
    permits: Option<crate::domains::uploads::media::Permits>,
}
impl HttpBody for AdmittedOriginal {
    type Data = Bytes;
    type Error = axum::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let result = Pin::new(&mut self.inner).poll_frame(cx);
        if matches!(result, Poll::Ready(None) | Poll::Ready(Some(Err(_))))
            || self.inner.is_end_stream()
        {
            self.permits.take();
        }
        result
    }
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
fn retain_admission(
    response: Response,
    permits: crate::domains::uploads::media::Permits,
) -> Response {
    response.map(|inner| {
        Body::new(AdmittedOriginal {
            inner,
            permits: Some(permits),
        })
    })
}
fn identity(text: &str) -> ApiResult<Id> {
    if !auth::valid_hash(text) {
        return Err(ApiError::bad_request("Invalid original identity"));
    }
    hex::decode(text)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| ApiError::bad_request("Invalid original identity"))
}
async fn visible_source(state: &AppState, event: &str) -> ApiResult<manga_node::catalogue::Entry> {
    state
        .protocol
        .entry(identity(event)?)
        .await?
        .ok_or_else(|| ApiError::not_found("Original protocol publication is unavailable"))
}
fn descriptor(page: &Value, hash: &str) -> ApiResult<(String, u64)> {
    if page["sha256"].as_str() != Some(hash) {
        return Err(ApiError::not_found(
            "Original does not belong to this publication",
        ));
    }
    let mime = page["mime"]
        .as_str()
        .filter(|s| ["image/png", "image/jpeg", "image/webp"].contains(s))
        .ok_or_else(ApiError::unavailable)?;
    let size = page["size"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(ApiError::unavailable)?;
    Ok((mime.to_owned(), size))
}
/// Return None only for an ordinary native chapter; source failures fail closed.
pub(crate) async fn chapter(
    state: &AppState,
    chapter: &str,
    name: &str,
    headers: &HeaderMap,
    method: &Method,
    lease: &mut Option<crate::domains::uploads::media::Permits>,
) -> ApiResult<Option<Response>> {
    let source: Option<String> = {
        let client = state.db.client().await?;
        let row=client.query_opt("SELECT c.protocol_event_id,(c.deleted_at IS NULL AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published') FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1",&[&chapter]).await?;
        if let Some(row) = row {
            let event: Option<String> = row.get(0);
            if event.is_some() && !row.get::<_, bool>(1) {
                return Err(ApiError::not_found(
                    "Original protocol publication is unavailable",
                ));
            }
            event
        } else {
            None
        }
    };
    let Some(event) = source else { return Ok(None) };
    let entry = visible_source(state, &event).await?;
    if entry.event.kind != 1001 {
        return Err(ApiError::unavailable());
    }
    let series = entry.metadata["series"]
        .as_str()
        .ok_or_else(ApiError::unavailable)?;
    let parent = visible_source(state, series).await?;
    if parent.event.kind != 1002 {
        return Err(ApiError::unavailable());
    }
    if !auth::valid_hash(name) {
        if name.split_once('.').is_some_and(|(id, ext)| {
            id.len() == 10
                && id.bytes().all(|b| b.is_ascii_alphanumeric())
                && ["jpg", "webp"].contains(&ext)
        }) {
            return Ok(None);
        }
        return Err(ApiError::bad_request(
            "Invalid protocol page or local derivative name",
        ));
    }

    let page = entry.metadata["pages"]
        .as_array()
        .and_then(|pages| pages.iter().find(|p| p["sha256"].as_str() == Some(name)))
        .ok_or_else(|| ApiError::not_found("Original does not belong to this chapter"))?;
    let (mime, size) = descriptor(page, name)?;
    let response = serve(state, name, &mime, size, headers, method).await?;
    Ok(Some(retain_admission(
        response,
        lease.take().ok_or_else(ApiError::unavailable)?,
    )))
}
pub(crate) async fn cover(
    state: &AppState,
    manga: &str,
    name: &str,
    headers: &HeaderMap,
    method: &Method,
    lease: &mut Option<crate::domains::uploads::media::Permits>,
) -> ApiResult<Option<Response>> {
    let (id, suffix) = name
        .split_once('.')
        .ok_or_else(|| ApiError::bad_request("Invalid cover image name"))?;
    if id.len() != 10 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Ok(None);
    }
    let source = {
        let client = state.db.client().await?;
        client.query_opt("SELECT s.event_id,s.blob_hash,c.ext FROM platform.protocol_cover_sources s JOIN platform.catalog_covers c ON c.id=s.cover_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.manga_id=$2 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published'",&[&id,&manga]).await?
    };
    let Some(source) = source else {
        return Ok(None);
    };
    let ext: String = source.get(2);
    if suffix != ext && !["256.webp", "512.webp"].contains(&suffix) {
        return Err(ApiError::not_found(
            "Protocol cover derivative is not materialized",
        ));
    }
    let event: String = source.get(0);
    let hash: String = source.get(1);
    let entry = visible_source(state, &event).await?;
    if entry.event.kind != 1002 {
        return Err(ApiError::unavailable());
    }
    let (mime, size) = descriptor(&entry.metadata["cover"], &hash)?;
    if suffix != ext {
        return Ok(None);
    }
    let response = serve(state, &hash, &mime, size, headers, method).await?;
    Ok(Some(retain_admission(
        response,
        lease.take().ok_or_else(ApiError::unavailable)?,
    )))
}
async fn serve(
    state: &AppState,
    hash: &str,
    mime: &str,
    length: u64,
    headers: &HeaderMap,
    method: &Method,
) -> ApiResult<Response> {
    let etag = format!("\"{hash}\"");
    let mut builder = Response::builder()
        .header(header::ETAG, &etag)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes");
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|s| {
            s.split(',').any(|v| {
                let v = v.trim();
                v == "*" || v == etag || v.strip_prefix("W/") == Some(&etag)
            })
        })
    {
        return builder
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .map_err(|_| ApiError::unavailable());
    }
    let range_header = if headers
        .get(header::IF_RANGE)
        .and_then(|h| h.to_str().ok())
        .is_none_or(|v| v == etag)
    {
        headers.get(header::RANGE).and_then(|h| h.to_str().ok())
    } else {
        None
    };
    let range = match range_header {
        None => None,
        Some(text) => match parse_range(text, length) {
            Some(range) => Some(range),
            None => {
                return builder
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{length}"))
                    .header(header::CONTENT_LENGTH, "0")
                    .body(Body::empty())
                    .map_err(|_| ApiError::unavailable());
            }
        },
    };
    if let Ok(runtime) = state.protocol.publication_runtime().await {
        let (snapshot, actual_mime) = runtime
            .download_original(identity(hash)?)
            .await
            .map_err(|error| {
                if error.is::<manga_node::download_budget::Busy>() {
                    ApiError::busy()
                } else {
                    ApiError::unavailable()
                }
            })?
            .ok_or_else(|| ApiError::not_found("Self-hosted original is unavailable"))?;
        if snapshot.len() as u64 != length || actual_mime != mime {
            return Err(ApiError::unavailable());
        }
        if method == Method::HEAD {
            drop(snapshot);
            builder = builder.header(
                header::CONTENT_LENGTH,
                range.map(|(s, e)| e - s + 1).unwrap_or(length),
            );
            if let Some((start, end)) = range {
                builder = builder.status(StatusCode::PARTIAL_CONTENT).header(
                    header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{length}"),
                );
            }
            return builder
                .body(Body::empty())
                .map_err(|_| ApiError::unavailable());
        }
        let mut response =
            snapshot.response(range.map(|(s, e)| (s as usize, e as usize)), mime, false);
        response.headers_mut().insert(
            header::ETAG,
            etag.parse().map_err(|_| ApiError::unavailable())?,
        );
        response
            .headers_mut()
            .insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
        if let Some((start, end)) = range {
            *response.status_mut() = StatusCode::PARTIAL_CONTENT;
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{length}")
                    .parse()
                    .map_err(|_| ApiError::unavailable())?,
            );
        }
        return Ok(response);
    }
    // This explicit operator assertion is required in addition to a configured
    // origin. A peer/source URL is never substituted for a self-hosted original.
    if !state.config.protocol_origin_self_hosted {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "original_runtime_not_configured",
            "Self-hosted protocol original serving is not configured",
        ));
    }
    let origin = state
        .config
        .protocol_origin
        .as_ref()
        .ok_or_else(ApiError::unavailable)?;
    let url = format!("{}/originals/{hash}", origin.trim_end_matches('/'));
    builder
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, url)
        .body(Body::empty())
        .map_err(|_| ApiError::unavailable())
}
fn parse_range(text: &str, length: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return None;
    }
    let (start, end) = text.strip_prefix("bytes=")?.split_once('-')?;
    if end.contains(',') {
        return None;
    }
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?;
        if suffix == 0 {
            return None;
        }
        Some((length.saturating_sub(suffix), length - 1))
    } else {
        let start = start.parse::<u64>().ok()?;
        let end = if end.is_empty() {
            length - 1
        } else {
            end.parse::<u64>().ok()?.min(length - 1)
        };
        if start >= length || start > end {
            return None;
        }
        Some((start, end))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_hashes_are_not_paths_and_ranges_are_bounded() {
        assert!(identity(&"ab".repeat(32)).is_ok());
        for bad in [
            "../../secret",
            "Abcdef1234.jpg",
            "https://peer.invalid/a",
            "AB",
        ]
        .iter()
        {
            assert!(identity(bad).is_err());
        }
        assert_eq!(parse_range("bytes=2-5", 10), Some((2, 5)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        for bad in ["bytes=10-", "bytes=3-1", "bytes=0-1,3-4", "bytes=-0"] {
            assert_eq!(parse_range(bad, 10), None);
        }
    }
    #[test]
    fn descriptors_cannot_reassign_an_original_to_another_hash() {
        let page = serde_json::json!({"sha256":"11".repeat(32),"mime":"image/png","size":10});
        assert!(descriptor(&page, &"11".repeat(32)).is_ok());
        assert!(descriptor(&page, &"22".repeat(32)).is_err());
    }
    #[tokio::test]
    async fn original_stream_retains_admission_until_consumed_or_dropped() {
        let requests = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let reads = manga_node::read_admission::Admission::new(
            manga_node::read_admission::Config::for_database(4),
            4,
        )
        .unwrap();
        let response = retain_admission(
            Response::new(Body::from("synthetic original")),
            crate::domains::uploads::media::Permits::new(
                requests.clone().try_acquire_owned().unwrap(),
                reads
                    .acquire(manga_node::read_admission::Class::Live)
                    .await
                    .unwrap(),
            ),
        );
        assert_eq!(requests.available_permits(), 0);
        let bytes = axum::body::to_bytes(response.into_body(), 128)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"synthetic original");
        assert_eq!(requests.available_permits(), 1);
        let response = retain_admission(
            Response::new(Body::from("cancelled original")),
            crate::domains::uploads::media::Permits::new(
                requests.clone().try_acquire_owned().unwrap(),
                reads
                    .acquire(manga_node::read_admission::Class::Live)
                    .await
                    .unwrap(),
            ),
        );
        assert_eq!(requests.available_permits(), 0);
        drop(response);
        assert_eq!(requests.available_permits(), 1);
    }
}
