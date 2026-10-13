//! Ordinary account/social NIP09 requests never enter strfry as raw kind5.
use crate::{
    accounts::AccountPermit,
    auth::{self, Event},
    db::{Id, Repository},
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;
fn id(s: &str) -> Result<Id> {
    ensure!(auth::valid_hash(s), "invalid event ID");
    Ok(hex::decode(s)?.try_into().unwrap())
}
async fn event_lock(tx: &tokio_postgres::Transaction<'_>, event: &Id) -> Result<()> {
    // Hash collisions only serialize unrelated IDs; never establish authority.
    let lock = i64::from_be_bytes(event[..8].try_into().unwrap());
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&lock])
        .await?;
    Ok(())
}
pub(crate) async fn admit_event_tx(tx: &tokio_postgres::Transaction<'_>, e: &Event) -> Result<()> {
    let event = id(&e.id)?;
    let author = id(&e.pubkey)?;
    let kind = e.kind as i32;
    let created = i64::try_from(e.created_at)?;
    event_lock(tx, &event).await?;
    ensure!(!tx.query_one("SELECT EXISTS(SELECT 1 FROM ordinary_event_tombstones WHERE event_id=$1 AND publisher=$2) OR EXISTS(SELECT 1 FROM account_deletion_cutoffs WHERE publisher=$2 AND kind=$3 AND deleted_through>=$4)",&[&event.as_slice(),&author.as_slice(),&kind,&created]).await?.get::<_,bool>(0),"event covered by author deletion");
    Ok(())
}
impl Repository {
    pub async fn delete_ordinary(&self, permit: &AccountPermit, e: &Event) -> Result<usize> {
        e.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            e.kind == 5 && e.pubkey == hex::encode(permit.publisher),
            "deletion author mismatch"
        );
        ensure!(
            e.tags.len() <= 256
                && e.tags
                    .iter()
                    .all(|t| t.len() <= 16 && t.iter().all(|v| v.len() <= 1024))
                && e.content.len() <= 65536,
            "deletion budget"
        );
        let mut targets = e
            .values("e")
            .into_iter()
            .map(id)
            .collect::<Result<BTreeSet<_>>>()?;
        let mut kinds = BTreeSet::new();
        for address in e.values("a") {
            let mut parts = address.splitn(3, ':');
            let kind = parts.next().context("address kind")?.parse::<i32>()?;
            ensure!(
                matches!(kind, 0 | 3 | 10002 | 11999)
                    && parts.next() == Some(e.pubkey.as_str())
                    && parts.next() == Some(""),
                "only this author's supported replaceable coordinates are accepted"
            );
            kinds.insert(kind);
        }
        ensure!(
            !targets.is_empty() || !kinds.is_empty(),
            "deletion targets required"
        );
        ensure!(targets.len() + kinds.len() <= 100, "deletion target budget");
        let request = id(&e.id)?;
        let created = i64::try_from(e.created_at)?;
        let raw = serde_json::to_value(e)?;
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        let _authority = tx
            .query_one(
                "SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR UPDATE",
                &[&permit.publisher.as_slice()],
            )
            .await?;
        crate::accounts::check_account_authority_tx(&tx, permit).await?;
        tx.execute("INSERT INTO deletion_requests(id,author,raw_event) VALUES($1,$2,$3) ON CONFLICT(id) DO NOTHING",&[&request.as_slice(),&permit.publisher.as_slice(),&raw]).await?;
        tx.execute(
            "INSERT INTO ordinary_deletion_requests(request_id) VALUES($1) ON CONFLICT DO NOTHING",
            &[&request.as_slice()],
        )
        .await?;
        for kind in kinds {
            tx.execute("INSERT INTO account_deletion_cutoffs(publisher,kind,deleted_through,request_id) VALUES($1,$2,$3,$4) ON CONFLICT(publisher,kind) DO UPDATE SET deleted_through=EXCLUDED.deleted_through,request_id=EXCLUDED.request_id WHERE account_deletion_cutoffs.deleted_through<EXCLUDED.deleted_through",&[&permit.publisher.as_slice(),&kind,&created,&request.as_slice()]).await?;
            for row in tx.query("SELECT id FROM events WHERE publisher=$1 AND kind=$2 AND created_at<=$3 AND state='hosted'",&[&permit.publisher.as_slice(),&kind,&created]).await? {targets.insert(row.get::<_,Vec<u8>>(0).try_into().unwrap());}
        }
        // Author lock makes address expansion stable; cap avoids unbounded historical work.
        ensure!(targets.len() <= 100, "expanded deletion target budget");
        for target in &targets {
            event_lock(&tx, target).await?;
        }
        let mut changed = 0;
        for target in targets {
            tx.execute("INSERT INTO deletion_targets(request_id,event_id) VALUES($1,$2) ON CONFLICT DO NOTHING",&[&request.as_slice(),&target.as_slice()]).await?;
            let row = tx
                .query_opt(
                    "SELECT publisher,kind,state FROM events WHERE id=$1 FOR UPDATE",
                    &[&target.as_slice()],
                )
                .await?;
            // Exact-ID tombstones are author-qualified, including unknown events.
            tx.execute("INSERT INTO ordinary_event_tombstones(event_id,publisher,request_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",&[&target.as_slice(),&permit.publisher.as_slice(),&request.as_slice()]).await?;
            let Some(row) = row else { continue };
            if row.get::<_, Vec<u8>>(0) != permit.publisher
                || !matches!(row.get::<_, i32>(1), 0 | 1 | 3 | 7 | 1111 | 10002 | 11999)
                || row.get::<_, String>(2) != "hosted"
            {
                continue;
            }
            if row.get::<_, i32>(1) == 11999 {
                // Current object ownership becomes pending until actual backend deletion ack.
                tx.execute("UPDATE library_object_holds SET state='pending',wait_event=$2 WHERE publisher=$1 AND state='current'",&[&permit.publisher.as_slice(),&target.as_slice()]).await?;
            }
            tx.execute(
                "UPDATE account_heads SET event_id=NULL WHERE publisher=$1 AND event_id=$2",
                &[&permit.publisher.as_slice(), &target.as_slice()],
            )
            .await?;
            tx.execute(
                "UPDATE events SET state='withdrawing',version=version+1 WHERE id=$1",
                &[&target.as_slice()],
            )
            .await?;
            tx.execute("INSERT INTO backend_outbox(event_id,action,payload,event_version) SELECT id,'delete',raw_event,version FROM events WHERE id=$1",&[&target.as_slice()]).await?;
            changed += 1;
        }
        crate::accounts::check_account_lease_tx(&tx, permit).await?;
        tx.commit().await?;
        Ok(changed)
    }
    pub async fn ordinary_deletion_request(
        &self,
        request: Id,
    ) -> Result<Option<serde_json::Value>> {
        Ok(self.pool.get().await?.query_opt("SELECT d.raw_event FROM deletion_requests d JOIN ordinary_deletion_requests o ON o.request_id=d.id WHERE d.id=$1",&[&request.as_slice()]).await?.map(|r|r.get(0)))
    }
}
