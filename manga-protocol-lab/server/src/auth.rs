use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
pub fn hash(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
pub fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}
impl Event {
    pub fn digest(&self) -> Vec<u8> {
        Sha256::digest(
            serde_json::to_vec(&json!([
                0,
                self.pubkey,
                self.created_at,
                self.kind,
                self.tags,
                self.content
            ]))
            .unwrap(),
        )
        .to_vec()
    }
    pub fn verify(&self) -> Result<(), String> {
        if !valid_hash(&self.id)
            || !valid_hash(&self.pubkey)
            || self.kind > 65535
            || self.tags.len() > 2000
            || self.created_at > now() + 900
        {
            return Err("invalid event fields".into());
        }
        let d = self.digest();
        if hex::encode(&d) != self.id {
            return Err("event ID mismatch".into());
        }
        let pk = XOnlyPublicKey::from_slice(&hex::decode(&self.pubkey).map_err(|_| "pubkey")?)
            .map_err(|_| "pubkey")?;
        let sig = Signature::from_slice(&hex::decode(&self.sig).map_err(|_| "signature")?)
            .map_err(|_| "signature")?;
        Secp256k1::verification_only()
            .verify_schnorr(
                &sig,
                &Message::from_digest_slice(&d).map_err(|_| "digest")?,
                &pk,
            )
            .map_err(|_| "bad signature".into())
    }
    pub fn values(&self, key: &str) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|t| t.len() >= 2 && t[0] == key)
            .map(|t| t[1].as_str())
            .collect()
    }
    pub fn one(&self, key: &str) -> Result<&str, String> {
        let v = self.values(key);
        if v.len() == 1 {
            Ok(v[0])
        } else {
            Err(format!("missing/duplicate {key}"))
        }
    }
}
pub fn sign(
    secret: &str,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
) -> Result<Event, String> {
    sign_at(secret, kind, tags, content, now())
}
pub(crate) fn sign_at(
    secret: &str,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    created_at: u64,
) -> Result<Event, String> {
    let sk = SecretKey::from_slice(&hex::decode(secret).map_err(|_| "secret encoding")?)
        .map_err(|_| "secret")?;
    let secp = Secp256k1::new();
    let key = Keypair::from_secret_key(&secp, &sk);
    let mut e = Event {
        id: String::new(),
        pubkey: key.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags,
        content,
        sig: String::new(),
    };
    let d = e.digest();
    e.id = hex::encode(&d);
    e.sig = secp
        .sign_schnorr_no_aux_rand(&Message::from_digest_slice(&d).unwrap(), &key)
        .to_string();
    Ok(e)
}
pub fn token(header: &str) -> Result<Event, String> {
    let b = header
        .strip_prefix("Nostr ")
        .ok_or("missing Nostr authorization")?;
    if b.len() > 32768 {
        return Err("authorization too large".into());
    }
    let raw = STANDARD
        .decode(b)
        .or_else(|_| URL_SAFE_NO_PAD.decode(b))
        .map_err(|_| "authorization encoding")?;
    let e: Event = serde_json::from_slice(&raw).map_err(|_| "authorization JSON")?;
    e.verify()?;
    Ok(e)
}
pub fn http_scope(e: &Event, url: &str, method: &str) -> Result<(), String> {
    if e.kind != 27235 || e.created_at.abs_diff(now()) > 60 {
        return Err("HTTP auth kind/freshness".into());
    }
    if e.one("u")? != url || e.one("method")? != method {
        return Err("HTTP request binding".into());
    }
    Ok(())
}
pub fn http(e: &Event, url: &str, method: &str, body: &[u8]) -> Result<(), String> {
    http_scope(e, url, method)?;
    if (!body.is_empty() || ["POST", "PUT", "PATCH", "DELETE"].contains(&method))
        && e.one("payload")? != hash(body)
    {
        return Err("HTTP body binding".into());
    }
    Ok(())
}
pub fn blossom(e: &Event, domain: &str, action: &str, h: &str) -> Result<(), String> {
    let expiry = e
        .one("expiration")?
        .parse::<u64>()
        .map_err(|_| "expiration")?;
    if e.kind != 24242
        || e.created_at >= now()
        || expiry <= now()
        || expiry.saturating_sub(e.created_at) > 300
        || e.content.is_empty()
    {
        return Err("blob auth lifetime/kind".into());
    }
    if e.one("t")? != action || !e.values("server").contains(&domain) {
        return Err("blob action/domain scope".into());
    }
    let hashes = e.values("x");
    if hashes.len() > 100 || !hashes.contains(&h) || hashes.iter().any(|h| !valid_hash(h)) {
        return Err("blob hash scope".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn secret() -> String {
        format!("{:064x}", 1)
    }
    fn tags(pairs: &[(&str, &str)]) -> Vec<Vec<String>> {
        pairs
            .iter()
            .map(|(k, v)| vec![k.to_string(), v.to_string()])
            .collect()
    }
    #[test]
    fn http_requires_exact_url_and_method() {
        let e = sign(
            &secret(),
            27235,
            tags(&[("u", "https://a/query?x=1"), ("method", "GET")]),
            String::new(),
        )
        .unwrap();
        assert!(http(&e, "https://a/query?x=1", "GET", b"").is_ok());
        assert!(http(&e, "https://a/query?x=2", "GET", b"").is_err());
        assert!(http(&e, "https://a/query?x=1", "POST", b"").is_err());
    }
    #[test]
    fn http_binds_empty_mutating_body_too() {
        let e = sign(
            &secret(),
            27235,
            tags(&[("u", "https://a/upload"), ("method", "POST")]),
            String::new(),
        )
        .unwrap();
        assert!(http(&e, "https://a/upload", "POST", b"").is_err());
        let e = sign(
            &secret(),
            27235,
            tags(&[
                ("u", "https://a/upload"),
                ("method", "POST"),
                (
                    "payload",
                    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                ),
            ]),
            String::new(),
        )
        .unwrap();
        e.verify().unwrap();
        assert!(http(&e, "https://a/upload", "POST", b"").is_ok());
        assert!(http(&e, "https://a/upload", "POST", b"changed").is_err());
    }
    #[test]
    fn duplicate_binding_is_rejected() {
        let e = sign(
            &secret(),
            27235,
            tags(&[("u", "https://a"), ("u", "https://a"), ("method", "GET")]),
            String::new(),
        )
        .unwrap();
        assert!(http(&e, "https://a", "GET", b"").is_err());
    }
    #[test]
    fn http_freshness_boundary() {
        let e = sign_at(
            &secret(),
            27235,
            tags(&[("u", "https://a"), ("method", "GET")]),
            String::new(),
            now() - 61,
        )
        .unwrap();
        e.verify().unwrap();
        assert!(http(&e, "https://a", "GET", b"").is_err());
    }
    #[test]
    fn blob_scope_and_lifetime() {
        let h = hash(b"fixture");
        let expiry = (now() + 120).to_string();
        let e = sign_at(
            &secret(),
            24242,
            tags(&[
                ("t", "get"),
                ("server", "a.example"),
                ("x", &h),
                ("expiration", &expiry),
            ]),
            "read test".into(),
            now() - 1,
        )
        .unwrap();
        e.verify().unwrap();
        assert!(blossom(&e, "a.example", "get", &h).is_ok());
        assert!(blossom(&e, "b.example", "get", &h).is_err());
        assert!(blossom(&e, "a.example", "delete", &h).is_err());
        assert!(blossom(&e, "a.example", "get", &hash(b"other")).is_err());
        let e = sign_at(&secret(), 24242, e.tags, e.content, now() - 301).unwrap();
        e.verify().unwrap();
        assert!(blossom(&e, "a.example", "get", &h).is_err());
    }
    #[test]
    fn malformed_signature_never_panics() {
        let mut e = sign(&secret(), 1111, vec![], String::new()).unwrap();
        for sig in ["", "zz", "01", &"0".repeat(128)] {
            e.sig = sig.into();
            assert!(e.verify().is_err());
        }
    }
}

#[cfg(test)]
mod independent_vectors {
    use crate::auth::{Event, sign_at, token};
    use base64::{
        Engine,
        engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    };

    #[test]
    fn signed_utf8_event_matches_external_nip01_vector() {
        // Python json/hashlib plus coincurve; fixture key 1 is public synthetic data.
        let raw = include_str!("../tests/fixtures/protocol-vectors/signed-event.json");
        let expected: Event = serde_json::from_str(raw).unwrap();
        expected.verify().unwrap();
        assert_eq!(
            hex::encode(expected.digest()),
            "fc4459a563bf110a9fca0683982380cb80b6fb5875d95f550169e91e0c8d46b0"
        );
        let actual = sign_at(
            &format!("{:064x}", 1),
            1,
            vec![vec!["t".into(), "manga".into()]],
            "Hello, 漫画".into(),
            1700000000,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        for encoding in [&STANDARD, &URL_SAFE_NO_PAD] {
            let decoded = token(&format!("Nostr {}", encoding.encode(raw))).unwrap();
            assert_eq!(decoded.id, expected.id);
            assert_eq!(decoded.content, "Hello, 漫画");
        }
        let mut tampered = expected;
        tampered.content.push('!');
        assert!(tampered.verify().is_err());
        assert!(
            token(&format!(
                "Nostr {}",
                STANDARD.encode(serde_json::to_vec(&tampered).unwrap())
            ))
            .is_err()
        );
    }
}
