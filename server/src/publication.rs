//! Complete-before-hosted publication with versioned preparation and short commit.
use crate::{
    auth::{self, Event},
    blobs::BlobIntent,
    db::{Id, Repository},
    manifest::{self, Manifest},
    storage_accounting::retain_blobs_tx,
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pointer {
    manifest: String,
    version: u64,
}
fn id(text: &str) -> Result<Id> {
    ensure!(auth::valid_hash(text), "invalid identifier");
    Ok(hex::decode(text)?.try_into().unwrap())
}
pub(crate) fn pointer(e: &Event) -> Result<Id> {
    e.verify().map_err(anyhow::Error::msg)?;
    ensure!(
        matches!(e.kind, 1001 | 1002),
        "not an experimental publication kind"
    );
    ensure!(
        e.values("expiration").is_empty() && e.tags.iter().flatten().all(|v| v.len() <= 1024),
        "unsupported lifecycle or tag policy"
    );
    let p: Pointer = serde_json::from_str(&e.content)?;
    ensure!(p.version == 1, "unsupported publication schema");
    id(&p.manifest)
}
pub(crate) fn pointer_for_sync(event: &Event) -> Result<Id> {
    use crate::transfer_rejection::{
        RejectionReason as Why, TransferRejection as R, pointer_version,
    };
    event
        .verify()
        .map_err(|_| R::invalid(Why::PublicationPointer))?;
    pointer_version(&event.content, Why::PublicationPointer)?;
    if !event.values("expiration").is_empty() {
        return Err(R::unsupported(Why::Lifecycle).into());
    }
    pointer(event).map_err(|_| R::invalid(Why::PublicationPointer).into())
}
enum Authority {
    Local(crate::publisher_authority::PublisherPermit),
    Review(crate::submission_review::ReviewPermit),
    Peers(Vec<(Id, i64)>),
    Group { group: Id, member: Id, version: i64 },
}
async fn check_authority(
    tx: &deadpool_postgres::Transaction<'_>,
    authority: &Authority,
) -> Result<()> {
    match authority {
        Authority::Review(permit) => {
            crate::submission_review::check_review_tx(tx, permit, true).await?
        }
        Authority::Local(permit) => {
            crate::publisher_authority::check_publisher_tx(tx, permit).await?;
        }
        Authority::Group {
            group,
            member,
            version,
        } => {
            crate::groups::check_group_tx(tx, *group, *member, *version, "publish").await?;
        }
        Authority::Peers(peers) => {
            ensure!(
                !peers.is_empty() && peers.len() <= 8,
                "peer authority bounds"
            );
            for (key, version) in peers {
                let row = tx
                    .query_opt(
                        "SELECT allowed,version FROM approved_peers WHERE service_key=$1 FOR SHARE",
                        &[&key.as_slice()],
                    )
                    .await?
                    .context("peer not approved")?;
                ensure!(
                    row.get::<_, bool>(0) && row.get::<_, i64>(1) == *version,
                    "peer approval changed during preparation"
                );
            }
        }
    }
    Ok(())
}
pub struct PreparedPublication {
    event: Event,
    event_version: i64,
    authority: Authority,
    staging_owner: Id,
    lease: Option<crate::db::Lease>,
    manifest: Id,
    chapter: Option<Manifest>,
    series_metadata: Option<crate::series_metadata::SeriesMetadata>,
    sources: BTreeMap<Id, (BlobIntent, String)>,
}
impl PreparedPublication {
    pub fn with_lease(mut self, lease: Option<crate::db::Lease>) -> Self {
        self.lease = lease;
        self
    }
}
pub struct Publications {
    repo: Repository,
    storage: Arc<UploadStore>,
    pub slots: Arc<tokio::sync::Semaphore>,
}
impl Publications {
    pub fn new(repo: Repository, storage: Arc<UploadStore>) -> Arc<Self> {
        Arc::new(Self {
            repo,
            storage,
            slots: Arc::new(tokio::sync::Semaphore::new(2)),
        })
    }
    pub async fn prepare(&self, event: &Event) -> Result<PreparedPublication> {
        self.prepare_with_permit(event, self.repo.publisher_permit(id(&event.pubkey)?).await?)
            .await
    }
    pub async fn prepare_with_permit(
        &self,
        event: &Event,
        permit: crate::publisher_authority::PublisherPermit,
    ) -> Result<PreparedPublication> {
        ensure!(
            id(&event.pubkey)? == permit.publisher,
            "publisher authorization identity mismatch"
        );
        self.prepare_authorized(event, None, None, Some(permit), None)
            .await
    }
    pub(crate) async fn prepare_from_peers(
        &self,
        event: &Event,
        mut peers: Vec<(Id, i64)>,
    ) -> Result<PreparedPublication> {
        peers.sort();
        peers.dedup();
        self.prepare_authorized(event, Some(peers), None, None, None)
            .await
    }
    pub async fn prepare_for_group(
        &self,
        event: &Event,
        group: Id,
        member: Id,
        version: i64,
    ) -> Result<PreparedPublication> {
        ensure!(
            id(&event.pubkey)? == group,
            "group signature identity mismatch"
        );
        self.prepare_authorized(event, None, Some((group, member, version)), None, None)
            .await
    }
    pub(crate) async fn prepare_for_review(
        &self,
        event: &Event,
        permit: crate::submission_review::ReviewPermit,
    ) -> Result<PreparedPublication> {
        ensure!(
            id(&event.pubkey)? == permit.author
                && id(&event.id)? == permit.submission
                && permit.moderator.is_some(),
            "review publication identity or moderator"
        );
        self.prepare_authorized(event, None, None, None, Some(permit))
            .await
    }
    async fn prepare_authorized(
        &self,
        event: &Event,
        peers: Option<Vec<(Id, i64)>>,
        group: Option<(Id, Id, i64)>,
        local: Option<crate::publisher_authority::PublisherPermit>,
        review: Option<crate::submission_review::ReviewPermit>,
    ) -> Result<PreparedPublication> {
        let manifest_id = pointer(event)?;
        let event_id = id(&event.id)?;
        let owner = id(&event.pubkey)?;
        let raw = serde_json::to_value(event)?;
        ensure!(
            serde_json::to_vec(&raw)?.len() <= 1048576,
            "event size budget"
        );
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        crate::submission_review::lock_event(&tx, event_id).await?;
        crate::submission_review::check_publication_verdict(&tx, event_id).await?;
        let staging_owner = self.storage.owner();
        let authority = if let Some(review) = review {
            Authority::Review(review)
        } else if let Some((group, member, version)) = group {
            Authority::Group {
                group,
                member,
                version,
            }
        } else if let Some(peers) = peers {
            Authority::Peers(peers)
        } else {
            Authority::Local(local.context("local publisher permit required")?)
        };
        check_authority(&tx, &authority).await?;
        tx.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state,manifest) VALUES($1,$2,$3,$4,$5,'candidate',$6) ON CONFLICT(id) DO NOTHING", &[&event_id.as_slice(),&owner.as_slice(),&(event.kind as i32),&(event.created_at as i64),&raw,&manifest_id.as_slice()]).await?;
        let row = tx
            .query_one(
                "SELECT version,state,manifest FROM events WHERE id=$1",
                &[&event_id.as_slice()],
            )
            .await?;
        ensure!(
            matches!(row.get::<_, String>(1).as_str(), "candidate" | "hosted"),
            crate::transfer_rejection::TransferRejection::policy(
                crate::transfer_rejection::RejectionReason::LocalRemoval
            )
        );
        ensure!(
            row.get::<_, Option<Vec<u8>>>(2) == Some(manifest_id.to_vec()),
            "publication manifest changed"
        );
        let event_version: i64 = row.get(0);
        let already_hosted = row.get::<_, String>(1) == "hosted";
        tx.commit().await?;
        drop(client);
        if already_hosted {
            // A durable accepted event outlives its temporary upload grants.
            // Commit still rechecks authority, review verdict, generation and
            // lifecycle before its existing idempotent hosted branch. Never
            // prepare a fresh publication this way or bypass local deletion.
            return Ok(PreparedPublication {
                event: event.clone(),
                event_version,
                authority,
                staging_owner,
                lease: None,
                manifest: manifest_id,
                chapter: None,
                series_metadata: None,
                sources: BTreeMap::new(),
            });
        }
        if let Authority::Review(permit) = &authority {
            let mut c = self.repo.pool.get().await?;
            let tx = c.transaction().await?;
            crate::submission_review::check_review_grants_tx(&tx, permit, &[manifest_id.to_vec()])
                .await?;
            tx.commit().await?;
        }
        if let Authority::Group { group, .. } = &authority {
            ensure!(self.repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM group_blob_grants g JOIN blob_intents b ON b.hash=g.hash AND b.generation=g.generation WHERE g.group_key=$1 AND g.hash=$2 AND g.lease_until>clock_timestamp())", &[&group.as_slice(),&manifest_id.as_slice()]).await?.get::<_,bool>(0),"group lacks manifest staging grant");
        }
        if let Authority::Local(permit) = &authority
            && permit.publisher != staging_owner
        {
            let mut c = self.repo.pool.get().await?;
            let tx = c.transaction().await?;
            crate::publisher_authority::check_grants_tx(&tx, permit, &[manifest_id.to_vec()])
                .await?;
            tx.commit().await?;
        }
        let (blob, bytes) = self.storage.snapshot(manifest_id, staging_owner).await?;
        let kind = event.kind;
        let (chapter, series_metadata) = crate::node::spawn_blocking_owned_scope(move || -> Result<(Option<Manifest>, Option<crate::series_metadata::SeriesMetadata>)> {
            if kind == 1001 {
                Ok((Some(manifest::chapter(&bytes).map_err(anyhow::Error::msg)?),None))
            } else {
                Ok((None,Some(crate::series_metadata::parse(&bytes).map_err(anyhow::Error::msg)?)))
            }
        })
        .await??;
        let mut sources = BTreeMap::from([(manifest_id, (blob, "application/cbor".into()))]);
        let originals = chapter
            .as_ref()
            .map(|m| m.pages.clone())
            .unwrap_or_else(|| {
                series_metadata
                    .as_ref()
                    .and_then(|m| m.cover.clone())
                    .into_iter()
                    .collect()
            });
        {
            if let Authority::Review(permit) = &authority {
                let hashes: Vec<Vec<u8>> = manifest::unique_pages(&originals)
                    .map_err(anyhow::Error::msg)?
                    .iter()
                    .map(|p| hex::decode(&p.hash).unwrap())
                    .collect();
                let mut c = self.repo.pool.get().await?;
                let tx = c.transaction().await?;
                crate::submission_review::check_review_grants_tx(&tx, permit, &hashes).await?;
                tx.commit().await?;
            }
            for page in manifest::unique_pages(&originals).map_err(anyhow::Error::msg)? {
                ensure!(
                    self.storage.formats.contains(&page.mime),
                    "image format disabled by instance policy"
                );
                let hash = id(&page.hash)?;
                if let Authority::Group { group, .. } = &authority {
                    ensure!(self.repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM group_blob_grants g JOIN blob_intents b ON b.hash=g.hash AND b.generation=g.generation WHERE g.group_key=$1 AND g.hash=$2 AND g.lease_until>clock_timestamp())", &[&group.as_slice(),&hash.as_slice()]).await?.get::<_,bool>(0),"group lacks original staging grant");
                }
                if let Authority::Local(permit) = &authority
                    && permit.publisher != staging_owner
                {
                    let mut c = self.repo.pool.get().await?;
                    let tx = c.transaction().await?;
                    crate::publisher_authority::check_grants_tx(&tx, permit, &[hash.to_vec()])
                        .await?;
                    tx.commit().await?;
                }
                let (blob, bytes) = self.storage.snapshot(hash, staging_owner).await?;
                let descriptor = page.clone();
                crate::node::spawn_blocking_owned_scope(move || {
                    manifest::validate_image(&bytes, &descriptor).map_err(anyhow::Error::msg)
                })
                .await??;
                ensure!(
                    !sources.contains_key(&hash),
                    "manifest/image role collision"
                );
                sources.insert(hash, (blob, page.mime.clone()));
            }
        }
        Ok(PreparedPublication {
            event: event.clone(),
            event_version,
            authority,
            staging_owner,
            lease: None,
            manifest: manifest_id,
            chapter,
            series_metadata,
            sources,
        })
    }
    pub async fn commit(&self, prepared: PreparedPublication) -> Result<bool> {
        let event_id = id(&prepared.event.id)?;
        let owner = prepared.staging_owner;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        if let Some(lease) = &prepared.lease {
            crate::db::check_lease_tx(&tx, lease).await?;
        }
        crate::submission_review::lock_event(&tx, event_id).await?;
        crate::submission_review::check_publication_verdict(&tx, event_id).await?;
        check_authority(&tx, &prepared.authority).await?;
        let row = tx
            .query_one(
                "SELECT version,state,manifest FROM events WHERE id=$1 FOR UPDATE",
                &[&event_id.as_slice()],
            )
            .await?;
        ensure!(
            matches!(row.get::<_, String>(1).as_str(), "candidate" | "hosted"),
            crate::transfer_rejection::TransferRejection::policy(
                crate::transfer_rejection::RejectionReason::LocalRemoval
            )
        );
        ensure!(
            row.get::<_, i64>(0) == prepared.event_version
                && row.get::<_, Option<Vec<u8>>>(2) == Some(prepared.manifest.to_vec()),
            "publication generation changed during preparation"
        );
        let state: String = row.get(1);
        ensure!(
            matches!(state.as_str(), "candidate" | "hosted"),
            "publication no longer eligible"
        );
        if state == "hosted" {
            if let Authority::Review(permit) = &prepared.authority {
                crate::submission_review::complete_review_tx(&tx, permit).await?;
                tx.commit().await?;
            }
            return Ok(false);
        }
        let hashes: Vec<Vec<u8>> = prepared.sources.keys().map(|h| h.to_vec()).collect();

        if let Authority::Review(permit) = &prepared.authority {
            crate::submission_review::check_review_grants_tx(&tx, permit, &hashes).await?;
        }
        let rows=tx.query("SELECT hash,generation,owner,size,state FROM blob_intents WHERE hash=ANY($1) ORDER BY hash FOR SHARE", &[&hashes]).await?;
        if let Authority::Group { group, .. } = &prepared.authority {
            let grants=tx.query("SELECT g.hash FROM group_blob_grants g JOIN blob_intents b ON b.hash=g.hash AND b.generation=g.generation WHERE g.group_key=$1 AND g.hash=ANY($2) AND g.lease_until>clock_timestamp() ORDER BY g.hash FOR SHARE OF g", &[&group.as_slice(),&hashes]).await?;
            ensure!(
                grants.len() == hashes.len(),
                "group lacks a staging grant for an original or manifest"
            );
        }
        if let Authority::Local(permit) = &prepared.authority
            && permit.publisher != owner
        {
            crate::publisher_authority::check_grants_tx(&tx, permit, &hashes).await?;
        }
        ensure!(
            rows.len() == prepared.sources.len(),
            "original disappeared during preparation"
        );
        ensure!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM quarantined_blobs WHERE hash=ANY($1))",
                &[&hashes]
            )
            .await?
            .get::<_, bool>(0),
            "original quarantined during preparation"
        );
        for row in &rows {
            let hash: Id = row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("invalid stored hash")?;
            let (expected, _) = &prepared.sources[&hash];
            ensure!(
                row.get::<_, Vec<u8>>(1) == expected.key.generation
                    && row.get::<_, Vec<u8>>(2) == owner
                    && row.get::<_, i64>(3) == expected.size as i64
                    && row.get::<_, String>(4) == "available",
                "original generation/state changed during preparation"
            );
        }
        ensure!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=ANY($1))",
                &[&hashes]
            )
            .await?
            .get::<_, bool>(0),
            "publication object reclamation pending"
        );
        retain_blobs_tx(&tx, &hashes, owner).await?;
        for (hash, (blob, mime)) in &prepared.sources {
            tx.execute(
                "INSERT INTO publication_blobs(event_id,hash,generation,mime) VALUES($1,$2,$3,$4)",
                &[
                    &event_id.as_slice(),
                    &hash.as_slice(),
                    &blob.key.generation.as_slice(),
                    &mime,
                ],
            )
            .await?;
        }
        let (series, language, chapter, pages, size) = if let Some(m) = &prepared.chapter {
            let total = m
                .pages
                .iter()
                .try_fold(0i64, |sum, p| {
                    sum.checked_add(i64::try_from(p.size).ok()?)
                        .filter(|v| *v >= 0)
                })
                .context("chapter byte total overflow")?;
            (
                Some(id(&m.series)?.to_vec()),
                Some(m.language.clone()),
                m.chapter.clone(),
                Some(m.pages.len() as i64),
                Some(total),
            )
        } else {
            (None, None, None, None, None)
        };
        tx.execute("UPDATE events SET state='hosted',series=$2,language=$3,chapter=$4,pages=$5,size=$6 WHERE id=$1", &[&event_id.as_slice(),&series,&language,&chapter,&pages,&size]).await?;
        let metadata = if let Some(m) = &prepared.series_metadata {
            serde_json::to_value(m)?
        } else if let Some(m) = &prepared.chapter {
            serde_json::json!({"series":m.series,"language":m.language,"chapter":m.chapter,"volume":m.volume,"title":m.title,"pages":m.pages})
        } else {
            serde_json::json!({})
        };
        tx.execute("INSERT INTO catalogue_metadata(event_id,metadata,title_values,metadata_version) VALUES($1,$2,$3,2) ON CONFLICT(event_id) DO UPDATE SET metadata=EXCLUDED.metadata,title_values=EXCLUDED.title_values,metadata_version=2,indexed_at=clock_timestamp()",&[&event_id.as_slice(),&metadata,&crate::catalogue::title_values(&metadata)]).await?;
        crate::mangadex_compat::index_tx(&tx, &prepared.event, &metadata).await?;
        if let Some(m) = &prepared.chapter {
            tx.execute(
                "UPDATE events SET volume=$2,title=$3 WHERE id=$1",
                &[&event_id.as_slice(), &m.volume, &m.title],
            )
            .await?;
        }
        if let Authority::Group {
            group,
            member,
            version,
        } = &prepared.authority
        {
            crate::groups::record_action_tx(&tx, *group, *member, *version, "publish", event_id)
                .await?;
        }

        tx.execute("INSERT INTO backend_outbox(event_id,action,payload,event_version) VALUES($1,'import',$2,$3)", &[&event_id.as_slice(),&serde_json::to_value(&prepared.event)?,&prepared.event_version]).await?;
        if let Some(lease) = &prepared.lease {
            crate::db::check_lease_tx(&tx, lease).await?;
        }
        if let Authority::Review(permit) = &prepared.authority {
            crate::submission_review::complete_review_tx(&tx, permit).await?;
        }
        tx.commit().await?;
        Ok(true)
    }
    pub async fn publish_with_permit(
        &self,
        event: &Event,
        permit: crate::publisher_authority::PublisherPermit,
    ) -> Result<bool> {
        self.commit(self.prepare_with_permit(event, permit).await?)
            .await
    }
    pub async fn publish(&self, event: &Event) -> Result<bool> {
        self.commit(self.prepare(event).await?).await
    }
}
