//! NIP65 bootstrapping through explicitly expected service identities.
use crate::{
    auth,
    db::Id,
    endpoint_policy::EndpointPolicy,
    library_client::{Outcome, Recipient, Signer, signed_json},
    library_client_bundle::write_relays,
};
use anyhow::{Context, Result, ensure};
use futures_util::{StreamExt, stream};
use std::{sync::Arc, time::Duration};
pub struct RelayList {
    pub event: auth::Event,
    pub unavailable_seeds: Vec<usize>,
}
pub async fn publish(
    list: &auth::Event,
    recipient: &Recipient,
    signer: &dyn Signer,
    policy: &EndpointPolicy,
) -> Result<Outcome> {
    let owner: Id = hex::decode(&list.pubkey)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("relay-list key"))?;
    write_relays(list, owner)?;
    let endpoint = policy
        .connect(&recipient.endpoint, Some(recipient.service_key))
        .await?;
    let (status, value) = signed_json(
        &endpoint,
        signer,
        &list.pubkey,
        "POST",
        "/account-metadata",
        serde_json::to_vec(list)?,
    )
    .await?;
    ensure!(value["id"] == list.id, "relay-list receipt identity");
    match status {
        200 | 202 => Ok(Outcome::Accepted {
            event_id: list.id.clone(),
            projected: value["projected"].as_bool().context("relay-list receipt")?,
        }),
        409 if value["superseded"] == true => Ok(Outcome::Superseded {
            event_id: list.id.clone(),
        }),
        _ => anyhow::bail!("relay-list publication unavailable: HTTP{status}"),
    }
}
pub async fn fetch(
    owner: Id,
    seeds: Vec<Recipient>,
    signer: Arc<dyn Signer>,
    policy: EndpointPolicy,
) -> Result<RelayList> {
    ensure!(
        !seeds.is_empty() && seeds.len() <= 32,
        "relay-list seed budget"
    );
    tokio::time::timeout(Duration::from_secs(60), async move {
        let mut valid = vec![];
        let mut unavailable = vec![];
        let publisher = hex::encode(owner);
        let tasks = stream::iter(seeds.into_iter().enumerate())
            .map(|(index, seed)| {
                let publisher = publisher.clone();
                let signer = signer.clone();
                let policy = policy.clone();
                async move {
                    let result = async {
                        let endpoint = policy
                            .connect(&seed.endpoint, Some(seed.service_key))
                            .await?;
                        let (status, head) = signed_json(
                            &endpoint,
                            signer.as_ref(),
                            &publisher,
                            "GET",
                            &format!("/accounts/{publisher}/metadata/10002"),
                            vec![],
                        )
                        .await?;
                        ensure!(status == 200, "relay-list head unavailable");
                        let id = head["id"]
                            .as_str()
                            .filter(|s| auth::valid_hash(s))
                            .context("relay-list head ID")?;
                        let (status, value) = signed_json(
                            &endpoint,
                            signer.as_ref(),
                            &publisher,
                            "GET",
                            &format!("/events/{id}"),
                            vec![],
                        )
                        .await?;
                        ensure!(
                            status == 200,
                            "relay-list event unavailable or projection pending"
                        );
                        let event: auth::Event = serde_json::from_value(value)?;
                        ensure!(event.id == id, "relay-list event/head mismatch");
                        write_relays(&event, owner)?;
                        Ok::<_, anyhow::Error>(event)
                    }
                    .await;
                    (index, result)
                }
            })
            .buffer_unordered(4);
        tokio::pin!(tasks);
        while let Some((index, result)) = tasks.next().await {
            match result {
                Ok(event) => valid.push(event),
                Err(_) => unavailable.push(index),
            }
        }
        valid.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        unavailable.sort();
        Ok(RelayList {
            event: valid
                .into_iter()
                .next()
                .context("no valid available signed relay list")?,
            unavailable_seeds: unavailable,
        })
    })
    .await
    .context("relay-list discovery deadline")?
}
