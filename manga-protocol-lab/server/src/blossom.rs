//! BUD wire helpers, with the reference node's explicit server-scope restriction.
use crate::auth::{self, Event};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

pub fn token(header: &str) -> Result<Event> {
    let encoded = header
        .strip_prefix("Nostr ")
        .ok_or_else(|| anyhow::anyhow!("Nostr authorization required"))?;
    ensure!(encoded.len() <= 16384, "authorization budget");
    let raw = URL_SAFE_NO_PAD.decode(encoded)?;
    let event: Event = serde_json::from_slice(&raw)?;
    event.verify().map_err(anyhow::Error::msg)?;
    Ok(event)
}
pub fn scope(event: &Event, domain: &str, action: &str, hash: &str) -> Result<()> {
    let now = auth::now();
    let expiry = event
        .one("expiration")
        .map_err(anyhow::Error::msg)?
        .parse::<u64>()?;
    ensure!(
        event.kind == 24242
            && event.created_at < now
            && expiry > now
            && expiry.saturating_sub(event.created_at) <= 300
            && !event.content.trim().is_empty(),
        "authorization lifetime or kind"
    );
    ensure!(
        event.one("t").map_err(anyhow::Error::msg)? == action,
        "authorization action"
    );
    let servers = event.values("server");
    ensure!(
        !servers.is_empty() && servers.contains(&domain),
        "server scope required"
    );
    ensure!(
        servers.iter().all(|s| !s.is_empty()
            && *s == s.to_ascii_lowercase()
            && !s.contains(['/', ':', '@', '?', '#'])
            && url::Host::parse(s).is_ok()),
        "lowercase domain-only server tags required"
    );
    let hashes = event.values("x");
    ensure!(
        hashes.len() <= 100 && hashes.iter().all(|h| auth::valid_hash(h)),
        "hash scope syntax"
    );
    ensure!(
        (action == "get" && hashes.is_empty()) || hashes.contains(&hash),
        "hash scope required"
    );
    Ok(())
}
pub fn path_hash(path: &str) -> Result<&str> {
    let (hash, extension) = path
        .split_once('.')
        .map_or((path, None), |(h, e)| (h, Some(e)));
    ensure!(auth::valid_hash(hash), "invalid hash");
    if let Some(extension) = extension {
        ensure!(
            !extension.is_empty()
                && extension.len() <= 32
                && extension.bytes().all(|b| b.is_ascii_alphanumeric()),
            "invalid extension"
        );
    }
    Ok(hash)
}
/// One satisfiable byte range. Unsupported multipart requests are ignored.
pub fn range(header: Option<&str>, len: usize) -> Result<Option<(usize, usize)>> {
    let Some(header) = header else {
        return Ok(None);
    };
    let Some(value) = header.strip_prefix("bytes=") else {
        return Ok(None);
    };
    if value.contains(',') {
        return Ok(None);
    }
    let (start, end) = value
        .split_once('-')
        .ok_or_else(|| anyhow::anyhow!("invalid range"))?;
    ensure!(len > 0, "empty blob");
    if start.is_empty() {
        let suffix = end.parse::<usize>()?;
        ensure!(suffix > 0, "empty suffix");
        return Ok(Some((len.saturating_sub(suffix), len - 1)));
    }
    let start = start.parse::<usize>()?;
    let end = if end.is_empty() {
        len - 1
    } else {
        end.parse::<usize>()?.min(len - 1)
    };
    ensure!(start < len && start <= end, "unsatisfiable range");
    Ok(Some((start, end)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_and_range_boundaries() {
        let hash = "ab".repeat(32);
        assert_eq!(path_hash(&format!("{hash}.jpg")).unwrap(), hash);
        assert!(path_hash(&format!("{hash}.../secret")).is_err());
        assert_eq!(range(Some("bytes=1-3"), 10).unwrap(), Some((1, 3)));
        assert_eq!(range(Some("bytes=-20"), 10).unwrap(), Some((0, 9)));
        assert_eq!(range(Some("bytes=8-99"), 10).unwrap(), Some((8, 9)));
        assert!(range(Some("bytes=10-"), 10).is_err());
        assert!(range(Some("bytes=-0"), 10).is_err());
        assert_eq!(range(Some("bytes=0-1,4-5"), 10).unwrap(), None);
    }
    #[test]
    fn domain_scope_is_mandatory_and_get_hash_scope_optional() {
        let hash = "ab".repeat(32);
        let mut event = auth::sign(
            &format!("{:064x}", 1),
            24242,
            vec![
                vec!["t".into(), "get".into()],
                vec!["server".into(), "a.example".into()],
                vec!["expiration".into(), (auth::now() + 60).to_string()],
            ],
            "Read blob".into(),
        )
        .unwrap();
        // Scope validation is independent of signature verification, which token() performs.
        event.created_at -= 1;
        assert!(scope(&event, "a.example", "get", &hash).is_ok());
        assert!(scope(&event, "b.example", "get", &hash).is_err());
        assert!(scope(&event, "a.example", "delete", &hash).is_err());
        event.tags.retain(|t| t[0] != "server");
        assert!(scope(&event, "a.example", "get", &hash).is_err());
        event
            .tags
            .push(vec!["server".into(), "https://a.example".into()]);
        assert!(scope(&event, "https://a.example", "get", &hash).is_err());
    }
}
