//! Bounded signer-agnostic delivery. One acceptance and all-recipient completion
//! are separate signals; no private key or authorization token is persisted here.
use crate::{
    auth::{self, Event},
    db::Id,
    endpoint_policy::{EndpointPolicy, VerifiedEndpoint},
    library_client_bundle::SaveBundle,
    service_proof,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Debug)]
pub struct SigningRequest {
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}
#[async_trait::async_trait]
pub trait Signer: Send + Sync {
    async fn sign(&self, request: SigningRequest) -> Result<Event>;
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub relay_url: String,
    pub endpoint: String,
    pub service_key: Id,
}
pub struct DeliveryPlan {
    pub recipients: Vec<Recipient>,
    pub unresolved_relays: Vec<String>,
}
/// NIP65 selects write relays; explicit bindings supply their HTTP API and
/// expected service key. No ws-to-https hostname inference confers identity.
pub fn plan_write_relays(
    list: &Event,
    owner: Id,
    bindings: Vec<Recipient>,
) -> Result<DeliveryPlan> {
    ensure!(bindings.len() <= 32, "relay binding budget");
    let selected = crate::library_client_bundle::write_relays(list, owner)?;
    let mut indexed = std::collections::BTreeMap::new();
    for mut binding in bindings {
        let route = url::Url::parse(&binding.relay_url)?.to_string();
        binding.relay_url = route.clone();
        binding.endpoint = crate::endpoint_policy::canonical_origin(&binding.endpoint)?;
        ensure!(
            indexed.insert(route, binding).is_none(),
            "duplicate relay binding"
        );
    }
    let mut recipients = vec![];
    let mut unresolved_relays = vec![];
    for route in selected {
        if let Some(recipient) = indexed.remove(&route) {
            recipients.push(recipient);
        } else {
            unresolved_relays.push(route);
        }
    }
    Ok(DeliveryPlan {
        recipients,
        unresolved_relays,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Outcome {
    Accepted { event_id: String, projected: bool },
    Superseded { event_id: String },
    Rejected { reason: String },
    Retryable { reason: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipientResult {
    pub recipient: usize,
    pub outcome: Outcome,
}
#[derive(Clone, Copy)]
pub struct DeliveryLimits {
    pub concurrency: usize,
    pub recipient_timeout: Duration,
}
impl Default for DeliveryLimits {
    fn default() -> Self {
        Self {
            concurrency: 4,
            recipient_timeout: Duration::from_secs(120),
        }
    }
}
pub struct Delivery {
    pub first_acceptance: oneshot::Receiver<RecipientResult>,
    pub updates: mpsc::Receiver<RecipientResult>,
    pub completion: tokio::task::JoinHandle<Vec<RecipientResult>>,
}
fn retry(reason: &str) -> Outcome {
    Outcome::Retryable {
        reason: reason.into(),
    }
}
fn rejected(reason: &str) -> Outcome {
    Outcome::Rejected {
        reason: reason.into(),
    }
}

/// A bounded attempt per recipient. Dropping an observer does not cancel the
/// remaining recipients. The returned completion handle can explicitly abort.
pub fn deliver(
    bundle: Arc<SaveBundle>,
    recipients: Vec<Recipient>,
    signer: Arc<dyn Signer>,
    policy: EndpointPolicy,
    limits: DeliveryLimits,
) -> Result<Delivery> {
    ensure!(
        !recipients.is_empty() && recipients.len() <= 32,
        "recipient budget"
    );
    ensure!(
        (1..=8).contains(&limits.concurrency)
            && limits.recipient_timeout >= Duration::from_secs(1)
            && limits.recipient_timeout <= Duration::from_secs(600),
        "delivery limits"
    );
    let mut identities = std::collections::BTreeSet::new();
    for recipient in &recipients {
        ensure!(
            identities.insert((recipient.endpoint.clone(), recipient.service_key)),
            "duplicate recipient"
        );
        ensure!(
            crate::endpoint_policy::canonical_origin(&recipient.endpoint)? == recipient.endpoint,
            "canonical endpoint required"
        );
    }
    let (first_tx, first_acceptance) = oneshot::channel();
    let (updates_tx, updates) = mpsc::channel(32);
    let completion = tokio::spawn(async move {
        let mut first = Some(first_tx);
        let mut results = vec![];
        let futures = stream::iter(recipients.into_iter().enumerate())
            .map(|(index, recipient)| {
                let bundle = bundle.clone();
                let signer = signer.clone();
                let policy = policy.clone();
                async move {
                    let outcome = tokio::time::timeout(
                        limits.recipient_timeout,
                        attempt(&bundle, &recipient, signer.as_ref(), &policy),
                    )
                    .await
                    .unwrap_or_else(|_| retry("recipient_deadline"));
                    RecipientResult {
                        recipient: index,
                        outcome,
                    }
                }
            })
            .buffer_unordered(limits.concurrency);
        tokio::pin!(futures);
        while let Some(result) = futures.next().await {
            if matches!(result.outcome, Outcome::Accepted { .. })
                && let Some(sender) = first.take()
            {
                let _ = sender.send(result.clone());
            }
            let _ = updates_tx.try_send(result.clone());
            results.push(result);
        }
        results.sort_by_key(|result| result.recipient);
        results
    });
    Ok(Delivery {
        first_acceptance,
        updates,
        completion,
    })
}
async fn attempt(
    bundle: &SaveBundle,
    recipient: &Recipient,
    signer: &dyn Signer,
    policy: &EndpointPolicy,
) -> Outcome {
    let endpoint = match policy
        .connect(&recipient.endpoint, Some(recipient.service_key))
        .await
    {
        Ok(endpoint) => endpoint,
        Err(error) if error.is::<crate::endpoint_policy::DestinationDenied>() => {
            return rejected("destination_policy");
        }
        Err(error) if error.is::<crate::service_identity::IdentityChanged>() => {
            return rejected("service_key_mismatch");
        }
        Err(_) => return retry("identity_unavailable"),
    };
    let owner = &bundle.event().pubkey;
    let mut cursor = 0;
    loop {
        let batch = match bundle.batch(cursor) {
            Ok(Some(batch)) => batch,
            Ok(None) => break,
            Err(_) => return rejected("invalid_local_bundle"),
        };
        let response = signed_json(
            &endpoint,
            signer,
            owner,
            "POST",
            "/library/batches",
            batch.body,
        )
        .await;
        let (status, value) = match response {
            Ok(response) => response,
            Err(_) => return retry("unverified_batch_response"),
        };
        if status != 200 {
            return classify_status(status);
        }
        let expected: Vec<_> = batch.hashes.iter().map(hex::encode).collect();
        if value["hashes"] != json!(expected) || value["staged"] != true {
            return rejected("invalid_batch_receipt");
        }
        cursor = batch.next_object;
    }
    let body = match bundle.commit_body() {
        Ok(body) => body,
        Err(_) => return rejected("invalid_local_bundle"),
    };
    let (status, value) =
        match signed_json(&endpoint, signer, owner, "POST", "/libraries", body).await {
            Ok(response) => response,
            Err(_) => return retry("unverified_commit_response"),
        };
    if matches!(status, 200 | 202 | 409) && value["id"] != bundle.event().id {
        return rejected("receipt_event_mismatch");
    }
    match status {
        200 | 202 => match value["projected"].as_bool() {
            Some(projected) => Outcome::Accepted {
                event_id: bundle.event().id.clone(),
                projected,
            },
            None => rejected("invalid_commit_receipt"),
        },
        409 if value["superseded"] == true => Outcome::Superseded {
            event_id: bundle.event().id.clone(),
        },
        _ => classify_status(status),
    }
}
fn classify_status(status: u16) -> Outcome {
    if status == 408 || status == 429 || status >= 500 {
        retry(&format!("http_{status}"))
    } else {
        rejected(&format!("http_{status}"))
    }
}
pub(crate) async fn authorization(
    signer: &dyn Signer,
    owner: &str,
    url: &str,
    method: &str,
    body: &[u8],
) -> Result<Event> {
    let mut tags = vec![
        vec!["u".into(), url.into()],
        vec!["method".into(), method.into()],
        vec!["nonce".into(), hex::encode(rand::random::<[u8; 32]>())],
    ];
    if method == "POST" {
        tags.push(vec!["payload".into(), auth::hash(body)]);
    }
    let request = SigningRequest {
        pubkey: owner.into(),
        created_at: auth::now(),
        kind: 27235,
        tags,
        content: String::new(),
    };
    let event = signer.sign(request.clone()).await?;
    event.verify().map_err(anyhow::Error::msg)?;
    ensure!(
        event.pubkey == request.pubkey
            && event.created_at == request.created_at
            && event.kind == request.kind
            && event.tags == request.tags
            && event.content == request.content,
        "signer changed requested event"
    );
    auth::http(&event, url, method, body).map_err(anyhow::Error::msg)?;
    Ok(event)
}
pub(crate) async fn signed_json(
    endpoint: &VerifiedEndpoint,
    signer: &dyn Signer,
    owner: &str,
    method: &str,
    path: &str,
    body: Vec<u8>,
) -> Result<(u16, Value)> {
    let url = format!("{}{path}", endpoint.origin());
    let event = authorization(signer, owner, &url, method, &body).await?;
    let token = format!("Nostr {}", STANDARD.encode(serde_json::to_vec(&event)?));
    let (status, headers, bytes) = endpoint
        .library_request(method.parse()?, path, &token, body, 65536)
        .await?;
    let header = headers
        .get(service_proof::HEADER)
        .context("missing response signature")?
        .to_str()?;
    service_proof::verify(
        header,
        endpoint.service_key(),
        &service_proof::Scope {
            url: &url,
            method,
            request: &event.id,
            status,
        },
        &bytes,
    )?;
    Ok((status, serde_json::from_slice(&bytes)?))
}
