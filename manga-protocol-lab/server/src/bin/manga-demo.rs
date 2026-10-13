//! Local-only executable walkthrough. Public synthetic keys: never expose this demo.
mod mvp_support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    endpoint_policy::EndpointPolicy,
    library_client::{self, DeliveryLimits, Outcome, Recipient, Signer, SigningRequest},
    library_client_bundle::SaveBundle,
    library_client_recovery,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
};
use mvp_support::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
struct DemoSigner;
#[async_trait::async_trait]
impl Signer for DemoSigner {
    async fn sign(&self, r: SigningRequest) -> Result<auth::Event> {
        Ok(sign_at(2, r.kind, r.tags, r.content, r.created_at))
    }
}
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
async fn post(
    c: &reqwest::Client,
    key: u8,
    origin: &str,
    path: &str,
    body: Value,
) -> Result<Value> {
    let response = post_json(c, key, &format!("{origin}{path}"), &body).await?;
    let status = response.status();
    let bytes = response.bytes().await?;
    ensure!(
        status.is_success(),
        "{path}: HTTP {status}: {}",
        String::from_utf8_lossy(&bytes)
    );
    Ok(serde_json::from_slice(&bytes)?)
}
#[tokio::main]
async fn main() -> Result<()> {
    let origins: Vec<_> = std::env::args().skip(1).collect();
    if origins == ["--fixture-public-keys"] {
        println!(
            "{}",
            json!({"admin":public(1),"author":public(2),"services":[public(11),public(12)]})
        );
        return Ok(());
    }
    ensure!(
        origins.len() == 2,
        "usage: manga-demo LOOPBACK_ORIGIN_A LOOPBACK_ORIGIN_B"
    );
    for origin in &origins {
        let url = url::Url::parse(origin)?;
        ensure!(
            url.scheme() == "http"
                && url.host_str().is_some_and(|h| h == "localhost"
                    || h.parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())),
            "public synthetic demo keys are restricted to loopback"
        );
    }
    eprintln!("Local demo only: all signing keys are public synthetic fixtures.");
    let c = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()?;
    let policy = EndpointPolicy::new(&origins)?;
    for (n, origin) in origins.iter().enumerate() {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if c.get(origin).send().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .context("node startup")?;
        policy
            .connect(origin, Some(id(&public(11 + n as u8))))
            .await?;
        post(
            &c,
            1,
            origin,
            &format!("/admin/accounts/{}", public(2)),
            json!({"allowed":true}),
        )
        .await?;
    }
    post(
        &c,
        1,
        &origins[0],
        &format!("/admin/publishers/{}", public(2)),
        json!({"allowed":true}),
    )
    .await?;
    let image = png();
    let image_hash = upload_to(&c, 2, &origins[0], &image).await?;
    let series_bytes = cbor(json!({"version":1,"titles":{"en":"MVP demonstration"}}));
    upload_to(&c, 2, &origins[0], &series_bytes).await?;
    let series = publication(2, 1002, &series_bytes);
    post(
        &c,
        2,
        &origins[0],
        "/publications",
        serde_json::to_value(&series)?,
    )
    .await?;
    wait_projected(&c, &origins[0], &series.id).await?;
    let chapter_bytes = cbor(
        json!({"version":1,"series":series.id,"language":"en","chapter":"1","pages":[page(&image)]}),
    );
    upload_to(&c, 2, &origins[0], &chapter_bytes).await?;
    let chapter = publication(2, 1001, &chapter_bytes);
    post(
        &c,
        2,
        &origins[0],
        "/publications",
        serde_json::to_value(&chapter)?,
    )
    .await?;
    wait_projected(&c, &origins[0], &chapter.id).await?;
    let query: Value = c
        .get(format!(
            "{}/query?order=asc&limit=10&include_total=true",
            origins[0]
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    ensure!(
        query.to_string().contains(&chapter.id),
        "published chapter must be discoverable"
    );
    for (n, origin) in origins.iter().enumerate() {
        post(
            &c,
            1,
            origin,
            "/admin/peers",
            json!({"key":public(if n==0{12}else{11}),"endpoint":origins[1-n],"allowed":true}),
        )
        .await?;
    }
    for event in [&series, &chapter] {
        let result = post(
            &c,
            1,
            &origins[1],
            "/admin/sync",
            json!({"peer":public(11),"event":event.id,"filter":"","fallbacks":[]}),
        )
        .await?;
        ensure!(
            result["hosted"] == true,
            "approved sync must host the publication"
        );
        wait_projected(&c, &origins[1], &event.id).await?;
    }
    for origin in &origins {
        let bytes = c
            .get(format!("{origin}/originals/{image_hash}"))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        ensure!(
            bytes.as_ref() == image,
            "exact original bytes on each instance"
        );
    }
    let limits = Limits::default();
    let chunk = Chunk::new(
        Family::Chapters,
        vec![Record::Chapter {
            id: id(&chapter.id),
            completed: false,
            page: Some(1),
            updated_at: auth::now(),
        }],
        &limits,
    )?
    .encode(&limits)?;
    let summary = Chunk::decode(&chunk, &limits)?.1;
    let root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![if family == Family::Chapters {
                    Descriptor::from(&summary)
                } else {
                    Descriptor::empty()
                }],
            })
            .collect(),
    }
    .encode(&limits)?;
    let library = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        auth::now(),
    );
    let bundle = Arc::new(SaveBundle::validate(
        library.clone(),
        root,
        BTreeMap::from([(summary.hash(), chunk)]),
    )?);
    let recipients: Vec<_> = origins
        .iter()
        .enumerate()
        .map(|(n, endpoint)| Recipient {
            relay_url: format!("ws://{}", endpoint.trim_start_matches("http://")),
            endpoint: endpoint.clone(),
            service_key: id(&public(11 + n as u8)),
        })
        .collect();
    let signer: Arc<dyn Signer> = Arc::new(DemoSigner);
    let delivery = library_client::deliver(
        bundle,
        recipients.clone(),
        signer.clone(),
        policy.clone(),
        DeliveryLimits::default(),
    )?;
    let first = delivery.first_acceptance.await?;
    let results = delivery.completion.await?;
    ensure!(
        results
            .iter()
            .all(|r| matches!(r.outcome, Outcome::Accepted { .. })),
        "both library replicas must accept"
    );
    let recovered = library_client_recovery::recover(
        id(&public(2)),
        vec![recipients[1].clone()],
        signer,
        policy,
    )
    .await?;
    ensure!(
        recovered.bundle.event().id == library.id,
        "cold recovery must preserve head identity"
    );
    for origin in &origins {
        let page: Value = c
            .get(format!(
                "{origin}/libraries/{}/entries?family=chapters&head={}",
                public(2),
                library.id
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        ensure!(
            page["total"] == 1
                && page["entries"][0]["id"] == chapter.id
                && page["entries"][0]["page"] == 1,
            "reader view must retain exact progress on each replica"
        );
        let url = format!("{origin}/compat/mangadex/user/me");
        let me: Value = c
            .get(&url)
            .header("authorization", http_get(2, &url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        ensure!(
            me["data"]["nostr"]["pubkey"] == public(2),
            "reader identity must use authenticated key"
        );
    }
    // The demo deliberately opens signup on its second LOOPBACK instance only.
    let closed = post_json(&c, 3, &format!("{}/enrollment", origins[0]), &json!({})).await?;
    ensure!(
        closed.status() == 403,
        "first demo instance keeps operator-only enrollment"
    );
    let joined = post(&c, 3, &origins[1], "/enrollment", json!({})).await?;
    ensure!(
        joined["account"]["publisher"] == public(3)
            && joined["account"]["created"] == true
            && joined["publisher_permission_granted"] == false,
        "self-registration is scoped to its personal key"
    );
    let again = post(&c, 3, &origins[1], "/enrollment", json!({})).await?;
    ensure!(
        again["account"]["created"] == false
            && again["account"]["permission_version"] == joined["account"]["permission_version"],
        "enrollment retry preserves admission epoch"
    );
    let reader = sign_at(
        3,
        0,
        vec![],
        json!({"name":"Self-enrolled local reader"}).to_string(),
        auth::now(),
    );
    post(
        &c,
        3,
        &origins[1],
        "/account-metadata",
        serde_json::to_value(reader)?,
    )
    .await?;
    let group = post(
        &c,
        1,
        &origins[0],
        "/compat/mangadex/group",
        json!({"administrator":public(2)}),
    )
    .await?;
    let group_key = group["data"]["nostr"]["group"]
        .as_str()
        .context("group key")?;
    let group_id = group["data"]["id"].as_str().context("group alias")?;
    let profile = post(
        &c,
        2,
        &origins[0],
        &format!("/compat/mangadex/group/{group_id}/profile"),
        json!({"metadata":{"name":"Local demo group"}}),
    )
    .await?;
    let profile: auth::Event = serde_json::from_value(profile["event"].clone())?;
    profile.verify().map_err(anyhow::Error::msg)?;
    ensure!(
        profile.pubkey == group_key && profile.kind == 0,
        "group profile signing identity"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"passed":true,"mode":"loopback_only_synthetic_keys","instances":origins,"series":series.id,"chapter":chapter.id,"original":image_hash,"library":library.id,"author":public(2),"first_library_acceptance":first.recipient,"library_replicas":results.len(),"checks":["signed series and chapter publication","chapter discovery","bilaterally approved synchronization","exact original bytes self-hosted by both nodes","public reading progress saved to both instances","complete library recovered from second instance","reader progress views and authenticated identity on both instances","administrator-managed group profile signed by group identity","closed signup on first node and scoped reader self-registration on second node"],"not_claimed":["website UI","production deployment hardening","target traffic or100TB capacity certification"]})
        )?
    );
    Ok(())
}
