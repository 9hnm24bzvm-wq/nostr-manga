//! Bounded peer responses: never buffer the whole response before checking its budget.
pub const JSON_LIMIT: usize = 2_000_000;
pub const MANIFEST_LIMIT: usize = 1_000_000;
pub const ORIGINAL_LIMIT: usize = 16 * 1024 * 1024;

/// A failed transport may be retried; complete but invalid/budget-breaking
/// responses must still fail verification. Never infer this distinction from text.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("peer response body interrupted")]
    Transport(#[from] reqwest::Error),
    #[error("{0}")]
    Invalid(&'static str),
}

pub async fn read_bounded(
    mut response: reqwest::Response,
    limit: usize,
    expected: Option<u64>,
) -> Result<Vec<u8>, ReadError> {
    if expected.is_some_and(|n| n > limit as u64) {
        return Err(ReadError::Invalid("declared response size exceeds budget"));
    }
    if let Some(length) = response.content_length() {
        if length > limit as u64 {
            return Err(ReadError::Invalid("peer response size budget"));
        }
        if expected.is_some_and(|n| n != length) {
            return Err(ReadError::Invalid(
                "peer content length differs from manifest",
            ));
        }
    }
    let cap = expected.map_or(limit, |n| n as usize);
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > cap.saturating_sub(data.len()) {
            return Err(ReadError::Invalid("peer response streaming budget"));
        }
        data.extend_from_slice(&chunk);
    }
    if expected.is_some_and(|n| n != data.len() as u64) {
        return Err(ReadError::Invalid("peer body size differs from manifest"));
    }
    Ok(data)
}
