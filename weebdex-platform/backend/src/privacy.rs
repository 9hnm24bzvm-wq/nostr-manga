//! Deployment CSP preserves attachment bytes while blocking upstream analytics.
//! Only exact generated SvelteKit bootstrap bodies receive inline script hashes.
//! Hash construction is for this trusted generated fallback, not HTML sanitizing.

use axum::http::HeaderValue;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MAX_FALLBACK_BYTES: usize = 2 * 1024 * 1024;
const MAX_BOOTSTRAP_SCRIPTS: usize = 8;

/// Quoted SHA-256 CSP sources for verified-shape SvelteKit inline bootstraps.
/// External tags and analytics bodies never receive a hash. HTML line endings
/// normalize to LF, matching browser HTML parsing before CSP hash validation.
pub fn bootstrap_hashes(html: &str) -> Vec<String> {
    if html.len() > MAX_FALLBACK_BYTES {
        return Vec::new();
    }
    let normalized = html.replace("\r\n", "\n").replace('\r', "\n");
    let lower = normalized.to_ascii_lowercase();
    let mut cursor = 0;
    let mut hashes = BTreeSet::new();
    while let Some(relative) = lower[cursor..].find("<script") {
        let start = cursor + relative;
        let after_name = start + "<script".len();
        if lower
            .as_bytes()
            .get(after_name)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'>')
        {
            cursor = after_name;
            continue;
        }
        let Some(open_end) = lower[after_name..]
            .find('>')
            .map(|index| after_name + index)
        else {
            break;
        };
        let body_start = open_end + 1;
        let Some(close_start) = lower[body_start..]
            .find("</script>")
            .map(|index| body_start + index)
        else {
            break;
        };
        cursor = close_start + "</script>".len();
        let attributes = &lower[after_name..open_end];
        let external = attributes
            .split_ascii_whitespace()
            .any(|attribute| attribute == "src" || attribute.starts_with("src="));
        let body = &normalized[body_start..close_start];
        let body_lower = body.to_ascii_lowercase();
        let upstream_tracking = [
            "googletagmanager",
            "google-analytics",
            "gtag(",
            "g-w464k5049g",
        ]
        .iter()
        .any(|marker| body_lower.contains(marker));
        if !external
            && !upstream_tracking
            && body.contains("__sveltekit_")
            && body.contains("import(")
            && body.contains(".start(")
        {
            let hash = STANDARD.encode(Sha256::digest(body.as_bytes()));
            hashes.insert(format!("'sha256-{hash}'"));
            if hashes.len() >= MAX_BOOTSTRAP_SCRIPTS {
                break;
            }
        }
    }
    hashes.into_iter().collect()
}

/// Compute once from the generated SPA fallback when constructing the router.
/// Missing/unrecognized bootstraps fail closed; never add script unsafe-inline,
/// unsafe-eval, arbitrary external script/connect, or analytics beacon origins.
pub fn content_security_policy(ui_index: &str) -> HeaderValue {
    content_security_policy_with_connect_origins(ui_index, &[])
}

pub(crate) fn is_tracking_origin(url: &url::Url) -> bool {
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    [
        "google-analytics.com",
        "googletagmanager.com",
        "analytics.google.com",
    ]
    .iter()
    .any(|name| host == *name || host.ends_with(&format!(".{name}")))
}

/// Only exact reviewed origins are added to connect-src. Invalid inputs fail
/// closed even if an internal caller constructs configuration without validation.
pub fn content_security_policy_with_connect_origins(
    ui_index: &str,
    origins: &[String],
) -> HeaderValue {
    content_security_policy_with_origins(ui_index, origins, &[])
}
/// Exact normalized connect origins shared by capabilities and the CSP.
pub fn approved_connect_origins(origins: &[String]) -> Vec<String> {
    let mut connections = BTreeSet::new();
    for origin in origins.iter().take(17) {
        let Ok(url) = url::Url::parse(origin) else {
            continue;
        };
        if origin.len() > 300
            || !matches!(url.scheme(), "https" | "wss" | "http" | "ws")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || is_tracking_origin(&url)
        {
            continue;
        }
        let local = match url.host() {
            Some(url::Host::Domain(host)) => host == "localhost",
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        };
        if matches!(url.scheme(), "http" | "ws") && !local {
            continue;
        }
        connections.insert(url.origin().ascii_serialization());
    }
    connections.into_iter().collect()
}

