//! Automatic binding for this project's same-origin root WebSocket relay profile.
//! A hostname is a candidate location, never identity evidence by itself.
use crate::{
    auth::Event,
    db::Id,
    endpoint_policy::{self, EndpointPolicy},
    library_client::Recipient,
    library_client_bundle::write_relays,
};
use anyhow::{Result, ensure};
use futures_util::{StreamExt, stream};
use serde::Serialize;
use std::{collections::BTreeMap, time::Duration};
#[derive(Debug, Serialize)]
pub struct Unresolved {
    pub relay_url: String,
    pub reason: &'static str,
}
#[derive(Debug)]
pub struct Discovery {
    pub recipients: Vec<Recipient>,
    /// Persist these verified bindings before future discovery. This SDK does
    /// not silently replace a previously remembered service key.
    pub first_observations: Vec<Recipient>,
    pub unresolved: Vec<Unresolved>,
}
fn candidate(route: &str) -> Result<String> {
    let mut url = url::Url::parse(route)?;
    ensure!(
        url.path() == "/" && url.query().is_none() && url.fragment().is_none(),
        "non-root relay requires explicit binding"
    );
    let scheme = match url.scheme() {
        "wss" => "https",
        "ws" => "http",
        _ => anyhow::bail!("WebSocket route required"),
    };
    url.set_scheme(scheme)
        .map_err(|_| anyhow::anyhow!("relay scheme"))?;
    endpoint_policy::canonical_origin(url.as_str())
}
/// Discover only routes selected by the owner's signed NIP65 list. Known
/// bindings may use non-root routes; their explicit expected keys still apply.
/// Unknown root routes require live possession proof and compatible metadata.
/// First contact establishes continuity from now, not historic ownership.
pub async fn discover(
    list: &Event,
    owner: Id,
    remembered: Vec<Recipient>,
    policy: EndpointPolicy,
) -> Result<Discovery> {
    let routes = write_relays(list, owner)?;
    ensure!(remembered.len() <= 32, "remembered binding budget");
    let mut known = BTreeMap::new();
    for mut entry in remembered {
        entry.relay_url = url::Url::parse(&entry.relay_url)?.to_string();
        entry.endpoint = endpoint_policy::canonical_origin(&entry.endpoint)?;
        ensure!(
            known.insert(entry.relay_url.clone(), entry).is_none(),
            "duplicate remembered binding"
        );
    }
    let tasks = stream::iter(routes.into_iter().enumerate())
        .map(|(index, route)| {
            let remembered = known.get(&route).cloned();
            let policy = policy.clone();
            async move {
                let result = tokio::time::timeout(Duration::from_secs(15), async {
                    let origin = match &remembered {
                        Some(entry) => entry.endpoint.clone(),
                        None => candidate(&route).map_err(|_| "explicit_binding_required")?,
                    };
                    let endpoint = policy
                        .connect(&origin, remembered.as_ref().map(|r| r.service_key))
                        .await
                        .map_err(|_| "identity_or_destination_unavailable")?;
                    let bytes = endpoint
                        .get_metadata("/")
                        .await
                        .map_err(|_| "capabilities_unavailable")?;
                    let value: serde_json::Value =
                        serde_json::from_slice(&bytes).map_err(|_| "invalid_capabilities")?;
                    let manga = &value["manga"];
                    if manga["namespace_version"] != 1
                        || manga["service_pubkey"] != hex::encode(endpoint.service_key())
                        || manga["features"]["public_libraries"] != true
                        || !manga["api_versions"]
                            .as_array()
                            .is_some_and(|versions| versions.contains(&serde_json::json!(1)))
                    {
                        return Err("unsupported_library_service");
                    }
                    // Never follow a URL advertised by unsigned capability metadata.
                    if manga["base_url"]
                        .as_str()
                        .and_then(|s| endpoint_policy::canonical_origin(s).ok())
                        .as_deref()
                        != Some(origin.as_str())
                    {
                        return Err("capability_origin_mismatch");
                    }
                    Ok(Recipient {
                        relay_url: route.clone(),
                        endpoint: origin,
                        service_key: endpoint.service_key(),
                    })
                })
                .await
                .unwrap_or(Err("discovery_timeout"));
                (index, route, remembered.is_none(), result)
            }
        })
        .buffer_unordered(4);
    tokio::pin!(tasks);
    let mut outcomes = vec![];
    while let Some(outcome) = tasks.next().await {
        outcomes.push(outcome)
    }
    outcomes.sort_by_key(|o| o.0);
    let mut result = Discovery {
        recipients: vec![],
        first_observations: vec![],
        unresolved: vec![],
    };
    for (_, route, first, outcome) in outcomes {
        match outcome {
            Ok(recipient) => {
                if first {
                    result.first_observations.push(recipient.clone())
                }
                result.recipients.push(recipient)
            }
            Err(reason) => result.unresolved.push(Unresolved {
                relay_url: route,
                reason,
            }),
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn signed_non_root_routes_require_explicit_binding() {
        let list = crate::auth::sign(
            &format!("{:064x}", 7),
            10002,
            vec![
                vec!["r".into(), "wss://example.com/relay".into(), "write".into()],
                vec![
                    "r".into(),
                    "wss://example.com/?token=x".into(),
                    "write".into(),
                ],
            ],
            String::new(),
        )
        .unwrap();
        let owner = hex::decode(&list.pubkey).unwrap().try_into().unwrap();
        let result = discover(&list, owner, vec![], EndpointPolicy::default())
            .await
            .unwrap();
        assert!(result.recipients.is_empty());
        assert!(result.first_observations.is_empty());
        assert_eq!(result.unresolved.len(), 2);
        assert!(
            result
                .unresolved
                .iter()
                .all(|route| route.reason == "explicit_binding_required")
        );
    }
}
