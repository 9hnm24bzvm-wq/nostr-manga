//! Bounded originals owned by this instance. User-controlled names never form paths.
use crate::error::{ApiError, ApiResult};
use axum::extract::multipart::Field;
use image::{AnimationDecoder, ImageDecoder, ImageFormat, ImageReader, Limits};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
    sync::Semaphore,
};

pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SESSION_BYTES: i64 = 500 * 1024 * 1024;
pub const MAX_FILES: usize = 500;
pub const MAX_PIXELS: u64 = 20_000_000;
pub const MAX_DIMENSION: u32 = 20_000;
const MAX_DECODE_BYTES: u64 = 96 * 1024 * 1024;

pub fn short_id() -> String {
    use rand::{Rng, distributions::Alphanumeric};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}
fn key() -> String {
    let mut value = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut value);
    hex::encode(value)
}
pub fn valid_id(value: &str) -> bool {
    value.len() == 10 && value.bytes().all(|b| b.is_ascii_alphanumeric())
}
pub fn valid_key(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn object_path(root: &Path, value: &str) -> ApiResult<PathBuf> {
    if !valid_key(value) {
        return Err(ApiError::bad_request("Invalid media key"));
    }
    Ok(root.join("objects").join(&value[..2]).join(value))
}
fn io_error(_: std::io::Error) -> ApiError {
    ApiError::unavailable()
}

/// Drop cleans interrupted streams and failed validation/database transactions.
/// Only persisted objects explicitly disarm the cleanup guard.
#[derive(Debug)]
pub struct StoredImage {
    pub key: String,
    pub digest: String,
    pub bytes: i64,
    pub width: i32,
    pub height: i32,
    pub mime: &'static str,
    pub ext: &'static str,
    path: PathBuf,
    retained: bool,
    pending: Option<PathBuf>,
    journal_lease: Option<std::fs::File>,
}
/// Persist directory entries on the supported local filesystem, not only file
/// contents. Failure is propagated before a database reference may commit.
pub(crate) async fn sync_directory(path: &Path) -> ApiResult<()> {
    fs::File::open(path)
        .await
        .map_err(io_error)?
        .sync_all()
        .await
        .map_err(io_error)
}
pub(crate) async fn ensure_directory(path: &Path) -> ApiResult<()> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().map_err(io_error)?.join(path)
    };
    let mut cursor = absolute.clone();
    let mut sync = Vec::new();
    loop {
        sync.push(cursor.clone());
        match fs::metadata(&cursor).await {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => return Err(ApiError::unavailable()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !cursor.pop() {
                    return Err(ApiError::unavailable());
                }
            }
            Err(error) => return Err(io_error(error)),
        }
    }
    fs::create_dir_all(&absolute).await.map_err(io_error)?;
    for directory in sync {
        sync_directory(&directory).await?;
    }
    Ok(())
}
/// A paused writer can resume after maintenance removed its pre-lock name.
/// Never continue writing an unlinked inode or attach an object without its
/// actual locked journal entry. Runtime storage requires local Unix semantics.
pub(crate) fn verify_named_lease(file: &std::fs::File, path: &Path) -> ApiResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let held = file.metadata().map_err(io_error)?;
        let named = std::fs::symlink_metadata(path).map_err(io_error)?;
        if held.is_file()
            && named.is_file()
            && held.dev() == named.dev()
            && held.ino() == named.ino()
        {
            return Ok(());
        }
    }
    #[cfg(not(unix))]
    let _ = (file, path);
    Err(ApiError::unavailable())
}
impl StoredImage {
    /// This key-only intent is durable BEFORE any final object entry exists.
    /// Its held OS lock survives async suspension and is released by process
    /// death; recovery cannot mistake a live staged object for an orphan.
    async fn prepare_write(&mut self, root: &Path) -> ApiResult<()> {
        if self.journal_lease.is_some() {
            return Ok(());
        }
        let directory = root.join("pending");
        ensure_directory(&directory).await?;
        let marker = directory.join(&self.key);
        self.pending = Some(marker.clone());
        let journal = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&marker)
            .await
            .map_err(io_error)?;
        let journal = journal.into_std().await;
        journal.try_lock().map_err(|_| ApiError::unavailable())?;
        verify_named_lease(&journal, &marker)?;
        let mut output = fs::File::from_std(journal.try_clone().map_err(io_error)?);
        self.journal_lease = Some(journal);
        output
            .write_all(self.key.as_bytes())
            .await
            .map_err(io_error)?;
        output.sync_all().await.map_err(io_error)?;
        sync_directory(&directory).await?;
        Ok(())
    }
    /// An uncertain COMMIT retains this already-durable original and intent.
    pub async fn protect_commit(&mut self, root: &Path) -> ApiResult<()> {
        self.prepare_write(root).await?;
        let file = fs::File::open(&self.path).await.map_err(io_error)?;
        let metadata = file.metadata().await.map_err(io_error)?;
        if !metadata.is_file() || metadata.len() != self.bytes as u64 {
            return Err(ApiError::unavailable());
        }
        file.sync_all().await.map_err(io_error)?;
        sync_directory(self.path.parent().ok_or_else(ApiError::unavailable)?).await?;
        self.retained = true;
        Ok(())
    }
    pub async fn confirm_persisted(&mut self) {
        if let Some(path) = self.pending.take() {
            if fs::remove_file(&path).await.is_ok() {
                // A leftover marker after a power interruption is safe: fresh
                // database references prevent recovery from removing its bytes.
                if let Some(directory) = path.parent() {
                    let _ = sync_directory(directory).await;
                }
            }
        }
        self.journal_lease.take();
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
/// The same durable staging seam for trusted local derivative encoders. Actual
/// header dimensions/type are checked before a final generated object is written.
pub(crate) async fn store_encoded(
    root: &Path,
    bytes: &[u8],
    mime: &'static str,
    ext: &'static str,
    width: i32,
    height: i32,
) -> ApiResult<StoredImage> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ApiError::bad_request("Generated media exceeds byte budget"));
    }
    validate_dimensions(width as u32, height as u32)?;
    let expected = match (mime, ext) {
        ("image/webp", "webp") => ImageFormat::WebP,
        ("image/jpeg", "jpg") => ImageFormat::Jpeg,
        _ => return Err(ApiError::bad_request("Unsupported generated media format")),
    };
    let reader = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(io_error)?;
    if reader.format() != Some(expected)
        || reader
            .into_dimensions()
            .map_err(|_| ApiError::unavailable())?
            != (width as u32, height as u32)
    {
        return Err(ApiError::unavailable());
    }
    let key = key();
    let path = object_path(root, &key)?;
    ensure_directory(path.parent().expect("object parent")).await?;
    let mut image = StoredImage {
        key,
        digest: hex::encode(Sha256::digest(bytes)),
        bytes: bytes.len() as i64,
        width,
        height,
        mime,
        ext,
        path,
        retained: false,
        pending: None,
        journal_lease: None,
    };
    image.prepare_write(root).await?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&image.path)
        .await
        .map_err(io_error)?;
    output.write_all(bytes).await.map_err(io_error)?;
    output.sync_all().await.map_err(io_error)?;
    sync_directory(image.path.parent().expect("object parent")).await?;
    Ok(image)
}

