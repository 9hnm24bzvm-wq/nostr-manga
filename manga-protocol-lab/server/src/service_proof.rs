//! Experimental signed HTTP metadata envelope. Identity challenges alone do not
//! authenticate later bytes, especially across a compromised domain/proxy.
use crate::{auth, db::Id};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
pub const HEADER: &str = "x-manga-response-proof";
pub const LIMIT: usize = 2 * 1024 * 1024;
pub struct Scope<'a> {
    pub url: &'a str,
    pub method: &'a str,
    pub request: &'a str,
    pub status: u16,
}
pub fn sign(secret: &str, scope: &Scope<'_>, body: &[u8]) -> Result<String> {
    let event = auth::sign(
        secret,
        20006,
        vec![
            vec!["protocol".into(), "manga-federation-response-v1".into()],
            vec!["u".into(), scope.url.into()],
            vec!["method".into(), scope.method.into()],
            vec!["request".into(), scope.request.into()],
            vec!["status".into(), scope.status.to_string()],
            vec!["payload".into(), auth::hash(body)],
        ],
        String::new(),
    )
    .map_err(anyhow::Error::msg)?;
    Ok(format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event)?)
    ))
}
pub fn verify(header: &str, key: Id, scope: &Scope<'_>, body: &[u8]) -> Result<()> {
    ensure!(header.len() <= 16384, "service proof header budget");
    let event = auth::token(header).map_err(anyhow::Error::msg)?;
    ensure!(
        event.kind == 20006
            && event.pubkey == hex::encode(key)
            && event.created_at.abs_diff(auth::now()) <= 60,
        "service response identity or freshness"
    );
    for (name, value) in [
        ("protocol", "manga-federation-response-v1"),
        ("u", scope.url),
        ("method", scope.method),
        ("request", scope.request),
        ("status", &scope.status.to_string()),
        ("payload", &auth::hash(body)),
    ] {
        ensure!(
            event.one(name).map_err(anyhow::Error::msg)? == value,
            "service response {name} binding"
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_is_bound_to_exact_request_status_and_bytes() {
        let secret = format!("{:064x}", 11);
        // Independent coincurve x-only public key for public fixture secret 11.
        let key: Id =
            hex::decode("774ae7f858a9411e5ef4246b70c65aac5649980be5c17891bbec17895da008cb")
                .unwrap()
                .try_into()
                .unwrap();
        let request = "ab".repeat(32);
        let scope = Scope {
            url: "https://relay.example/reconcile",
            method: "POST",
            request: &request,
            status: 200,
        };
        let body = b"{\"message\":\"valid\"}";
        let proof = sign(&secret, &scope, body).unwrap();
        verify(&proof, key, &scope, body).unwrap();
        let envelope = auth::token(&proof).unwrap();
        assert_eq!(envelope.kind, 20006);
        assert_eq!(
            envelope.one("protocol").unwrap(),
            "manga-federation-response-v1"
        );
        assert!(
            verify(&proof, key, &scope, b"{\"message\":\"valid\"} ").is_err(),
            "JSON-equivalent proxy rewrite must fail"
        );
        for changed in [
            Scope {
                url: "https://other.example/reconcile",
                ..scope
            },
            Scope {
                method: "GET",
                ..scope
            },
            Scope {
                request: &"cd".repeat(32),
                ..scope
            },
            Scope {
                status: 413,
                ..scope
            },
        ] {
            assert!(verify(&proof, key, &changed, body).is_err());
        }
        assert!(verify(&proof, [8; 32], &scope, body).is_err());
        assert!(verify("", key, &scope, body).is_err());
    }
}
