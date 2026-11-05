//! Public NIP22 comments and NIP25 reactions on locally known event IDs.
use crate::{
    accounts::{AccountPermit, check_account_authority_tx},
    auth::{self, Event},
    db::{Id, Repository},
};
use anyhow::{Context, Result, ensure};
fn identifier(s: &str) -> Result<Id> {
    ensure!(auth::valid_hash(s), "invalid target ID");
    Ok(hex::decode(s)?.try_into().unwrap())
}
fn one<'a>(e: &'a Event, name: &str) -> Result<&'a str> {
    let values = e.values(name);
    ensure!(values.len() == 1, "exactly one {name} tag required");
    Ok(values[0])
}
struct Targets {
    root: Id,
    parent: Id,
}
fn targets(e: &Event) -> Result<Targets> {
    ensure!(
        matches!(e.kind, 7 | 1111),
        "supported social kinds are7 and1111"
    );
    ensure!(
        e.content.len() <= 65536 && (e.kind != 7 || e.content.len() <= 256),
        "social content budget"
    );
    ensure!(
        e.tags.len() <= 128
            && e.tags
                .iter()
                .all(|t| t.len() <= 16 && t.iter().all(|v| v.len() <= 1024)),
        "social tag budget"
    );
    ensure!(
        e.values("expiration").is_empty(),
        "expiration requires another lifecycle"
    );
    if e.kind == 7 {
        let values = e.values("e");
        let parent = identifier(values.last().context("reaction target required")?)?;
        return Ok(Targets {
            root: parent,
            parent,
        });
    }
    ensure!(
        e.values("A").is_empty()
            && e.values("I").is_empty()
            && e.values("a").is_empty()
            && e.values("i").is_empty(),
        "this profile uses exact manga event IDs"
    );
    let root = identifier(one(e, "E")?)?;
    let parent = identifier(one(e, "e")?)?;
    for tag in ["P", "p"] {
        identifier(one(e, tag)?)?;
    }
    for tag in ["K", "k"] {
        one(e, tag)?
            .parse::<u32>()
            .context("numeric kind required")?;
    }
    Ok(Targets { root, parent })
}
pub(crate) fn replication_dependencies(event: &Event) -> Result<(Option<Id>, Vec<Id>)> {
    if event.kind == 1 {
        return Ok((None, vec![]));
    }
    let target = targets(event)?;
    Ok((
        if event.kind == 1111 {
            Some(target.root)
        } else {
            None
        },
        vec![target.parent],
    ))
}
impl Repository {
    pub async fn stage_social(&self, permit: &AccountPermit, event: &Event) -> Result<bool> {
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            event.pubkey == hex::encode(permit.publisher),
            "social author mismatch"
        );
        let target = targets(event)?;
        let id = identifier(&event.id)?;
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        check_account_authority_tx(&tx, permit).await?;
        crate::ordinary_deletion::admit_event_tx(&tx, event).await?;
        let ids = vec![target.root.to_vec(), target.parent.to_vec()];
        let rows = tx
            .query(
                "SELECT id,publisher,kind,state FROM events WHERE id=ANY($1) ORDER BY id FOR SHARE",
                &[&ids],
            )
            .await?;
        let root = rows
            .iter()
            .find(|r| r.get::<_, Vec<u8>>(0) == target.root)
            .context("root not known locally")?;
        let parent = rows
            .iter()
            .find(|r| r.get::<_, Vec<u8>>(0) == target.parent)
            .context("parent not known locally")?;
        ensure!(
            rows.iter().all(|r| r.get::<_, String>(3) == "hosted"),
            "target is not hosted"
        );
        if event.kind == 1111 {
            ensure!(
                matches!(root.get::<_, i32>(2), 1001 | 1002),
                "comment root must be a manga publication"
            );
            ensure!(
                one(event, "K")? == root.get::<_, i32>(2).to_string()
                    && one(event, "k")? == parent.get::<_, i32>(2).to_string(),
                "comment kind mismatch"
            );
            ensure!(
                one(event, "P")? == hex::encode(root.get::<_, Vec<u8>>(1))
                    && one(event, "p")? == hex::encode(parent.get::<_, Vec<u8>>(1)),
                "comment author tags mismatch"
            );
            if target.parent != target.root {
                ensure!(
                    parent.get::<_, i32>(2) == 1111,
                    "parent must be comment or root"
                );
                ensure!(
                    tx.query_opt(
                        "SELECT event_id FROM social_targets WHERE event_id=$1 AND root_id=$2",
                        &[&target.parent.as_slice(), &target.root.as_slice()]
                    )
                    .await?
                    .is_some(),
                    "reply belongs to another thread"
                );
            }
        } else {
            if let Some(p) = event.values("p").last() {
                ensure!(
                    *p == hex::encode(parent.get::<_, Vec<u8>>(1)),
                    "reaction author hint mismatch"
                );
            }
            if let Some(k) = event.values("k").last() {
                ensure!(
                    *k == parent.get::<_, i32>(2).to_string(),
                    "reaction kind hint mismatch"
                );
            }
        }
        let raw = serde_json::to_value(event)?;
        let kind = event.kind as i32;
        let created = i64::try_from(event.created_at)?;
        let inserted=tx.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state) VALUES($1,$2,$3,$4,$5,'hosted') ON CONFLICT(id) DO NOTHING",&[&id.as_slice(),&permit.publisher.as_slice(),&kind,&created,&raw]).await?==1;
        if inserted {
            tx.execute("INSERT INTO social_targets(event_id,root_id,parent_id,kind,created_at) VALUES($1,$2,$3,$4,$5)",&[&id.as_slice(),&target.root.as_slice(),&target.parent.as_slice(),&kind,&created]).await?;
            tx.execute(
                "INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'import',$2)",
                &[&id.as_slice(), &raw],
            )
            .await?;
        }
        crate::accounts::check_account_lease_tx(&tx, permit).await?;
        tx.commit().await?;
        Ok(inserted)
    }
    pub async fn social_events(
        &self,
        target: Id,
        limit: i64,
        offset: i64,
        since: i64,
        until: i64,
    ) -> Result<Vec<serde_json::Value>> {
        ensure!(
            (1..=100).contains(&limit)
                && (0..=10000).contains(&offset)
                && since >= 0
                && until >= since,
            "social query bounds"
        );
        Ok(self.pool.get().await?.query("SELECT e.raw_event FROM social_targets s JOIN events e ON e.id=s.event_id WHERE s.root_id=$1 AND s.created_at>=$2 AND s.created_at<=$3 AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL) ORDER BY s.created_at DESC,s.event_id LIMIT $4 OFFSET $5",&[&target.as_slice(),&since,&until,&limit,&offset]).await?.into_iter().map(|r|r.get(0)).collect())
    }
}
