//! Real instance-local cover derivatives of verified self-hosted originals.
//! Canonical original descriptors and protocol storage are never rewritten.
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::body::to_bytes;
use image::{ExtendedColorType, ImageReader, Limits};
use manga_node::auth;
use std::{
    io::Cursor,
    sync::{Arc, OnceLock},
};

fn slots() -> Arc<tokio::sync::Semaphore> {
    static SLOTS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    SLOTS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
}
async fn original(state: &AppState, hash: &str, size: u64, mime: &str) -> ApiResult<Vec<u8>> {
    if size == 0 || size > 64 * 1024 * 1024 {
        return Err(ApiError::bad_request(
            "Source cover exceeds the original byte budget",
        ));
    }
    let bytes = if let Ok(runtime) = state.protocol.publication_runtime().await {
        let id = hex::decode(hash)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(ApiError::unavailable)?;
        let (snapshot, actual) = runtime
            .download_original(id)
            .await
            .map_err(|_| ApiError::unavailable())?
            .ok_or_else(|| ApiError::not_found("Self-hosted source cover is unavailable"))?;
        if snapshot.len() as u64 != size || actual != mime {
            return Err(ApiError::unavailable());
        }
        to_bytes(
            snapshot.response(None, mime, false).into_body(),
            size as usize,
        )
        .await
        .map_err(|_| ApiError::unavailable())?
        .to_vec()
    } else {
        if !state.config.protocol_origin_self_hosted {
            return Err(ApiError::unavailable());
        }
        let origin = state
            .config
            .protocol_origin
            .as_ref()
            .ok_or_else(ApiError::unavailable)?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| ApiError::unavailable())?;
        let mut response = client
            .get(format!("{}/originals/{hash}", origin.trim_end_matches('/')))
            .send()
            .await
            .map_err(|_| ApiError::unavailable())?;
        if !response.status().is_success() {
            return Err(ApiError::unavailable());
        }
        let mut bytes = Vec::with_capacity(size as usize);
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ApiError::unavailable())?
        {
            if (bytes.len() as u64).saturating_add(chunk.len() as u64) > size {
                return Err(ApiError::unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        bytes
    };
    // This verification is at derivative ingestion, never on every client GET.
    if bytes.len() as u64 != size
        || auth::hash(&bytes) != hash
        || manga_node::manifest::detected_mime(&bytes) != Some(mime)
    {
        return Err(ApiError::unavailable());
    }
    Ok(bytes)
}
fn encode(bytes: Vec<u8>) -> ApiResult<Vec<(String, Vec<u8>, i32, i32)>> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ApiError::unavailable())?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(192 * 1024 * 1024);
    limits.max_image_width = Some(40_000_000);
    limits.max_image_height = Some(40_000_000);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| {
        ApiError::bad_request("Source cover cannot be decoded within its bounded budget")
    })?;
    if u64::from(image.width()) * u64::from(image.height()) > 40_000_000 {
        return Err(ApiError::bad_request(
            "Source cover exceeds its original pixel budget",
        ));
    }
    let mut result = Vec::new();
    for target in [256u32, 512] {
        let rgba = image
            .thumbnail(target.min(image.width()), (target * 4).min(image.height()))
            .to_rgba8();
        let mut bytes = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
            .encode(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|_| ApiError::unavailable())?;
        result.push((
            target.to_string(),
            bytes,
            rgba.width() as i32,
            rgba.height() as i32,
        ));
    }
    Ok(result)
}
/// One cover per call; no remote peer fallback and no fake optimized filename.
pub async fn materialize_page(state: &AppState) -> ApiResult<usize> {
    let permit = slots().try_acquire_owned().map_err(|_| ApiError::busy())?;
    let row = {
        let client = state.db.client().await?;
        client.query_opt("SELECT s.cover_id,s.event_id,s.blob_hash,s.mime,s.size FROM platform.protocol_cover_sources s JOIN platform.catalog_covers c ON c.id=s.cover_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE NOT s.derivatives_ready AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND platform.protocol_source_is_visible(s.event_id) AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY s.checked_at,s.cover_id LIMIT 1",&[]).await?
    };
    let Some(row) = row else { return Ok(0) };
    let cover: String = row.get(0);
    let event: String = row.get(1);
    let hash: String = row.get(2);
    let mime: String = row.get(3);
    let size = row.get::<_, i64>(4) as u64;
    let bytes = match original(state, &hash, size, &mime).await {
        Ok(bytes) => bytes,
        Err(error) => {
            record_error(state, &cover).await;
            return Err(error);
        }
    };
    let variants = match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        encode(bytes)
    })
    .await
    {
        Ok(Ok(variants)) => variants,
        Ok(Err(error)) => {
            record_error(state, &cover).await;
            return Err(error);
        }
        Err(_) => {
            record_error(state, &cover).await;
            return Err(ApiError::unavailable());
        }
    };
    let mut files = Vec::new();
    let mut descriptors = Vec::new();
    for (variant, bytes, width, height) in variants {
        let image = crate::domains::uploads::storage::store_encoded(
            &state.config.uploads_dir,
            &bytes,
            "image/webp",
            "webp",
            width,
            height,
        )
        .await?;
        descriptors.push((
            variant,
            image.key.clone(),
            image.digest.clone(),
            image.bytes,
            width,
            height,
        ));
        files.push(image);
    }
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    tx.batch_execute("SET LOCAL lock_timeout='500ms'").await?;
    if tx.query_opt("SELECT s.cover_id FROM platform.protocol_cover_sources s JOIN platform.catalog_covers c ON c.id=s.cover_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE s.cover_id=$1 AND s.event_id=$2 AND NOT s.derivatives_ready AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND platform.protocol_source_is_visible(s.event_id) AND platform.protocol_source_is_visible(m.protocol_series_id) FOR UPDATE OF s FOR SHARE OF c,m",&[&cover,&event]).await?.is_none(){return Ok(0);}
    for (variant, key, digest, bytes, width, height) in descriptors {
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,724194))",
            &[&key],
        )
        .await?;
        tx.execute("INSERT INTO platform.media_objects(id,digest,storage_key,mime,extension,bytes,width,height) VALUES($1,$2,$1,'image/webp','webp',$3,$4,$5)",&[&key,&digest,&bytes,&width,&height]).await?;
        tx.execute("INSERT INTO platform.cover_media(cover_id,variant,object_id) VALUES($1,$2,$3) ON CONFLICT(cover_id,variant) DO NOTHING",&[&cover,&variant,&key]).await?;
    }
    tx.execute("UPDATE platform.protocol_cover_sources SET derivatives_ready=TRUE,derivative_error=NULL,checked_at=clock_timestamp() WHERE cover_id=$1",&[&cover]).await?;
    // Unknown COMMIT outcomes are reconciled by the existing pending-file journal.
    for file in &mut files {
        file.protect_commit(&state.config.uploads_dir).await?;
    }
    tx.commit().await?;
    for file in &mut files {
        file.confirm_persisted().await;
    }
    Ok(1)
}
async fn record_error(state: &AppState, cover: &str) {
    if let Ok(client) = state.db.client().await {
        let _=client.execute("UPDATE platform.protocol_cover_sources SET derivative_error='Self-hosted cover materialization unavailable',checked_at=clock_timestamp() WHERE cover_id=$1",&[&cover]).await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_variants_are_real_webp_with_bounded_dimensions() {
        let image = image::DynamicImage::new_rgba8(10, 20);
        let mut input = Cursor::new(Vec::new());
        image.write_to(&mut input, image::ImageFormat::Png).unwrap();
        let output = encode(input.into_inner()).unwrap();
        assert_eq!(output.len(), 2);
        for (_, bytes, w, h) in output {
            assert_eq!(
                image::guess_format(&bytes).unwrap(),
                image::ImageFormat::WebP
            );
            assert_eq!((w, h), (10, 20));
        }
    }
    #[tokio::test]
    async fn source_cover_variants_use_the_shared_durable_write_lease() {
        let root = tempfile::tempdir().unwrap();
        let image = image::DynamicImage::new_rgba8(10, 20);
        let mut input = Cursor::new(Vec::new());
        image.write_to(&mut input, image::ImageFormat::Png).unwrap();
        for (_, bytes, width, height) in encode(input.into_inner()).unwrap() {
            let mut staged = crate::domains::uploads::storage::store_encoded(
                root.path(),
                &bytes,
                "image/webp",
                "webp",
                width,
                height,
            )
            .await
            .unwrap();
            assert_eq!(std::fs::read(staged.path()).unwrap(), bytes);
            let marker = root.path().join("pending").join(&staged.key);
            assert_eq!(std::fs::read_to_string(&marker).unwrap(), staged.key);
            let contender = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&marker)
                .unwrap();
            assert!(
                contender.try_lock().is_err(),
                "Recovery cannot claim a live cover write"
            );
            staged.protect_commit(root.path()).await.unwrap();
            let original = staged.path().to_owned();
            drop(staged);
            assert!(
                original.exists(),
                "An unknown reference-commit outcome retains bytes"
            );
            assert!(
                marker.exists(),
                "An unknown reference-commit outcome retains its journal"
            );
            assert!(contender.try_lock().is_ok());
        }
    }
}
