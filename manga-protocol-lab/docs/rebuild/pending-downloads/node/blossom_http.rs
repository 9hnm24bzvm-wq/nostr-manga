//! Blossom transport adapters. Retention and publisher authority remain shared services.
use super::*;
use axum::response::{IntoResponse, Response};
fn unauthorized() -> ApiError {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error":"blob_authorization_required_or_invalid"})),
    )
}
async fn authorization(
    a: &App,
    headers: &HeaderMap,
    action: &str,
    hash: &str,
) -> Result<auth::Event, ApiError> {
    if headers.get_all("authorization").iter().count() != 1 {
        return Err(unauthorized());
    }
    let header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(unauthorized)?;
    let event = crate::blossom::token(header).map_err(|_| unauthorized())?;
    let origin = url::Url::parse(&a.base_url).map_err(|_| unavailable())?;
    crate::blossom::scope(
        &event,
        origin.host_str().ok_or_else(unavailable)?,
        action,
        hash,
    )
    .map_err(|_| unauthorized())?;
    if !a
        .repo
        .authorization_clock_allowed(event.created_at)
        .await
        .map_err(|_| unavailable())?
    {
        return Err(unauthorized());
    }
    Ok(event)
}
async fn publisher(
    a: &App,
    event: &auth::Event,
) -> Result<crate::publisher_authority::PublisherPermit, ApiError> {
    a.repo
        .publisher_permit(id(&event.pubkey).map_err(|_| unauthorized())?)
        .await
        .map_err(publisher_error)
}
pub(super) async fn upload(
    State(a): State<Shared>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| crate::blossom::token(h).ok())
        .ok_or_else(unauthorized)?;
    let hashes = token.values("x");
    let claimed = headers
        .get("x-sha-256")
        .map(|v| v.to_str().map_err(|_| bad("invalid X-SHA-256")))
        .transpose()?;
    let prehash = claimed
        .or_else(|| hashes.first().copied())
        .ok_or_else(unauthorized)?;
    if !auth::valid_hash(prehash) {
        return Err(bad("invalid upload hash"));
    }
    let event = authorization(&a, &headers, "upload", prehash).await?;
    let authority = publisher(&a, &event).await?;
    let storage = a.storage.clone().ok_or_else(unavailable)?;
    let permit = storage.slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"upload_slots_busy"})),
        )
    })?;
    if headers
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse::<u64>().ok())
        .is_some_and(|n| n > storage.max_blob_bytes)
    {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        ));
    }
    let body = tokio::time::timeout(
        Duration::from_secs(30),
        axum::body::to_bytes(request.into_body(), storage.max_blob_bytes as usize),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::REQUEST_TIMEOUT,
            Json(json!({"error":"body_timeout"})),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error":"body_budget"})),
        )
    })?;
    if body.is_empty() {
        return Err(bad("empty upload"));
    }
    let hash = auth::hash(&body);
    if claimed.is_some_and(|h| h != hash) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error":"upload_hash_mismatch"})),
        ));
    }
    authorization(&a, &headers, "upload", &hash).await?;
    let mime = crate::manifest::detected_mime(&body).unwrap_or("application/octet-stream");
    let extension = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "bin",
    };
    let hash_id = id(&hash).map_err(|_| bad("invalid hash"))?;
    let outcome = tokio::spawn(async move {
        let _permit = permit;
        storage
            .put_for_publisher_detailed(&authority, hash_id, &body)
            .await
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(publisher_error)?;
    let uploaded:i64=a.repo.pool.get().await.map_err(|_|unavailable())?.query_one("SELECT floor(extract(epoch FROM created_at))::bigint FROM blob_intents WHERE hash=$1 AND generation=$2",&[&hash_id.as_slice(),&outcome.intent.key.generation.as_slice()]).await.map_err(|_|unavailable())?.get(0);
    Ok((if outcome.created {StatusCode::CREATED}else{StatusCode::OK},Json(json!({"url":format!("{}/{}.{}",a.base_url,hash,extension),"sha256":hash,"size":outcome.intent.size,"type":mime,"uploaded":uploaded,"staged":true,"staged_read_requires_authorization":true}))).into_response())
}
pub(super) async fn read(
    State(a): State<Shared>,
    Path(path): Path<String>,
    method: axum::http::Method,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let hash = crate::blossom::path_hash(&path).map_err(|_| bad("invalid blob path"))?;
    let hash_id = id(hash).map_err(|_| bad("invalid hash"))?;
    let event = if headers.contains_key("authorization") {
        Some(authorization(&a, &headers, "get", hash).await?)
    } else {
        None
    };
    if a.relay.read_policy != crate::relay::ReadPolicy::Public && event.is_none() {
        return Err(unauthorized());
    }
    if a.relay.read_policy == crate::relay::ReadPolicy::Admitted {
        let event = event.as_ref().ok_or_else(unauthorized)?;
        a.repo
            .account_permit(id(&event.pubkey).map_err(|_| unauthorized())?)
            .await
            .map_err(|error| account_error(error, "reader is not admitted"))?;
    }
    let storage = a.storage.clone().ok_or_else(unavailable)?;
    let (snapshot, mime) = match storage
        .download_public_optional(hash_id)
        .await
        .map_err(download_error)?
    {
        Some(value) => value,
        None => {
            let Some(event) = event.as_ref() else {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({"error":"blob_unavailable"})),
                ));
            };
            let permit = publisher(&a, event).await?;
            let snapshot = storage
                .download_for_publisher(&permit, hash_id)
                .await
                .map_err(publisher_read_error)?;
            let mime = snapshot.mime().to_owned();
            (snapshot, mime)
        }
    };
    let length = snapshot.len();
    let range = if method != axum::http::Method::GET || headers.contains_key("if-range") {
        None
    } else {
        headers.get("range").and_then(|h| h.to_str().ok())
    };
    let range = match crate::blossom::range(range, length) {
        Ok(v) => v,
        Err(_) => {
            return Ok((
                StatusCode::RANGE_NOT_SATISFIABLE,
                [("content-range", format!("bytes */{length}"))],
            )
                .into_response());
        }
    };
    let mut response = snapshot.response(range, &mime, true);
    if let Some((start, end)) = range {
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
        response.headers_mut().insert("content-range", format!("bytes {start}-{end}/{length}").parse().unwrap());
    }
    let headers = response.headers_mut();
    headers.insert("content-type", mime.parse().map_err(|_| unavailable())?);
    headers.insert(
        "content-length",
        (range.map_or(length, |(s, e)| e - s + 1)).into(),
    );
    headers.insert("accept-ranges", "bytes".parse().unwrap());
    headers.insert("cache-control", "private, no-store".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    Ok(response)
}
pub(super) async fn delete(
    State(a): State<Shared>,
    Path(path): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let hash = crate::blossom::path_hash(&path).map_err(|_| bad("invalid blob path"))?;
    let hash_id = id(hash).map_err(|_| bad("invalid hash"))?;
    let event = authorization(&a, &headers, "delete", hash).await?;
    let authority = publisher(&a, &event).await?;
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 0),
    )
    .await
    .map_err(|_| bad("delete body timeout"))?
    .map_err(|_| bad("delete body must be empty"))?;
    if !body.is_empty() {
        return Err(bad("delete body must be empty"));
    }
    authorization(&a, &headers, "delete", hash).await?;
    a.storage
        .as_ref()
        .ok_or_else(unavailable)?
        .delete_staged(
            crate::staged_deletion::DeleteAuthority::Publisher(authority),
            hash_id,
        )
        .await
        .map_err(|e| {
            let mut error = staged_delete_error(e);
            if error.0 == StatusCode::CONFLICT {
                error.0 = StatusCode::FORBIDDEN;
            }
            error
        })?;
    Ok(Json(json!({"sha256":hash,"deleted":true})))
}
pub(super) async fn cors(
    State(a): State<Shared>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or("");
    // Blossom and NIP11 mandate wildcard discovery/blob interoperability.
    let standard = matches!(route, "/" | "/upload" | "/{blob}");
    let origin = request
        .headers()
        .get("origin")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    let wildcard = standard || a.cors_origins.iter().any(|o| o == "*");
    let allowed = wildcard || origin.as_ref().is_some_and(|o| a.cors_origins.contains(o));
    if origin.is_some() && !allowed {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"origin_not_allowed"})),
        )
            .into_response();
    }
    let mut response = if request.method() == axum::http::Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    if allowed {
        headers.insert(
            "access-control-allow-origin",
            if wildcard {
                "*".parse().unwrap()
            } else {
                origin.unwrap().parse().unwrap()
            },
        );
        headers.insert(
            "access-control-allow-headers",
            "Authorization, Content-Type, X-SHA-256, X-Content-Type, X-Content-Length, Range, *"
                .parse()
                .unwrap(),
        );
        headers.insert(
            "access-control-allow-methods",
            "GET, HEAD, PUT, POST, DELETE, OPTIONS".parse().unwrap(),
        );
        headers.insert(
            "access-control-expose-headers",
            format!(
                "Content-Length, Content-Range, Accept-Ranges, X-Reason, {}",
                crate::service_proof::HEADER
            )
            .parse()
            .unwrap(),
        );
    }
    if !standard {
        headers.append("vary", "Origin".parse().unwrap());
    }
    response
}
