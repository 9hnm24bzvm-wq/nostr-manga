use crate::{
    contracts::library::ProfilePatch,
    error::{ApiError, ApiResult},
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
pub fn id(s: &str) -> ApiResult<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':'))
    {
        Err(ApiError::bad_request("Invalid identifier"))
    } else {
        Ok(())
    }
}
pub fn ids(values: &[String], max: usize) -> ApiResult<Vec<String>> {
    if values.len() > max {
        return Err(ApiError::bad_request("Too many identifiers"));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for v in values {
        id(v)?;
        if seen.insert(v.clone()) {
            out.push(v.clone());
        }
    }
    Ok(out)
}
pub fn text(s: &str, max: usize, nonempty: bool) -> ApiResult<String> {
    let s = s.trim();
    if s.len() > max || s.contains('\0') || nonempty && s.is_empty() {
        Err(ApiError::bad_request("Invalid text length"))
    } else {
        Ok(s.to_owned())
    }
}
pub fn profile(p: &ProfilePatch) -> ApiResult<Value> {
    if p.version < 1 {
        return Err(ApiError::bad_request(
            "A positive profile version is required",
        ));
    }
    let mut out = Map::new();
    if let Some(s) = &p.description {
        out.insert("description".into(), Value::String(text(s, 20_000, false)?));
    }
    for (k, s) in [
        ("discord", &p.discord),
        ("twitter", &p.twitter),
        ("website", &p.website),
    ] {
        if let Some(s) = s {
            let s = text(s, 2048, false)?;
            if !s.is_empty() {
                match k {
                    "discord" if !s.starts_with("http://") && !s.starts_with("https://") => {
                        if !s
                            .trim_start_matches('/')
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                        {
                            return Err(ApiError::bad_request("Invalid Discord invite"));
                        }
                    }
                    _ => {
                        let u = url::Url::parse(&s).map_err(|_| {
                            ApiError::bad_request("Contact links must use HTTP or HTTPS")
                        })?;
                        if !matches!(u.scheme(), "http" | "https")
                            || u.host_str().is_none()
                            || !u.username().is_empty()
                            || u.password().is_some()
                        {
                            return Err(ApiError::bad_request("Invalid contact link"));
                        }
                        if k == "twitter"
                            && !u.host_str().is_some_and(|h| {
                                h == "twitter.com"
                                    || h.ends_with(".twitter.com")
                                    || h == "x.com"
                                    || h.ends_with(".x.com")
                            })
                        {
                            return Err(ApiError::bad_request(
                                "Twitter link must use twitter.com or x.com",
                            ));
                        }
                        if k == "discord"
                            && !u.host_str().is_some_and(|h| {
                                h == "discord.gg" || h == "discord.com" || h == "discordapp.com"
                            })
                        {
                            return Err(ApiError::bad_request("Invalid Discord invite host"));
                        }
                    }
                }
            }
            out.insert(k.into(), Value::String(s));
        }
    }
    Ok(Value::Object(out))
}
pub fn new_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!("list_{}", hex::encode(bytes))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_bounded_deduped_and_path_safe() {
        assert_eq!(
            ids(&["a".into(), "a".into(), "b".into()], 3).unwrap(),
            vec!["a", "b"]
        );
        for bad in ["../file", "a/b", "a?b", "", "a b"] {
            assert!(id(bad).is_err());
        }
        assert!(ids(&["a".into(), "b".into()], 1).is_err());
    }
    #[test]
    fn profile_rejects_unsafe_links_and_accepts_utf8() {
        let mut p = ProfilePatch {
            version: 1,
            description: Some("日本語の説明".into()),
            discord: Some("invite_42".into()),
            twitter: None,
            website: Some("https://example.invalid/about".into()),
        };
        assert!(profile(&p).is_ok());
        p.website = Some("javascript:alert(1)".into());
        assert!(profile(&p).is_err());
        p.website = None;
        p.twitter = Some("https://twitter.com.evil.invalid/foo".into());
        assert!(profile(&p).is_err());
        p.twitter = None;
        p.version = 0;
        assert!(profile(&p).is_err());
    }
}
