//! Inline discussion-image proxy only. Chapter readers always use instance-owned originals.
//! Every redirect is independently vetted and DNS-pinned, with no ambient proxy/credentials.
use super::{media, storage};
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::Response,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt,
    sync::Semaphore,
};
const MAX_PROXY_BYTES: u64 = 8 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProxyQuery {
    image: String,
}
pub(crate) struct ProxyFile(PathBuf, Option<std::fs::File>);
impl Drop for ProxyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub async fn proxy(
    State(state): State<AppState>,
    Query(query): Query<ProxyQuery>,
    headers: HeaderMap,
    method: Method,
) -> ApiResult<Response> {
    let mut permits = media::permits(&state).await?;
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    permits.proxy = Some(
        SLOTS
            .get_or_init(|| Arc::new(Semaphore::new(4)))
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError::busy())?,
    );
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        fetch(&query.image, &state.config.uploads_dir),
    )
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "proxy_timeout",
            "Image retrieval timed out",
        )
    })??;
    let (file, digest, mime, length, guard) = result;
    permits.cleanup = Some(guard);
    // Proxied content has a bounded temporary lifetime and isn't a federation/cache authority.
    media::file_response(
        file, &digest, &mime, length, &headers, &method, true, permits,
    )
    .await
}
fn target(value: &str) -> ApiResult<url::Url> {
    if value.len() > 2048 {
        return Err(ApiError::bad_request("Image URL is too long"));
    }
    let url = url::Url::parse(value).map_err(|_| ApiError::bad_request("Invalid image URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(if url.scheme() == "https" { 443 } else { 80 })
    {
        return Err(ApiError::bad_request(
            "Image URL destination is not allowed",
        ));
    }
    if let Some(url::Host::Domain(domain)) = url.host() {
        if domain == "localhost"
            || [".localhost", ".local", ".internal", ".test", ".invalid"]
                .iter()
                .any(|suffix| domain.ends_with(suffix))
        {
            return Err(ApiError::bad_request("Image URL destination is not public"));
        }
    }
    match url.host() {
        Some(url::Host::Ipv4(ip)) if !public_address(ip.into()) => {
            return Err(ApiError::bad_request("Image URL destination is not public"));
        }
        Some(url::Host::Ipv6(ip)) if !public_address(ip.into()) => {
            return Err(ApiError::bad_request("Image URL destination is not public"));
        }
        _ => {}
    }
    Ok(url)
}
async fn addresses(url: &url::Url) -> ApiResult<Vec<SocketAddr>> {
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ApiError::bad_request("Image URL port is invalid"))?;
    let values = match url
        .host()
        .ok_or_else(|| ApiError::bad_request("Image URL host is missing"))?
    {
        url::Host::Ipv4(ip) => vec![SocketAddr::new(ip.into(), port)],
        url::Host::Ipv6(ip) => vec![SocketAddr::new(ip.into(), port)],
        url::Host::Domain(host) => tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| ApiError::unavailable())?
        .map_err(|_| ApiError::unavailable())?
        .take(17)
        .collect(),
    };
    if values.is_empty()
        || values.len() > 16
        || values.iter().any(|address| !public_address(address.ip()))
    {
        return Err(ApiError::bad_request(
            "Image destination did not pass public address validation",
        ));
    }
    Ok(values)
}
async fn fetch(value: &str, root: &Path) -> ApiResult<(File, String, String, u64, ProxyFile)> {
    let mut url = target(value)?;
    let mut response = None;
    for redirect in 0..=3 {
        let ips = addresses(&url).await?;
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(0);
        if let Some(url::Host::Domain(host)) = url.host() {
            builder = builder.resolve_to_addrs(host, &ips);
        }
        let client = builder.build().map_err(|_| ApiError::unavailable())?;
        let result = client
            .get(url.clone())
            .header("accept", "image/jpeg,image/png,image/webp,image/gif")
            .send()
            .await
            .map_err(|_| ApiError::unavailable())?;
        if result.status().is_redirection() {
            if redirect == 3 {
                return Err(ApiError::bad_request("Image redirect limit exceeded"));
            }
            let location = result
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|h| h.to_str().ok())
                .ok_or_else(|| ApiError::bad_request("Invalid image redirect"))?;
            url = target(
                url.join(location)
                    .map_err(|_| ApiError::bad_request("Invalid image redirect"))?
                    .as_str(),
            )?;
        } else {
            response = Some(result);
            break;
        }
    }
    let mut response = response.ok_or_else(ApiError::unavailable)?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(ApiError::not_found("Remote image not found"));
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.split(';').next())
        .map(str::trim)
        .unwrap_or("")
        .to_owned();
    if !matches!(
        mime.as_str(),
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp" | "image/gif"
    ) {
        return Err(ApiError::bad_request(
            "Remote response is not a supported image",
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PROXY_BYTES)
    {
        return Err(ApiError::bad_request("Remote image exceeds 8 MiB"));
    }
    let directory = root.join("proxy-temporary");
    storage::ensure_directory(&directory).await?;
    let key = hex::encode(rand::random::<[u8; 32]>());
    let mut guard = ProxyFile(directory.join(key), None);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&guard.0)
        .await
        .map_err(|_| ApiError::unavailable())?;
    let lease = output.into_std().await;
    lease.try_lock().map_err(|_| ApiError::unavailable())?;
    super::storage::verify_named_lease(&lease, &guard.0)?;
    output = File::from_std(lease.try_clone().map_err(|_| ApiError::unavailable())?);
    guard.1 = Some(lease);
    let mut length = 0u64;
    let mut hash = Sha256::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::unavailable())?
    {
        length = length
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| ApiError::bad_request("Remote image exceeds byte budget"))?;
        if length > MAX_PROXY_BYTES {
            return Err(ApiError::bad_request("Remote image exceeds 8 MiB"));
        }
        hash.update(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|_| ApiError::unavailable())?;
    }
    if length == 0 {
        return Err(ApiError::bad_request("Remote image is empty"));
    }
    output.flush().await.map_err(|_| ApiError::unavailable())?;
    drop(output);
    let (_, _, format, _) = storage::inspect_path(guard.0.clone(), false).await?;
    let actual = match format {
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::WebP => "image/webp",
        image::ImageFormat::Gif => "image/gif",
        _ => return Err(ApiError::bad_request("Unsupported remote image format")),
    };
    if actual != mime && !(actual == "image/jpeg" && mime == "image/jpg") {
        return Err(ApiError::bad_request(
            "Remote image header disagrees with its media type",
        ));
    }
    let file = File::open(&guard.0)
        .await
        .map_err(|_| ApiError::unavailable())?;
    Ok((
        file,
        hex::encode(hash.finalize()),
        actual.to_owned(),
        length,
        guard,
    ))
}
/// Conservative public-unicast policy includes private, metadata, transition and documentation ranges.
fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && ((b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99) || b == 168))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] <= 0x01ff || s[1] == 0x0db8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && (s[1] & 0xf000) == 0)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_metadata_ipv4_mapped_and_transition_destinations_are_denied() {
        for ip in [
            "127.0.0.1",
            "0.0.0.0",
            "10.0.0.1",
            "172.16.1.1",
            "192.168.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "224.0.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:1::1",
        ] {
            assert!(!public_address(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(public_address(ip.parse().unwrap()));
        }
    }
    #[test]
    fn image_urls_cannot_carry_credentials_or_scan_ports() {
        for value in [
            "file:///etc/passwd",
            "http://localhost/image",
            "http://127.0.0.1/image",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::ffff:127.0.0.1]/image",
            "https://user:secret@example.com/image",
            "https://example.com:8443/image",
            "https://example.com/image#secret",
        ] {
            assert!(target(value).is_err());
        }
        assert!(target("https://example.com/image.png?format=png").is_ok());
    }
    #[tokio::test]
    async fn interrupted_proxy_file_guard_removes_unshared_temporary_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("temp");
        tokio::fs::write(&path, b"image").await.unwrap();
        drop(ProxyFile(path.clone(), None));
        assert!(!path.exists());
    }
}
