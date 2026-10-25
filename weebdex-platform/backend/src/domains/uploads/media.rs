use super::storage;
use crate::{
    domains::auth,
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::Response,
};
use manga_node::read_admission::{Class, Permit};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, AsyncSeekExt, ReadBuf},
    sync::OwnedSemaphorePermit,
};
use tokio_postgres::Row;
use tokio_util::io::ReaderStream;

pub async fn chapter(
    State(state): State<AppState>,
    Path((chapter, name)): Path<(String, String)>,
    headers: HeaderMap,
    method: Method,
) -> ApiResult<Response> {
    let chapter = crate::public_api::media::native_media_id(
        &state,
        crate::public_api::ResourceKind::Chapter,
        &chapter,
    )
    .await?;
    let mut lease = Some(permits(&state).await?);
    if let Some(response) =
        crate::protocol::media::chapter(&state, &chapter, &name, &headers, &method, &mut lease)
            .await?
    {
        return Ok(response);
    }
    let permits = lease.take().ok_or_else(ApiError::unavailable)?;
    valid_name(&name)?;
    let row=state.db.client().await?.query_opt("SELECT o.storage_key,o.digest,o.mime,o.bytes,c.protocol_event_id,p.derived FROM (SELECT chapter_id,name,object_id,FALSE derived FROM platform.chapter_pages UNION ALL SELECT chapter_id,name,object_id,TRUE derived FROM platform.chapter_page_derivatives) p JOIN platform.media_objects o ON o.id=p.object_id JOIN platform.catalog_chapters c ON c.id=p.chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE p.chapter_id=$1 AND p.name=$2 AND c.deleted_at IS NULL AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id) LIMIT 1",&[&chapter,&name]).await?.ok_or_else(||ApiError::not_found("Chapter page not found"))?;
    if row.get::<_, Option<String>>(4).is_some() && !row.get::<_, bool>(5) {
        return Err(ApiError::unavailable());
    }
    serve(&state, &row, &headers, &method, false, permits).await
}
pub async fn preview(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    method: Method,
) -> ApiResult<Response> {
    let permits = permits(&state).await?;
    let (id, ext) = valid_name(&name)?;
    let actor = auth::require_principal(&state, &headers).await?;
    if actor.is_api_client || actor.recovery_session {
        return Err(ApiError::forbidden(
            "A browser session is required for upload previews",
        ));
    }
    let row=state.db.client().await?.query_opt("SELECT o.storage_key,o.digest,o.mime,o.bytes FROM platform.upload_files f JOIN platform.upload_sessions s ON s.id=f.session_id JOIN platform.media_objects o ON o.id=f.object_id WHERE f.id=$1 AND o.extension=$2 AND s.owner_id=$3 AND s.state='open' AND (s.expires_at>clock_timestamp() OR EXISTS(SELECT 1 FROM platform.protocol_publication_commands p WHERE p.upload_session_id=s.id AND p.user_id=s.owner_id AND p.state IN ('prepared','source_accepted')))",&[&id,&ext,&actor.user_id]).await?.ok_or_else(||ApiError::not_found("Upload preview not found"))?;
    serve(&state, &row, &headers, &method, true, permits).await
}
pub async fn cover(
    State(state): State<AppState>,
    Path((manga, name)): Path<(String, String)>,
    headers: HeaderMap,
    method: Method,
) -> ApiResult<Response> {
    let manga = crate::public_api::media::native_media_id(
        &state,
        crate::public_api::ResourceKind::Manga,
        &manga,
    )
    .await?;
    let name = if let Some((id, suffix)) = name.split_once('.') {
        if id.len() == 36 {
            format!(
                "{}.{}",
                crate::public_api::media::native_media_id(
                    &state,
                    crate::public_api::ResourceKind::Cover,
                    id
                )
                .await?,
                suffix
            )
        } else {
            name
        }
    } else {
        name
    };
    let mut lease = Some(permits(&state).await?);
    if let Some(response) =
        crate::protocol::media::cover(&state, &manga, &name, &headers, &method, &mut lease).await?
    {
        return Ok(response);
    }
    let permits = lease.take().ok_or_else(ApiError::unavailable)?;
    let (id, variant, ext) = cover_name(&name)?;
    let row=state.db.client().await?.query_opt("SELECT o.storage_key,o.digest,o.mime,o.bytes,m.state,m.owner_id FROM platform.cover_media media JOIN platform.media_objects o ON o.id=media.object_id JOIN platform.catalog_covers c ON c.id=media.cover_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.manga_id=$2 AND media.variant=$3 AND o.extension=$4 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&id,&manga,&variant,&ext]).await?.ok_or_else(||ApiError::not_found("Cover image not found"))?;
    let private = row.get::<_, String>(4) != "published";
    if private {
        let actor = auth::optional_principal(&state, &headers)
            .await?
            .ok_or_else(|| ApiError::not_found("Cover image not found"))?;
        let owner: String = row.get(5);
        if !(actor.user_id == owner && actor.has_perm("manga.create")
            || actor.is_moderator() && actor.has_perm("manga.cover.edit"))
        {
            return Err(ApiError::not_found("Cover image not found"));
        }
    }
    serve(&state, &row, &headers, &method, private, permits).await
}
pub(crate) struct Permits {
    _read: Permit,
    _request: OwnedSemaphorePermit,
    pub(crate) cleanup: Option<super::proxy::ProxyFile>,
    pub(crate) proxy: Option<OwnedSemaphorePermit>,
}
impl Permits {
    pub(crate) fn new(request: OwnedSemaphorePermit, read: Permit) -> Self {
        Self {
            _read: read,
            _request: request,
            cleanup: None,
            proxy: None,
        }
    }
}
pub(crate) async fn permits(state: &AppState) -> ApiResult<Permits> {
    let request = state
        .requests
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::busy())?;
    let read = state
        .reads
        .acquire(Class::Live)
        .await
        .map_err(|_| ApiError::busy())?;
    Ok(Permits::new(request, read))
}
struct AdmittedReader<R> {
    inner: R,
    _permits: Permits,
}
impl<R: AsyncRead + Unpin> AsyncRead for AdmittedReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
async fn serve(
    state: &AppState,
    row: &Row,
    headers: &HeaderMap,
    method: &Method,
    private: bool,
    permits: Permits,
) -> ApiResult<Response> {
    let key: String = row.get(0);
    let digest: String = row.get(1);
    let mime: String = row.get(2);
    let length = row.get::<_, i64>(3) as u64;
    let path = storage::object_path(&state.config.uploads_dir, &key)?;
    let file = File::open(path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ApiError::not_found("Media file unavailable")
        } else {
            ApiError::unavailable()
        }
    })?;
    let actual = file.metadata().await.map_err(|_| ApiError::unavailable())?;
    if !actual.is_file() || actual.len() != length {
        return Err(ApiError::unavailable());
    }
    file_response(
        file, &digest, &mime, length, headers, method, private, permits,
    )
    .await
}
/// The caller has transaction-rechecked account and exact session/file ownership.
/// Keep immutable object validation and bounded streaming shared with native media.
pub(crate) async fn signed_upload_preview(
    state: &AppState,
    row: &Row,
    headers: &HeaderMap,
) -> ApiResult<Response> {
    let digest: &str = row.get(1);
    let mime: &str = row.get(2);
    let bytes: i64 = row.get(3);
    if digest.len() != 64
        || !digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !["image/jpeg", "image/png", "image/webp", "image/gif"].contains(&mime)
        || bytes <= 0
        || bytes as u64 > storage::MAX_FILE_BYTES
    {
        return Err(ApiError::unavailable());
    }
    serve(state, row, headers, &Method::GET, true, permits(state).await?).await
}
pub(crate) async fn file_response(
    mut file: File,
    digest: &str,
    mime: &str,
    length: u64,
    headers: &HeaderMap,
    method: &Method,
    private: bool,
    permits: Permits,
) -> ApiResult<Response> {
    let etag = format!("\"{digest}\"");
    let mut builder = Response::builder()
        .header(header::ETAG, &etag)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CACHE_CONTROL,
            if private {
                "private, no-store"
            } else {
                "no-cache, must-revalidate"
            },
        );
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
        .is_none_or(|value| value == etag)
    {
        headers.get(header::RANGE).and_then(|h| h.to_str().ok())
    } else {
        None
    };
    let (start, end, status) = match range_header {
        None => (0, length - 1, StatusCode::OK),
        Some(value) => match parse_range(value, length) {
            Some((start, end)) => (start, end, StatusCode::PARTIAL_CONTENT),
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
    let bytes = end - start + 1;
    builder = builder.status(status).header(header::CONTENT_LENGTH, bytes);
    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{length}"),
        );
    }
    if method == Method::HEAD {
        return builder
            .body(Body::empty())
            .map_err(|_| ApiError::unavailable());
    }
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|_| ApiError::unavailable())?;
    let reader = AdmittedReader {
        inner: file.take(bytes),
        _permits: permits,
    };
    builder
        .body(Body::from_stream(ReaderStream::with_capacity(
            reader,
            64 * 1024,
        )))
        .map_err(|_| ApiError::unavailable())
}
pub fn valid_name(name: &str) -> ApiResult<(&str, &str)> {
    let (id, ext) = name
        .split_once('.')
        .ok_or_else(|| ApiError::bad_request("Invalid media name"))?;
    if !storage::valid_id(id) || !["jpg", "png", "webp", "gif"].contains(&ext) {
        return Err(ApiError::bad_request("Invalid media name"));
    }
    Ok((id, ext))
}
fn cover_name(name: &str) -> ApiResult<(&str, &str, &str)> {
    for (suffix, variant) in [(".256.webp", "256"), (".512.webp", "512")] {
        if let Some(id) = name.strip_suffix(suffix) {
            if storage::valid_id(id) {
                return Ok((id, variant, "webp"));
            } else {
                return Err(ApiError::bad_request("Invalid cover name"));
            }
        }
    }
    let (id, ext) = valid_name(name)?;
    Ok((id, "original", ext))
}
pub fn parse_range(value: &str, length: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return None;
    }
    let range = value.strip_prefix("bytes=")?;
    if range.contains(',') {
        return None;
    }
    let (start, end) = range.split_once('-')?;
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
    fn single_ranges_have_exact_bounds_and_reject_multiple_or_overflowing_ranges() {
        assert_eq!(parse_range("bytes=0-2", 10), Some((0, 2)));
        assert_eq!(parse_range("bytes=7-", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=0-999", 10), Some((0, 9)));
        for text in [
            "bytes=10-",
            "bytes=3-1",
            "bytes=-0",
            "bytes=0-1,3-4",
            "bytes=18446744073709551616-",
            "items=0-1",
            "bytes=--1",
        ] {
            assert_eq!(parse_range(text, 10), None);
        }
    }
    #[test]
    fn path_and_variant_names_are_exact_frontend_filenames() {
        assert_eq!(valid_name("Abcdef1234.jpg").unwrap(), ("Abcdef1234", "jpg"));
        assert_eq!(
            cover_name("Abcdef1234.512.webp").unwrap(),
            ("Abcdef1234", "512", "webp")
        );
        for value in [
            "../../secret",
            "Abcdef1234.jpg/other",
            "Abcdef1234.1024.webp",
            "Abcdef1234.webp?secret=1",
            "%2e%2e.png",
        ] {
            assert!(cover_name(value).is_err());
        }
    }
    async fn media_permits() -> (
        Permits,
        std::sync::Arc<manga_node::read_admission::Admission>,
        std::sync::Arc<tokio::sync::Semaphore>,
    ) {
        let mut config = manga_node::read_admission::Config::for_database(1);
        config.wait_ms = 1;
        let admission =
            std::sync::Arc::new(manga_node::read_admission::Admission::new(config, 1).unwrap());
        let requests = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let read = admission.acquire(Class::Live).await.unwrap();
        let request = requests.clone().acquire_owned().await.unwrap();
        (
            Permits {
                _read: read,
                _request: request,
                cleanup: None,
                proxy: None,
            },
            admission,
            requests,
        )
    }
    #[tokio::test]
    async fn http_ranges_etags_and_cancelled_body_release_owned_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image");
        tokio::fs::write(&path, b"0123456789").await.unwrap();
        let (permits, admission, requests) = media_permits().await;
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=2-5".parse().unwrap());
        let response = file_response(
            File::open(&path).await.unwrap(),
            "digest",
            "image/png",
            10,
            &headers,
            &Method::GET,
            false,
            permits,
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "4");
        assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
        assert_eq!(requests.available_permits(), 0);
        assert!(admission.acquire(Class::Live).await.is_err());
        let bytes = axum::body::to_bytes(response.into_body(), 100)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"2345");
        assert_eq!(requests.available_permits(), 1);
        drop(admission.acquire(Class::Live).await.unwrap());
        let (permits, admission, requests) = media_permits().await;
        let response = file_response(
            File::open(&path).await.unwrap(),
            "digest",
            "image/png",
            10,
            &HeaderMap::new(),
            &Method::GET,
            false,
            permits,
        )
        .await
        .unwrap();
        assert_eq!(requests.available_permits(), 0);
        drop(response);
        assert_eq!(requests.available_permits(), 1);
        drop(admission.acquire(Class::Live).await.unwrap());
        let (permits, _, requests) = media_permits().await;
        headers.insert(header::IF_NONE_MATCH, "W/\"digest\"".parse().unwrap());
        let response = file_response(
            File::open(&path).await.unwrap(),
            "digest",
            "image/png",
            10,
            &headers,
            &Method::GET,
            false,
            permits,
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(requests.available_permits(), 1);
    }
    #[tokio::test]
    async fn http_head_invalid_ranges_and_if_range_preserve_actual_lengths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image");
        tokio::fs::write(&path, b"0123456789").await.unwrap();
        let (permits, _, requests) = media_permits().await;
        let response = file_response(
            File::open(&path).await.unwrap(),
            "digest",
            "image/png",
            10,
            &HeaderMap::new(),
            &Method::HEAD,
            true,
            permits,
        )
        .await
        .unwrap();
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "10");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
        assert_eq!(requests.available_permits(), 1);
        assert!(
            axum::body::to_bytes(response.into_body(), 1)
                .await
                .unwrap()
                .is_empty()
        );
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=99-".parse().unwrap());
        let (permits, _, _) = media_permits().await;
        let response = file_response(
            File::open(&path).await.unwrap(),
            "digest",
            "image/png",
            10,
            &headers,
            &Method::GET,
            false,
            permits,
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */10");
        headers.insert(header::IF_RANGE, "\"other-digest\"".parse().unwrap());
        let (permits, _, _) = media_permits().await;
        let response = file_response(
            File::open(&path).await.unwrap(),
            "digest",
            "image/png",
            10,
            &headers,
            &Method::GET,
            false,
            permits,
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "10");
    }
}
