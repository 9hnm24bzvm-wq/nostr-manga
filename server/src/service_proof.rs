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
