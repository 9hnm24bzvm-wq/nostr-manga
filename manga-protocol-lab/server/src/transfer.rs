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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn response(wire: &'static [u8]) -> reqwest::Response {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = socket.read(&mut request).await;
            let _ = socket.write_all(wire).await;
        });
        reqwest::Client::new()
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn rejects_oversized_header_before_body() {
        let r = response(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000000\r\n\r\n").await;
        assert!(matches!(
            read_bounded(r, 10, None).await,
            Err(ReadError::Invalid("peer response size budget"))
        ));
    }
    #[tokio::test]
    async fn rejects_chunked_overflow() {
        let r = response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nabcd\r\n4\r\nefgh\r\n0\r\n\r\n").await;
        assert!(matches!(
            read_bounded(r, 7, None).await,
            Err(ReadError::Invalid("peer response streaming budget"))
        ));
    }
    #[tokio::test]
    async fn accepts_exact_chunked_body() {
        let r = response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nabcd\r\n0\r\n\r\n",
        )
        .await;
        assert_eq!(read_bounded(r, 10, Some(4)).await.unwrap(), b"abcd");
    }
    #[tokio::test]
    async fn rejects_header_manifest_mismatch() {
        let r = response(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nabcd").await;
        assert!(matches!(
            read_bounded(r, 10, Some(3)).await,
            Err(ReadError::Invalid(
                "peer content length differs from manifest"
            ))
        ));
    }
    #[tokio::test]
    async fn rejects_short_chunked_body() {
        let r =
            response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n")
                .await;
        assert!(matches!(
            read_bounded(r, 10, Some(4)).await,
            Err(ReadError::Invalid("peer body size differs from manifest"))
        ));
    }
    #[tokio::test]
    async fn declared_size_cannot_expand_local_budget() {
        let r = response(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").await;
        assert!(matches!(
            read_bounded(r, 10, Some(u64::MAX)).await,
            Err(ReadError::Invalid("declared response size exceeds budget"))
        ));
    }
}
