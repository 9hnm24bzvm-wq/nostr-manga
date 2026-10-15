//! Signed root admission and delta retention. Indexed per-author holds avoid a
//! single counter row becoming the write lock for every shared object.
use crate::{
    accounts::{AccountPermit, StageOutcome},
    auth::Event,
    db::Id,
    library_manifest::{self, Limits, Root},
    library_references::PreparedReferences,
    library_storage::LibraryObjects,
    storage_accounting::retain_blobs_tx,
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy, Debug)]
pub enum ExpectedHead {
    Any,
    Absent,
    Exact(Id),
}
#[derive(Debug, thiserror::Error)]
#[error("library head changed")]
pub struct HeadConflict;
struct Head {
    time: i64,
    winner: Vec<u8>,
    current: Option<Vec<u8>>,
}
fn head(row: tokio_postgres::Row) -> Head {
    Head {
        time: row.get(0),
        winner: row.get(1),
        current: row.get(2),
    }
}
fn decide(
    old: Option<&Head>,
    created: i64,
    id: &[u8],
    expected: ExpectedHead,
) -> Result<Option<StageOutcome>> {
    let current = old.and_then(|h| h.current.as_deref());
    if current == Some(id) {
        return Ok(Some(StageOutcome::Current));
    }
    let matches = match expected {
        ExpectedHead::Any => true,
        ExpectedHead::Absent => current.is_none(),
        ExpectedHead::Exact(want) => current == Some(want.as_slice()),
    };
    if !matches {
        return Err(HeadConflict.into());
    }
    if let Some(h) = old
        && (created < h.time || (created == h.time && id >= h.winner.as_slice()))
    {
        return Ok(Some(StageOutcome::Superseded));
    }
    Ok(None)
}
/// New HTTP intent commits require strict CAS even when the signed event is
/// already current. The original native commit's idempotent ordering is intact.
fn strict_expected(old: Option<&Head>, expected: ExpectedHead) -> Result<()> {
    let current = old.and_then(|head| head.current.as_deref());
    let matches = match expected {
        ExpectedHead::Any => false,
        ExpectedHead::Absent => current.is_none(),
        ExpectedHead::Exact(id) => current == Some(id.as_slice()),
    };
    if !matches {
        return Err(HeadConflict.into());
    }
    Ok(())
}
impl LibraryObjects {
    /// Internal final boundary for an already intent-validated HTTP mutation.
    /// Only the original mutation module may select its intent/root; callers
    /// cannot manufacture or clone the request capability. No HTTP route calls
    /// this entry directly.
    pub(crate) async fn commit_admitted_http(
        self: &Arc<Self>,
        capability: crate::http_auth::AdmittedHttpCapability,
        event: Event,
        expected: ExpectedHead,
        targets: crate::library_http_mutation::MutationTargets,
    ) -> Result<StageOutcome> {
        ensure!(capability.method() == "POST", "library mutation method");
        ensure!(
            !matches!(expected, ExpectedHead::Any),
            "explicit library head required"
        );
        let permit = capability.account_permit().clone();
        self.commit_request(permit, event, expected, Some((capability, targets)))
            .await
    }
    pub async fn commit(
        self: &Arc<Self>,
        permit: AccountPermit,
        event: Event,
        expected: ExpectedHead,
    ) -> Result<StageOutcome> {
        self.commit_request(permit, event, expected, None).await
    }
    async fn commit_request(
        self: &Arc<Self>,
        permit: AccountPermit,
        event: Event,
        expected: ExpectedHead,
        guard: Option<(
            crate::http_auth::AdmittedHttpCapability,
            crate::library_http_mutation::MutationTargets,
        )>,
    ) -> Result<StageOutcome> {
        ensure!(
            event.content.len() <= 512
                && event.tags.len() <= 128
                && event
                    .tags
                    .iter()
                    .all(|t| t.len() <= 16 && t.iter().all(|v| v.len() <= 1024)),
            "library pointer field budget"
        );
        ensure!(
            serde_json::to_vec(&event)?.len() <= 16_384,
            "library pointer byte budget"
        );
        let root_hash = library_manifest::pointer(&event)?;
        ensure!(
            event.pubkey == hex::encode(permit.publisher),
            "library author mismatch"
        );
        let mut waiting = crate::perf::Trace::new("commit_queue");
        let slot = crate::library_admission::wait(self.commits.clone()).await?;
        waiting.mark("commit_slot");
        drop(waiting);
        let this = self.clone();
        crate::node::spawn_owned_scope(async move {
            let _slot = slot;
            this.commit_inner(permit, event, expected, root_hash, guard)
                .await
        })
        .await?
    }
    async fn commit_inner(
        self: Arc<Self>,
        permit: AccountPermit,
        event: Event,
        expected: ExpectedHead,
        root_hash: Id,
        guard: Option<(
            crate::http_auth::AdmittedHttpCapability,
            crate::library_http_mutation::MutationTargets,
        )>,
    ) -> Result<StageOutcome> {
        let mut trace = crate::perf::Trace::new("root");
        self.repo.verify_account_storage_permit(&permit).await?;
        let event_id = hex::decode(&event.id)?;
        let created = i64::try_from(event.created_at)?;
        let probe=self.repo.pool.get().await?.query_opt("SELECT winner_at,winner_id,event_id FROM account_heads WHERE publisher=$1 AND kind=11999", &[&permit.publisher.as_slice()]).await?.map(head);
        if guard.is_some() {
            strict_expected(probe.as_ref(), expected)?;
        }
        if let Some(result) = decide(probe.as_ref(), created, &event_id, expected)? {
            // A new adapter receipt cannot claim an immutable old/current or
            // superseded event applied the requested delta. Native semantics stay.
            if guard.is_some() {
                return Err(HeadConflict.into());
            }
            return Ok(result);
        }
        let previous_validation = self
            .validation
            .previous(
                permit.publisher,
                probe.as_ref().and_then(|head| head.current.as_deref()),
            )
            .await;
        trace.mark("precheck");
        // All filesystem and canonical root decoding work precedes the transaction.
        let bytes = self.read(&permit, root_hash).await?;
        let root_size = bytes.len() as u64;
        let root = if let Some(root) = previous_validation
            .as_ref()
            .and_then(|previous| previous.decoded_root(root_hash, root_size))
        {
            trace.count("root_decode_reused", 1);
            root
        } else {
            trace.count("root_decode_reused", 0);
            Arc::new(
                crate::node::spawn_blocking_owned_scope(move || {
                    Root::decode(&bytes, &Limits::default())
                })
                .await??,
            )
        };
        let owner = self.storage.owner();
        let prepared = PreparedReferences::prepare(
            &self.repo,
            permit.publisher,
            owner,
            root_hash,
            root_size,
            root,
            previous_validation.as_deref(),
        )
        .await?;
        let raw = serde_json::to_value(&event)?;
        trace.mark("read_decode");
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        if let Some((capability, _)) = &guard {
            tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL lock_timeout='500ms'")
                .await?;
            capability.recheck_service_transaction(&tx).await?;
        }
        let _authority = tx
            .query_one(
                &tx.prepare_cached(
                    "SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR UPDATE",
                )
                .await?,
                &[&permit.publisher.as_slice()],
            )
            .await?;
        crate::accounts::check_account_authority_tx(&tx, &permit).await?;
        if let Some((capability, _)) = &guard {
            capability.recheck_transaction(&tx).await?;
        }
        let old=tx.query_opt(&tx.prepare_cached("SELECT winner_at,winner_id,event_id FROM account_heads WHERE publisher=$1 AND kind=11999").await?, &[&permit.publisher.as_slice()]).await?.map(head);
        if guard.is_some() {
            strict_expected(old.as_ref(), expected)?;
            // Raw CAS distinguishes a quarantined/hidden head from absence;
            // the signed current-public policy must agree before any mutation.
            let visible = self
                .repo
                .current_library_head_transaction(&tx, permit.publisher)
                .await?;
            let public_id = visible.as_ref().map(|head| head.event.id.as_str());
            let expected_id = match expected {
                ExpectedHead::Exact(id) => Some(hex::encode(id)),
                ExpectedHead::Absent => None,
                ExpectedHead::Any => unreachable!("guarded explicit head"),
            };
            if public_id != expected_id.as_deref() {
                return Err(HeadConflict.into());
            }
            guard.as_ref().unwrap().1.check_transaction(&tx).await?;
        }
        if let Some(result) = decide(old.as_ref(), created, &event_id, expected)? {
            if guard.is_some() {
                return Err(HeadConflict.into());
            }
            return Ok(result);
        }
        crate::ordinary_deletion::admit_event_tx(&tx, &event).await?;
        trace.mark("author_lock");
        let objects = prepared.lock_and_recheck(&tx, permit.publisher).await?;
        trace.mark("validate_refs");
        let old_rows=tx.query(&tx.prepare_cached("SELECT hash,generation FROM library_object_holds WHERE publisher=$1 AND state='current' ORDER BY hash").await?, &[&permit.publisher.as_slice()]).await?;
        let mut previous = BTreeMap::<Id, [u8; 16]>::new();
        for row in old_rows {
            previous.insert(
                row.get::<_, Vec<u8>>(0)
                    .try_into()
                    .ok()
                    .context("hold hash")?,
                row.get::<_, Vec<u8>>(1)
                    .try_into()
                    .ok()
                    .context("hold generation")?,
            );
        }
        let old_event = old.as_ref().and_then(|h| h.current.as_ref());
        ensure!(
            previous.is_empty() == old_event.is_none(),
            "current holds without a library head"
        );
        let mut added_hashes = Vec::new();
        let mut added_generations = Vec::new();
        for (hash, generation) in objects {
            if let Some(old_generation) = previous.get(hash) {
                ensure!(old_generation == generation, "retained generation changed");
            } else {
                added_hashes.push(hash.to_vec());
                added_generations.push(generation.to_vec());
            }
        }
        // Current holds were created atomically after staging credit transfer.
        // Reusing that exact generation cannot need another transfer. New holds
        // still pass the full durable receipt/reservation/refund lifecycle.
        if !added_hashes.is_empty() {
            retain_blobs_tx(&tx, &added_hashes, owner).await?;
        }
        trace.count("retention_objects", added_hashes.len() as u64);
        trace.mark("retain_credits");
        tx.execute(&tx.prepare_cached("INSERT INTO events(id,publisher,kind,created_at,raw_event,state,manifest) VALUES($1,$2,11999,$3,$4,'hosted',$5)").await?, &[&event_id,&permit.publisher.as_slice(),&created,&raw,&root_hash.as_slice()]).await?;
        tx.execute(
            &tx.prepare_cached(
                "INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'import',$2)",
            )
            .await?,
            &[&event_id, &raw],
        )
        .await?;
        if !added_hashes.is_empty() {
            tx.execute(&tx.prepare_cached(
                "INSERT INTO library_object_holds(publisher,hash,generation,state) SELECT $1,added.hash,added.generation,'current' FROM unnest($2::bytea[],$3::bytea[]) added(hash,generation) ON CONFLICT(publisher,hash,generation) DO UPDATE SET state='current',wait_event=NULL,grace_until=NULL"
            ).await?, &[&permit.publisher.as_slice(), &added_hashes, &added_generations]).await?;
        }
        if let Some(old_event) = old_event {
            let removed: Vec<_> = previous
                .iter()
                .filter(|(hash, _)| !objects.contains_key(*hash))
                .collect();
            let removed_hashes: Vec<Vec<u8>> =
                removed.iter().map(|(hash, _)| hash.to_vec()).collect();
            let removed_generations: Vec<Vec<u8>> = removed
                .iter()
                .map(|(_, generation)| generation.to_vec())
                .collect();
            if !removed.is_empty() {
                let changed = tx.execute(&tx.prepare_cached(
                    "UPDATE library_object_holds h SET state='pending',wait_event=$2 FROM unnest($3::bytea[],$4::bytea[]) removed(hash,generation) WHERE h.publisher=$1 AND h.hash=removed.hash AND h.generation=removed.generation AND h.state='current'"
                ).await?, &[&permit.publisher.as_slice(), &old_event, &removed_hashes, &removed_generations]).await?;
                ensure!(
                    changed as usize == removed.len(),
                    "library hold transition mismatch"
                );
            }
            ensure!(tx.execute(&tx.prepare_cached("UPDATE events SET state='withdrawing',version=version+1 WHERE id=$1 AND state='hosted'").await?, &[&old_event]).await?==1,"library predecessor state mismatch");
            tx.execute(&tx.prepare_cached("INSERT INTO backend_outbox(event_id,action,payload,event_version) SELECT id,'delete',raw_event,version FROM events WHERE id=$1").await?, &[&old_event]).await?;
        }
        tx.execute(&tx.prepare_cached("INSERT INTO account_heads(publisher,kind,winner_at,winner_id,event_id) VALUES($1,11999,$2,$3,$3) ON CONFLICT(publisher,kind) DO UPDATE SET winner_at=EXCLUDED.winner_at,winner_id=EXCLUDED.winner_id,event_id=EXCLUDED.event_id").await?, &[&permit.publisher.as_slice(),&created,&event_id]).await?;
        crate::accounts::check_account_lease_tx(&tx, &permit).await?;
        tx.commit().await?;
        drop(client);
        trace.mark("write_commit");
        self.validation
            .remember(
                permit.publisher,
                event_id.as_slice().try_into().context("event identifier")?,
                prepared,
            )
            .await;
        trace.mark("remember_validation");
        Ok(StageOutcome::Accepted)
    }
}
pub(crate) async fn retire(tx: &deadpool_postgres::Transaction<'_>, event: &[u8]) -> Result<()> {
    // Immediate hold→blob generation FK and the caller's author fence keep this
    // set stable. Discover and lock the exact blobs in one ordered round trip.
    let rows = tx.query(&tx.prepare_cached(
        "SELECT b.hash FROM library_object_holds h JOIN blob_intents b ON b.hash=h.hash AND b.generation=h.generation WHERE h.wait_event=$1 AND h.state='pending' ORDER BY b.hash FOR SHARE OF b"
    ).await?, &[&event]).await?;
    if rows.is_empty() {
        return Ok(());
    }
    tx.execute(&tx.prepare_cached("WITH retired AS (UPDATE library_object_holds SET state='grace',wait_event=NULL,grace_until=clock_timestamp()+interval '5 minutes' WHERE wait_event=$1 AND state='pending' RETURNING hash,generation,grace_until) INSERT INTO blob_gc_queue(hash,generation,due_at) SELECT hash,generation,max(grace_until) FROM retired GROUP BY hash,generation ON CONFLICT(hash,generation) DO UPDATE SET due_at=GREATEST(blob_gc_queue.due_at,EXCLUDED.due_at),source_mask=blob_gc_queue.source_mask|1").await?, &[&event]).await?;
    Ok(())
}