pub fn content_security_policy_with_origins(
    ui_index: &str,
    origins: &[String],
    self_hosted_media: &[String],
) -> HeaderValue {
    let connections = approved_connect_origins(origins);
    let connect_sources = if connections.is_empty() {
        "'self'".to_owned()
    } else {
        format!(
            "'self' {}",
            connections.into_iter().collect::<Vec<_>>().join(" ")
        )
    };
    let mut images = BTreeSet::new();
    for origin in self_hosted_media.iter().take(1) {
        let Ok(url) = url::Url::parse(origin) else {
            continue;
        };
        let local = match url.host() {
            Some(url::Host::Domain(host)) => host == "localhost",
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        };
        if origin.len() <= 300
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && !is_tracking_origin(&url)
            && (url.scheme() == "https" || local && url.scheme() == "http")
        {
            images.insert(url.origin().ascii_serialization());
        }
    }
    let image_sources = if images.is_empty() {
        "'self' data: blob:".to_owned()
    } else {
        format!(
            "'self' data: blob: {}",
            images.into_iter().collect::<Vec<_>>().join(" ")
        )
    };
    let hashes = bootstrap_hashes(ui_index).join(" ");
    let script_sources = if hashes.is_empty() {
        "'self'".to_owned()
    } else {
        format!("'self' {hashes}")
    };
    let policy = format!(
        "default-src 'none'; script-src {script_sources}; script-src-attr 'none'; \
         connect-src {connect_sources}; img-src {image_sources}; style-src 'self' 'unsafe-inline'; \
         font-src 'self' data:; media-src 'self' blob:; worker-src 'self' blob:; \
         manifest-src 'self'; frame-src 'none'; frame-ancestors 'none'; object-src 'none'; \
         base-uri 'self'; form-action 'self'"
    );
    HeaderValue::from_str(&policy).expect("controlled CSP sources contain only ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOT: &str = "{__sveltekit_sample={base:''};Promise.all([import('/_app/start.js'),import('/_app/app.js')]).then(([kit,app])=>kit.start(app,document.body));}";

    #[test]
    fn exact_bootstrap_hash_is_allowed_and_tracking_is_not() {
        let html = format!(
            "<script>{BOOT}</script><script async src=\"https://www.googletagmanager.com/gtag/js?id=G-W464K5049G\"></script><script>gtag('config','G-W464K5049G')</script>"
        );
        let expected = format!(
            "'sha256-{}'",
            STANDARD.encode(Sha256::digest(BOOT.as_bytes()))
        );
        assert_eq!(bootstrap_hashes(&html), vec![expected.clone()]);
        let policy = content_security_policy(&html).to_str().unwrap().to_owned();
        assert!(policy.contains(&format!("script-src 'self' {expected}")));
        assert!(policy.contains("connect-src 'self'"));
        assert!(!policy.contains("googletagmanager"));
        assert!(!policy.contains("google-analytics"));
        assert!(!policy.contains("script-src 'unsafe-inline'"));
        assert!(!policy.contains("unsafe-eval"));
    }

    #[test]
    fn preserves_exact_script_whitespace_and_normalizes_html_line_endings() {
        let body = format!("\n\t{BOOT}\n");
        let lf = format!("<script>{body}</script>");
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(bootstrap_hashes(&lf), bootstrap_hashes(&crlf));
        assert_ne!(
            bootstrap_hashes(&lf),
            bootstrap_hashes(&format!("<script>{BOOT}</script>"))
        );
    }

    #[test]
    fn missing_external_or_tracking_bootstraps_fail_closed() {
        for html in [
            "",
            "<script>alert(1)</script>",
            "<script>__sveltekit_x;gtag('config','x');import('/x').then(app=>app.start())</script>",
            "<script src='/local.js'>__sveltekit_x;import('/x').then(app=>app.start())</script>",
        ] {
            assert!(bootstrap_hashes(html).is_empty());
            assert!(content_security_policy(html)
                .to_str()
                .unwrap()
                .contains("script-src 'self';"));
        }
    }

    #[test]
    fn configured_native_and_relay_origins_get_only_reviewed_transport_capabilities() {
        let connections = vec![
            "https://native.example.invalid".into(),
            "wss://relay.example.invalid".into(),
            "http://127.0.0.1:4041".into(),
            "https://www.google-analytics.com".into(),
            "https://native.example.invalid/path".into(),
            "https://user:pass@relay.example.invalid".into(),
            "https://relay.example.invalid?token=redacted".into(),
            "https://native.example.invalid/".into(),
        ];
        assert_eq!(
            approved_connect_origins(&connections),
            vec![
                "http://127.0.0.1:4041",
                "https://native.example.invalid",
                "wss://relay.example.invalid"
            ]
        );
        let policy = content_security_policy_with_origins(
            &format!("<script>{BOOT}</script>"),
            &connections,
            &["https://native.example.invalid".into()],
        );
        let policy = policy.to_str().unwrap();
        assert!(policy.contains("connect-src 'self' http://127.0.0.1:4041 https://native.example.invalid wss://relay.example.invalid;"));
        assert!(policy.contains("img-src 'self' data: blob: https://native.example.invalid;"));
        assert!(!policy.contains("img-src 'self' data: blob: wss:"));
        assert!(!policy.contains("google-analytics"));
        assert!(!policy.contains("user:pass"));
        assert!(!policy.contains("token="));
        assert!(policy.contains("frame-src 'none'"));
        assert!(policy.contains("script-src-attr 'none'"));
        assert!(!policy.contains(" *"));
    }
}
