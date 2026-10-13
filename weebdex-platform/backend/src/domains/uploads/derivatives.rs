//! Optional genuinely smaller reader images. Originals and protocol source remain unchanged.
//! This executes within the storage decoder's two owned blocking-job permits.
use crate::error::{ApiError, ApiResult};
use image::{DynamicImage, ExtendedColorType};
use std::path::Path;

pub(crate) fn encode(
    image: &DynamicImage,
    original: &Path,
) -> ApiResult<Option<(String, Vec<u8>, u32, u32)>> {
    // At most 3.84M derivative pixels; no upscaling. Animation is intentionally
    // unsupported here: storage validates and retains every original GIF frame.
    let resized = image.thumbnail(1280.min(image.width()), 3000.min(image.height()));
    let mut bytes = Vec::new();
    let (variant, width, height) = if image.color().has_alpha() {
        let rgba = resized.to_rgba8();
        image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
            .encode(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|_| ApiError::unavailable())?;
        ("optimized-webp", rgba.width(), rgba.height())
    } else {
        let rgb = resized.to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 72)
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )
            .map_err(|_| ApiError::unavailable())?;
        ("optimized-jpg", rgb.width(), rgb.height())
    };
    let original_bytes = std::fs::metadata(original)
        .map_err(|_| ApiError::unavailable())?
        .len();
    // A larger re-encoding must never masquerade as a data-saving capability.
    if bytes.is_empty()
        || bytes.len() as u64 >= original_bytes
        || bytes.len() as u64 > super::storage::MAX_FILE_BYTES
    {
        return Ok(None);
    }
    Ok(Some((variant.to_owned(), bytes, width, height)))
}