impl LibraryObjects {
    async fn public_scope(&self, hash: Id) -> Result<(Id, [u8; 16], bool)> {
        use crate::read_diagnostics::{Phase, during, phase};
        let row = {
            let client = during(Phase::PoolCheckout, self.repo.pool.get()).await?;
            during(Phase::DbRoundtrip, client.query_opt("SELECT b.owner,b.generation,EXISTS(SELECT 1 FROM library_chunk_summaries s WHERE s.hash=b.hash AND s.generation=b.generation) FROM blob_intents b WHERE b.hash=$1 AND b.state='available' AND NOT EXISTS(SELECT 1 FROM quarantined_blobs q WHERE q.hash=b.hash AND q.generation=b.generation) AND (EXISTS(SELECT 1 FROM library_object_holds h WHERE h.hash=b.hash AND h.generation=b.generation AND h.state IN ('current','pending')) OR EXISTS(SELECT 1 FROM library_object_holds h WHERE h.hash=b.hash AND h.generation=b.generation AND h.state='grace' AND h.grace_until>statement_timestamp()))", &[&hash.as_slice()])).await?
        }.context("library object is not publicly retained")?;
        let _decode = phase(Phase::DecodeResponse);
        Ok((
            row.get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("object owner")?,
            row.get::<_, Vec<u8>>(1)
                .try_into()
                .ok()
                .context("object generation")?,
            row.get(2),
        ))
    }
    pub async fn public_object(self: &Arc<Self>, hash: Id) -> Result<Vec<u8>> {
        use crate::read_diagnostics::{Phase, during, phase};
        // Snapshot owns its independent read permit through filesystem I/O.
        // An unrelated staged write must not consume library read admission.
        let (owner, generation, is_chunk) = self.public_scope(hash).await?;
        let (intent, bytes) = self.storage.snapshot(hash, owner).await?;
        ensure!(
            intent.key.generation == generation,
            "library generation changed"
        );
        if !is_chunk {
            let _decode = phase(Phase::DecodeResponse);
            let root = Root::decode(&bytes, &Limits::default())?;
            let mut children = BTreeMap::new();
            for family in root.families {
                for d in family.chunks {
                    if !d.is_empty() {
                        children.insert(d.hash, d.size);
                    }
                }
            }
            let hashes: Vec<Vec<u8>> = children.keys().map(|h| h.to_vec()).collect();
            let rows = {
                let client = during(Phase::PoolCheckout, self.repo.pool.get()).await?;
                during(Phase::DbRoundtrip, client.query("SELECT b.hash,b.size FROM blob_intents b WHERE b.hash=ANY($1) AND b.state='available' AND NOT EXISTS(SELECT 1 FROM quarantined_blobs q WHERE q.hash=b.hash AND q.generation=b.generation)", &[&hashes])).await?
            };
            ensure!(
                rows.len() == children.len(),
                "library root has unhealthy or missing children"
            );
            for row in rows {
                let h: Id = row
                    .get::<_, Vec<u8>>(0)
                    .try_into()
                    .ok()
                    .context("child hash")?;
                ensure!(
                    u64::try_from(row.get::<_, i64>(1))? == children[&h],
                    "library child size changed"
                );
            }
        }
        ensure!(
            self.public_scope(hash).await?.1 == generation,
            "library visibility changed during read"
        );
        Ok(bytes)
    }
}

