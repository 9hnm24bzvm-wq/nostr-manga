//! Author requests are evidence, not local deletion authorization.
use crate::{
    auth::{self, Event},
    db::{Id, Repository},
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;
fn id(text: &str) -> Result<Id> {
    ensure!(auth::valid_hash(text), "invalid deletion identifier");
    Ok(hex::decode(text)?.try_into().unwrap())
}
impl Repository {
    pub async fn record_deletion_request(&self, event: &Event) -> Result<bool> {
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(event.kind == 5, "not a deletion request");
        let targets = event
            .values("e")
            .into_iter()
            .map(id)
            .collect::<Result<BTreeSet<_>>>()?;
        ensure!(
            !targets.is_empty() && targets.len() <= 100,
            "deletion target budget"
        );
        let raw = serde_json::to_value(event)?;
        ensure!(
            serde_json::to_vec(&raw)?.len() <= 1048576,
            "request size budget"
        );
        let request = id(&event.id)?;
        let author = id(&event.pubkey)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let inserted=tx.execute("INSERT INTO deletion_requests(id,author,raw_event) VALUES($1,$2,$3) ON CONFLICT(id) DO NOTHING", &[&request.as_slice(),&author.as_slice(),&raw]).await?==1;
        if inserted {
            for target in targets {
                tx.execute(
                    "INSERT INTO deletion_targets(request_id,event_id) VALUES($1,$2)",
                    &[&request.as_slice(), &target.as_slice()],
                )
                .await?;
            }
        }
        tx.commit().await?;
        Ok(inserted)
    }
    /// Local operator approval only. A subset allows valid known targets to be
    /// handled independently of missing or inapplicable targets in the same request.
    pub async fn approve_deletion(&self, request: Id, selected: Option<Vec<Id>>) -> Result<usize> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let author: Vec<u8> = tx
            .query_one(
                "SELECT author FROM deletion_requests WHERE id=$1",
                &[&request.as_slice()],
            )
            .await?
            .get(0);
        let rows=tx.query("SELECT event_id,approved_generation FROM deletion_targets WHERE request_id=$1 ORDER BY event_id FOR UPDATE", &[&request.as_slice()]).await?;
        let selected = selected.map(|v| v.into_iter().collect::<BTreeSet<_>>());
        if let Some(selected) = &selected {
            ensure!(
                !selected.is_empty() && selected.len() <= 100,
                "approval target budget"
            );
            for target in selected {
                ensure!(
                    rows.iter().any(|r| r.get::<_, Vec<u8>>(0) == *target),
                    "target is not in signed request"
                );
            }
        }
        let mut changed = 0;
        for row in rows {
            let target: Vec<u8> = row.get(0);
            if selected
                .as_ref()
                .is_some_and(|s| !s.contains(target.as_slice()))
                || row.get::<_, Option<i64>>(1).is_some()
            {
                continue;
            }
            let event = tx
                .query_opt(
                    "SELECT publisher,kind,state,version FROM events WHERE id=$1 FOR UPDATE",
                    &[&target],
                )
                .await?
                .context("requested event is not known locally")?;
            ensure!(
                event.get::<_, Vec<u8>>(0) == author,
                "request author does not own target"
            );
            ensure!(
                matches!(event.get::<_, i32>(1), 1001 | 1002),
                "target requires ordinary Nostr deletion lifecycle"
            );
            let state: String = event.get(2);
            let mut generation: i64 = event.get(3);
            if !matches!(state.as_str(), "withdrawing" | "removed") {
                generation = generation
                    .checked_add(1)
                    .context("event generation overflow")?;
                tx.execute(
                    "UPDATE events SET state='withdrawing',version=$2 WHERE id=$1",
                    &[&target, &generation],
                )
                .await?;
                tx.execute("INSERT INTO backend_outbox(event_id,action,payload,event_version) VALUES($1,'delete','{}'::jsonb,$2)", &[&target,&generation]).await?;
                changed += 1;
            }
            tx.execute("INSERT INTO publication_tombstones(event_id,request_id,generation) VALUES($1,$2,$3) ON CONFLICT(event_id) DO UPDATE SET request_id=EXCLUDED.request_id,generation=EXCLUDED.generation,active=TRUE,approved_at=clock_timestamp(),restored_at=NULL", &[&target,&request.as_slice(),&generation]).await?;
            tx.execute("UPDATE deletion_targets SET approved_at=clock_timestamp(),approved_generation=$3 WHERE request_id=$1 AND event_id=$2", &[&request.as_slice(),&target,&generation]).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
    /// Explicit local restore opens a new candidate generation; it does not skip
    /// validation, re-create references, or publish a revision/equivalence link.
    pub async fn restore_publication(&self, event: Id) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_one(
                "SELECT state,kind,version FROM events WHERE id=$1 FOR UPDATE",
                &[&event.as_slice()],
            )
            .await?;
        ensure!(
            row.get::<_, String>(0) == "removed" && matches!(row.get::<_, i32>(1), 1001 | 1002),
            "only completed protected removal can be restored"
        );
        let pending:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM backend_outbox WHERE event_id=$1 AND completed_at IS NULL)", &[&event.as_slice()]).await?.get(0);
        ensure!(!pending, "withdrawal projection still pending");
        let next = row
            .get::<_, i64>(2)
            .checked_add(1)
            .context("event generation overflow")?;
        tx.execute(
            "UPDATE events SET state='candidate',version=$2 WHERE id=$1",
            &[&event.as_slice(), &next],
        )
        .await?;
        tx.execute("UPDATE publication_tombstones SET active=FALSE,restored_at=clock_timestamp() WHERE event_id=$1", &[&event.as_slice()]).await?;
        tx.commit().await?;
        Ok(())
    }
}
