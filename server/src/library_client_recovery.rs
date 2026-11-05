//! Recover a complete signed public library from explicitly bound service keys.
//! Newer observed but unavailable candidates remain visible in the result.
use crate::{
    auth,
    db::Id,
    endpoint_policy::{EndpointPolicy, VerifiedEndpoint},
    library_client::{Recipient, Signer, authorization, signed_json},
    library_client_bundle::SaveBundle,
    library_manifest::{Limits, Root},
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{StreamExt, stream};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

pub struct Recovery {
    pub bundle: SaveBundle,
    pub newer_unavailable: Vec<String>,
    pub unavailable_recipients: Vec<usize>,
    pub freshness: &'static str,
}
pub async fn recover(
    owner: Id,
    recipients: Vec<Recipient>,
    signer: Arc<dyn Signer>,
    policy: EndpointPolicy,
) -> Result<Recovery> {
    ensure!(
        !recipients.is_empty() && recipients.len() <= 32,
        "recovery recipient budget"
    );
    tokio::time::timeout(
        Duration::from_secs(300),
        recover_inner(owner, recipients, signer, policy),
    )
    .await
    .context("recovery deadline")?
}
async fn recover_inner(
    owner: Id,
    recipients: Vec<Recipient>,
    signer: Arc<dyn Signer>,
    policy: EndpointPolicy,
) -> Result<Recovery> {
    let publisher = hex::encode(owner);
    let mut endpoints = vec![];
    let mut candidates = BTreeMap::new();
    let mut unavailable = vec![];
    let tasks = stream::iter(recipients.into_iter().enumerate())
        .map(|(index, recipient)| {
            let policy = policy.clone();
            let signer = signer.clone();
            let publisher = publisher.clone();
            async move {
                let outcome = async {
                    let endpoint = policy
                        .connect(&recipient.endpoint, Some(recipient.service_key))
                        .await?;
                    let (status, value) = signed_json(
                        &endpoint,
                        signer.as_ref(),
                        &publisher,
                        "GET",
                        &format!("/libraries/{publisher}"),
                        vec![],
                    )
                    .await?;
                    ensure!(status == 200, "library head unavailable");
                    let event: auth::Event = serde_json::from_value(value["event"].clone())?;
                    crate::library_manifest::pointer(&event)?;
                    ensure!(
                        event.pubkey == publisher
                            && event.created_at <= auth::now().saturating_add(60),
                        "head author or future timestamp"
                    );
                    Ok::<_, anyhow::Error>((endpoint, event))
                }
                .await;
                (index, outcome)
            }
        })
        .buffer_unordered(4);
    tokio::pin!(tasks);
    while let Some((index, outcome)) = tasks.next().await {
        match outcome {
            Ok((endpoint, event)) => {
                candidates.insert(event.id.clone(), event);
                endpoints.push(endpoint);
            }
            Err(_) => unavailable.push(index),
        }
    }
    unavailable.sort();
    let mut candidates: Vec<_> = candidates.into_values().collect();
    candidates.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut newer_unavailable = vec![];
    for event in candidates {
        let graph = load_graph(&endpoints, signer.as_ref(), &publisher, &event).await;
        match graph {
            Ok(bundle) => {
                return Ok(Recovery {
                    bundle,
                    newer_unavailable,
                    unavailable_recipients: unavailable,
                    freshness: "newest_complete_observed_not_global_consensus",
                });
            }
            Err(_) => newer_unavailable.push(event.id),
        }
    }
    anyhow::bail!(
        "no complete verified library; {} observed candidates unavailable",
        newer_unavailable.len()
    )
}
async fn object(
    endpoints: &[VerifiedEndpoint],
    signer: &dyn Signer,
    owner: &str,
    hash: Id,
    limit: usize,
) -> Result<Vec<u8>> {
    let path = format!("/library/blobs/{}", hex::encode(hash));
    for endpoint in endpoints {
        let attempt = async {
            let url = format!("{}{path}", endpoint.origin());
            let event = authorization(signer, owner, &url, "GET", &[]).await?;
            let token = format!("Nostr {}", STANDARD.encode(serde_json::to_vec(&event)?));
            let (status, _, bytes) = endpoint
                .library_request(reqwest::Method::GET, &path, &token, vec![], limit)
                .await?;
            ensure!(
                status == 200 && auth::hash(&bytes) == hex::encode(hash),
                "object unavailable or corrupt"
            );
            Ok::<_, anyhow::Error>(bytes)
        }
        .await;
        if let Ok(bytes) = attempt {
            return Ok(bytes);
        }
    }
    anyhow::bail!("verified object unavailable")
}
async fn load_graph(
    endpoints: &[VerifiedEndpoint],
    signer: &dyn Signer,
    owner: &str,
    event: &auth::Event,
) -> Result<SaveBundle> {
    let limits = Limits::default();
    let hash = crate::library_manifest::pointer(event)?;
    let root_bytes = object(endpoints, signer, owner, hash, limits.max_root_bytes).await?;
    let root = Root::decode(&root_bytes, &limits)?;
    let hashes: BTreeSet<_> = root
        .families
        .iter()
        .flat_map(|f| f.chunks.iter())
        .filter(|d| !d.is_empty())
        .map(|d| d.hash)
        .collect();
    let tasks = stream::iter(hashes)
        .map(|hash| async move {
            Ok::<_, anyhow::Error>((
                hash,
                object(endpoints, signer, owner, hash, limits.max_chunk_bytes).await?,
            ))
        })
        .buffer_unordered(4);
    tokio::pin!(tasks);
    let mut chunks = BTreeMap::new();
    while let Some(result) = tasks.next().await {
        let (hash, bytes) = result?;
        chunks.insert(hash, bytes);
    }
    SaveBundle::validate(event.clone(), root_bytes, chunks)
}