impl LibraryObjects {
    pub(crate) async fn public_sizes(&self, hashes: &[Id]) -> Result<BTreeMap<Id, u64>> {
        use crate::read_diagnostics::{Phase, during, phase};
        ensure!(
            !hashes.is_empty() && hashes.len() <= 64,
            "public size lookup budget"
        );
        let keys: Vec<Vec<u8>> = hashes.iter().map(|h| h.to_vec()).collect();
        let rows = {
            let client = during(Phase::PoolCheckout, self.repo.pool.get()).await?;
            during(Phase::DbRoundtrip, client.query("SELECT b.hash,b.size FROM blob_intents b WHERE b.hash=ANY($1) AND b.state='available' AND NOT EXISTS(SELECT 1 FROM quarantined_blobs q WHERE q.hash=b.hash AND q.generation=b.generation) AND (EXISTS(SELECT 1 FROM library_object_holds h WHERE h.hash=b.hash AND h.generation=b.generation AND h.state IN ('current','pending')) OR EXISTS(SELECT 1 FROM library_object_holds h WHERE h.hash=b.hash AND h.generation=b.generation AND h.state='grace' AND h.grace_until>statement_timestamp()))", &[&keys])).await?
        };
        let _decode = phase(Phase::DecodeResponse);
        let mut sizes = BTreeMap::new();
        for row in rows {
            let hash: Id = row
                .get::<_, Vec<u8>>(0)
                .try_into()
                .ok()
                .context("public hash")?;
            let size = u64::try_from(row.get::<_, i64>(1))?;
            ensure!(size <= 1_000_000, "library object size");
            sizes.insert(hash, size);
        }
        ensure!(
            hashes.iter().all(|h| sizes.contains_key(h)),
            "public object unavailable"
        );
        Ok(sizes)
    }
}
