//! RECONSTRUCTED, UNVERIFIED. Native ext may already begin with exactly one dot.
use super::errors::{PublicApiError, PublicApiResult};
pub fn cover_filename(native_id: &str, native_ext: &str) -> PublicApiResult<String> {
    if native_id.len() != 10 || !native_id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(PublicApiError::internal());
    }
    let ext = native_ext.strip_prefix('.').unwrap_or(native_ext);
    if !matches!(ext, "jpg" | "png" | "webp" | "gif") {
        return Err(PublicApiError::internal());
    }
    Ok(format!("{native_id}.{ext}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_native_extension_produces_one_dot() {
        assert_eq!(
            cover_filename("Cover00001", ".webp").unwrap(),
            "Cover00001.webp"
        );
        assert_eq!(
            cover_filename("Cover00001", "webp").unwrap(),
            "Cover00001.webp"
        );
        for invalid in ["..webp", ".WEBP", "webp/../../etc/passwd", ""] {
            assert!(cover_filename("Cover00001", invalid).is_err());
        }
    }
}