impl Drop for StoredImage {
    fn drop(&mut self) {
        if !self.retained {
            let _ = std::fs::remove_file(&self.path);
            if let Some(directory) = self.path.parent() {
                let _ = std::fs::File::open(directory).and_then(|file| file.sync_all());
            }
            // Keep the durable intent even after best-effort unlink: a canceled
            // Tokio filesystem job can finish later, and Drop cannot certify
            // power-loss durability. Bounded reconciliation clears it only
            // after the live lease is gone and fresh DB references are checked.
        }
    }
}

struct TempGuard(PathBuf, Option<std::fs::File>);
impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub async fn receive(root: &Path, field: Field<'_>, covers: bool) -> ApiResult<(StoredImage, Vec<(String, StoredImage)>)> {
    receive_with_derivatives(root, field, covers, true).await
}
/// Public cover intake preserves the exact original and all existing image
/// validation, without invoking an encoder or creating derivative objects.
pub(crate) async fn receive_cover_original(root: &Path, field: Field<'_>) -> ApiResult<StoredImage> {
    let (original, derivatives) = receive_with_derivatives(root, field, true, false).await?;
    debug_assert!(derivatives.is_empty());
    Ok(original)
}
async fn receive_with_derivatives(
    root: &Path,
    mut field: Field<'_>,
    covers: bool,
    derivatives: bool,
) -> ApiResult<(StoredImage, Vec<(String, StoredImage)>)> {
    let original_name = field.file_name().unwrap_or("").to_owned();
    let declared = field.content_type().map(str::to_owned);
    let extension = original_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    if !matches!(
        extension.as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "gif")
    ) {
        return Err(ApiError::bad_request(
            "Only JPEG, PNG, WebP and GIF images are supported",
        ));
    }
    if covers && extension.as_deref() == Some("gif") {
        return Err(ApiError::bad_request("GIF covers are not supported"));
    }
    if let Some(mime) = declared.as_deref() {
        if !matches!(
            mime,
            "image/jpeg"
                | "image/jpg"
                | "image/png"
                | "image/webp"
                | "image/gif"
                | "application/octet-stream"
        ) {
            return Err(ApiError::bad_request("Unsupported image media type"));
        }
    }
    let temporary = root.join("temporary");
    ensure_directory(&temporary).await?;
    let mut tmp = TempGuard(temporary.join(key()), None);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp.0)
        .await
        .map_err(io_error)?;
    let lease = file.into_std().await;
    lease.try_lock().map_err(|_| ApiError::unavailable())?;
    verify_named_lease(&lease, &tmp.0)?;
    file = fs::File::from_std(lease.try_clone().map_err(io_error)?);
    tmp.1 = Some(lease);
    let mut size = 0u64;
    let mut hash = Sha256::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    while let Some(chunk) = tokio::time::timeout_at(deadline, field.chunk())
        .await
        .map_err(|_| ApiError::bad_request("Image upload timed out"))?
        .map_err(|_| ApiError::bad_request("Invalid multipart image"))?
    {
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| ApiError::bad_request("Image is too large"))?;
        if size > MAX_FILE_BYTES {
            return Err(ApiError::new(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "image_too_large",
                "Image exceeds 32 MiB",
            ));
        }
        hash.update(&chunk);
        file.write_all(&chunk).await.map_err(io_error)?;
    }
    if size == 0 {
        return Err(ApiError::bad_request("Image is empty"));
    }
    file.sync_all().await.map_err(io_error)?;
    drop(file);
    let (width, height, format, variants) = inspect_path_with_derivatives(tmp.0.clone(), covers, derivatives).await?;
    let (mime, ext) = match format {
        ImageFormat::Jpeg => ("image/jpeg", "jpg"),
        ImageFormat::Png => ("image/png", "png"),
        ImageFormat::WebP => ("image/webp", "webp"),
        ImageFormat::Gif => ("image/gif", "gif"),
        _ => return Err(ApiError::bad_request("Unsupported image format")),
    };
    if declared.as_deref().is_some_and(|m| {
        m != "application/octet-stream" && m != mime && !(m == "image/jpg" && mime == "image/jpeg")
    }) {
        return Err(ApiError::bad_request(
            "Image header does not match declared media type",
        ));
    }
    let name_format = match extension.as_deref() {
        Some("jpg" | "jpeg") => ImageFormat::Jpeg,
        Some("png") => ImageFormat::Png,
        Some("webp") => ImageFormat::WebP,
        Some("gif") => ImageFormat::Gif,
        _ => unreachable!(),
    };
    if name_format != format {
        return Err(ApiError::bad_request(
            "Image header does not match filename extension",
        ));
    }
    let image_key = key();
    let final_path = object_path(root, &image_key)?;
    ensure_directory(final_path.parent().expect("object parent")).await?;
    let mut original = StoredImage {
        key: image_key,
        digest: hex::encode(hash.finalize()),
        bytes: size as i64,
        width: width as i32,
        height: height as i32,
        mime,
        ext,
        path: final_path,
        retained: false,
        pending: None,
        journal_lease: None,
    };
    original.prepare_write(root).await?;
    fs::rename(&tmp.0, &original.path).await.map_err(io_error)?;
    sync_directory(original.path.parent().expect("object parent")).await?;
    sync_directory(tmp.0.parent().expect("temporary parent")).await?;
    let mut result = Vec::new();
    for (variant, encoded, w, h) in variants {
        let variant_key = key();
        let path = object_path(root, &variant_key)?;
        ensure_directory(path.parent().expect("object parent")).await?;
        let mut guard = StoredImage {
            key: variant_key,
            digest: hex::encode(Sha256::digest(&encoded)),
            bytes: encoded.len() as i64,
            width: w as i32,
            height: h as i32,
            mime: if variant == "optimized-jpg" {
                "image/jpeg"
            } else {
                "image/webp"
            },
            ext: if variant == "optimized-jpg" {
                "jpg"
            } else {
                "webp"
            },
            path,
            retained: false,
            pending: None,
            journal_lease: None,
        };
        guard.prepare_write(root).await?;
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&guard.path)
            .await
            .map_err(io_error)?;
        out.write_all(&encoded).await.map_err(io_error)?;
        out.sync_all().await.map_err(io_error)?;
        sync_directory(guard.path.parent().expect("object parent")).await?;
        // Ownership remains guarded until the calling transaction commits.
        guard.retained = false;
        result.push((variant, guard));
    }
    Ok((original, result))
}
pub(crate) async fn inspect_path(
    path: PathBuf,
    covers: bool,
) -> ApiResult<(u32, u32, ImageFormat, Variants)> {
    inspect_path_with_derivatives(path, covers, true).await
}
async fn inspect_path_with_derivatives(path: PathBuf, covers: bool, derivatives: bool) -> ApiResult<(u32, u32, ImageFormat, Variants)> {
    let permit = decode_slots()
        .try_acquire_owned()
        .map_err(|_| ApiError::busy())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        validate_image(&path, covers, derivatives)
    })
    .await
    .map_err(|_| ApiError::unavailable())?
}
fn decode_slots() -> Arc<Semaphore> {
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SLOTS.get_or_init(|| Arc::new(Semaphore::new(2))).clone()
}
type Variants = Vec<(String, Vec<u8>, u32, u32)>;
fn validate_image(path: &Path, covers: bool, derivatives: bool) -> ApiResult<(u32, u32, ImageFormat, Variants)> {
    let reader = ImageReader::open(path)
        .map_err(io_error)?
        .with_guessed_format()
        .map_err(io_error)?;
    let format = reader
        .format()
        .ok_or_else(|| ApiError::bad_request("Image header is invalid"))?;
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP | ImageFormat::Gif
    ) {
        return Err(ApiError::bad_request("Unsupported image format"));
    }
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| ApiError::bad_request("Image dimensions are invalid"))?;
    validate_dimensions(width, height)?;
    if format == ImageFormat::Gif {
        if covers {
            return Err(ApiError::bad_request("GIF covers are not supported"));
        }
        let input = std::io::BufReader::new(std::fs::File::open(path).map_err(io_error)?);
        let mut decoder = image::codecs::gif::GifDecoder::new(input)
            .map_err(|_| ApiError::bad_request("GIF header is invalid"))?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        decoder
            .set_limits(limits)
            .map_err(|_| ApiError::bad_request("GIF exceeds decode budget"))?;
        let mut pixels = 0u64;
        let mut count = 0usize;
        for frame in decoder.into_frames() {
            let frame = frame.map_err(|_| ApiError::bad_request("GIF frame is corrupt"))?;
            count += 1;
            let frame = frame.buffer();
            validate_dimensions(frame.width(), frame.height())?;
            pixels = pixels.saturating_add(u64::from(frame.width()) * u64::from(frame.height()));
            if count > 100 || pixels > MAX_PIXELS {
                return Err(ApiError::bad_request(
                    "GIF animation exceeds frame or decoded-pixel budget",
                ));
            }
        }
        if count == 0 {
            return Err(ApiError::bad_request("GIF has no image frames"));
        }
        return Ok((width, height, format, Vec::new()));
    }
    let mut reader = ImageReader::open(path)
        .map_err(io_error)?
        .with_guessed_format()
        .map_err(io_error)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| ApiError::bad_request("Image data is corrupt or exceeds the decode budget"))?;
    let mut variants = Vec::new();
    if covers && derivatives {
        for target in [256, 512] {
            let thumbnail =
                image.thumbnail(target.min(image.width()), (target * 4).min(image.height()));
            let rgba = thumbnail.to_rgba8();
            let mut out = Vec::new();
            image::codecs::webp::WebPEncoder::new_lossless(&mut out)
                .encode(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|_| ApiError::unavailable())?;
            if out.len() as u64 > MAX_FILE_BYTES {
                return Err(ApiError::bad_request(
                    "Generated cover exceeds storage budget",
                ));
            }
            variants.push((target.to_string(), out, rgba.width(), rgba.height()));
        }
    }
    if !covers && derivatives {
        if let Some(derivative) = super::derivatives::encode(&image, path)? {
            variants.push(derivative);
        }
    }
    Ok((width, height, format, variants))
}
pub fn validate_dimensions(width: u32, height: u32) -> ApiResult<()> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        Err(ApiError::bad_request(
            "Image dimensions exceed the pixel budget",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_only_use_generated_fixed_width_keys() {
        let root = Path::new("/storage");
        for key in [
            "../private",
            "/etc/passwd",
            "a/b",
            "AA",
            &"a".repeat(63),
            &"g".repeat(64),
        ] {
            assert!(object_path(root, key).is_err());
        }
        assert_eq!(
            object_path(root, &"a".repeat(64)).unwrap(),
            root.join("objects/aa").join("a".repeat(64))
        );
        assert!(valid_id(&short_id()));
    }
    #[test]
    fn pixel_and_dimension_limits_are_checked_without_overflow() {
        assert!(validate_dimensions(1, 1).is_ok());
        assert!(validate_dimensions(20000, 1000).is_ok());
        for (w, h) in [
            (0, 1),
            (1, 0),
            (20001, 1),
            (5000, 5000),
            (u32::MAX, u32::MAX),
        ] {
            assert!(validate_dimensions(w, h).is_err());
        }
    }
    #[test]
    fn genuine_image_decode_rejects_truncated_headers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image");
        std::fs::write(&path, b"\x89PNG\r\n\x1a\n").unwrap();
        assert!(validate_image(&path, false, true).is_err());
        let image = image::RgbImage::from_pixel(4, 3, image::Rgb([42, 0, 42]));
        image.save_with_format(&path, ImageFormat::Png).unwrap();
        let (w, h, fmt, variants) = validate_image(&path, true, true).unwrap();
        assert_eq!((w, h, fmt), (4, 3, ImageFormat::Png));
        let original = std::fs::read(&path).unwrap();
        let (ow, oh, ofmt, none) = validate_image(&path, true, false).unwrap();
        assert_eq!((ow, oh, ofmt), (4, 3, ImageFormat::Png));
        assert!(none.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(variants.len(), 2);
        for (_, bytes, _, _) in variants {
            assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::WebP);
        }
    }
    #[test]
    fn interrupted_or_uncommitted_storage_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("uncommitted");
        std::fs::write(&path, b"x").unwrap();
        drop(StoredImage {
            key: "a".repeat(64),
            digest: "a".repeat(64),
            bytes: 1,
            width: 1,
            height: 1,
            mime: "image/png",
            ext: "png",
            path: path.clone(),
            retained: false,
            pending: None,
            journal_lease: None,
        });
        assert!(!path.exists());
    }
    #[test]
    fn gif_all_frames_are_decoded_with_aggregate_pixel_budget() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image");
        let mut encoder =
            image::codecs::gif::GifEncoder::new(std::fs::File::create(&path).unwrap());
        for _ in 0..2 {
            encoder
                .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                    3,
                    4,
                    image::Rgba([42, 0, 42, 255]),
                )))
                .unwrap();
        }
        drop(encoder);
        let (width, height, format, variants) = validate_image(&path, false, true).unwrap();
        assert_eq!((width, height, format), (3, 4, ImageFormat::Gif));
        assert!(variants.is_empty());
        assert!(validate_image(&path, true, true).is_err());
    }

    #[tokio::test]
    async fn prewrite_intent_precedes_object_and_holds_live_recovery_lease() {
        let root = tempfile::tempdir().unwrap();
        let key = "b".repeat(64);
        let path = object_path(root.path(), &key).unwrap();
        ensure_directory(path.parent().unwrap()).await.unwrap();
        let mut staged = StoredImage {
            key: key.clone(),
            digest: key.clone(),
            bytes: 8,
            width: 1,
            height: 1,
            mime: "image/png",
            ext: "png",
            path: path.clone(),
            retained: false,
            pending: None,
            journal_lease: None,
        };
        staged.prepare_write(root.path()).await.unwrap();
        assert!(
            !path.exists(),
            "No final object exists before the durable intent"
        );
        let marker = root.path().join("pending").join(&key);
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), key);
        let contender = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&marker)
            .unwrap();
        assert!(
            contender.try_lock().is_err(),
            "Recovery cannot claim a live staged object"
        );
        drop(staged);
        assert!(
            marker.exists(),
            "Drop does not erase the durable uncertain-write intent"
        );
        assert!(
            contender.try_lock().is_ok(),
            "Process/owner lease release permits recovery"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_writer_cannot_resume_using_a_reclaimed_or_replaced_lease_name() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("lease");
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        held.try_lock().unwrap();
        assert!(verify_named_lease(&held, &path).is_ok());
        std::fs::remove_file(&path).unwrap();
        assert!(verify_named_lease(&held, &path).is_err());
        std::fs::write(&path, b"different inode").unwrap();
        assert!(verify_named_lease(&held, &path).is_err());
    }

    #[tokio::test]
    async fn uncertain_commit_retains_original_and_a_key_only_recovery_journal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("original");
        std::fs::write(&path, b"original").unwrap();
        let key = "a".repeat(64);
        let marker = dir.path().join("pending").join(&key);
        let mut image = StoredImage {
            key: key.clone(),
            digest: key.clone(),
            bytes: 8,
            width: 1,
            height: 1,
            mime: "image/png",
            ext: "png",
            path: path.clone(),
            retained: false,
            pending: None,
            journal_lease: None,
        };
        image.protect_commit(dir.path()).await.unwrap();
        drop(image);
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(marker).unwrap(), key);
    }
}
