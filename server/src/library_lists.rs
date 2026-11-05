//! Existing signed public-list views only. RECONSTRUCTED UNVERIFIED after reset.
use crate::{
    db::Id,
    library_manifest::{Family, Record},
    library_storage::LibraryObjects,
    library_view::{SnapshotChanged, ViewBudget},
};
use anyhow::{Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub struct PublicListSnapshot {
    pub publisher: Id,
    pub head: Option<String>,
    pub root: Option<Id>,
    pub total: u64,
    pub lists: Vec<PublicList>,
}
pub struct PublicList {
    pub id: [u8; 16],
    pub name: String,
    pub updated_at: u64,
    pub members: Vec<PublicListMember>,
}
pub struct PublicListMember {
    pub series: Id,
    pub position: u64,
}
pub struct ListAliasPage {
    pub publisher: Id,
    pub head: Option<String>,
    pub lists: Vec<[u8; 16]>,
    pub next_offset: Option<u64>,
}
impl LibraryObjects {
    pub async fn public_lists(
        self: &Arc<Self>,
        publisher: Id,
        expected: Option<&str>,
        offset: u64,
        limit: usize,
        target: Option<[u8; 16]>,
    ) -> Result<PublicListSnapshot> {
        ensure!(limit <= 100, ViewBudget("list_limit"));
        let _read = crate::library_admission::wait(self.reads.clone()).await?;
        let Some(view) = self.public_view(publisher, expected).await? else {
            return Ok(PublicListSnapshot {
                publisher,
                head: None,
                root: None,
                total: 0,
                lists: vec![],
            });
        };
        let (records, total) = if let Some(target) = target {
            let r = self
                .view_lookup(&view, Family::Lists, &[target.to_vec()])
                .await?;
            let total = r.len() as u64;
            (r, total)
        } else {
            let p = self.view_page(&view, Family::Lists, offset, limit).await?;
            (p.records, p.total)
        };
        let wanted = records
            .iter()
            .filter_map(|r| match r {
                Record::List { id, .. } => Some(*id),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let members = if wanted.is_empty() {
            vec![]
        } else {
            let page = self
                .view_filtered(
                    &view,
                    Family::Members,
                    move |r| matches!(r,Record::Member{list,..}if wanted.contains(list)),
                    0,
                    5001,
                )
                .await?;
            ensure!(page.total <= 5000, ViewBudget("list_relationships"));
            page.records
        };
        let keys = members
            .iter()
            .filter_map(|r| match r {
                Record::Member { series, .. } => Some(series.to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let visible = self
            .repo
            .pool
            .get()
            .await?
            .query(
                &format!(
                    "SELECT e.id FROM events e WHERE e.id=ANY($1) AND e.kind=1002 AND {}",
                    crate::catalogue::VISIBLE
                ),
                &[&keys],
            )
            .await?
            .into_iter()
            .map(|r| r.get::<_, Vec<u8>>(0))
            .collect::<BTreeSet<_>>();
        let mut by_list: BTreeMap<[u8; 16], Vec<PublicListMember>> = BTreeMap::new();
        for r in members {
            if let Record::Member {
                list,
                series,
                position,
                ..
            } = r
            {
                if visible.contains(series.as_slice()) {
                    by_list
                        .entry(list)
                        .or_default()
                        .push(PublicListMember { series, position });
                }
            }
        }
        let mut lists = vec![];
        for r in records {
            if let Record::List {
                id,
                name,
                updated_at,
            } = r
            {
                let mut members = by_list.remove(&id).unwrap_or_default();
                members.sort_by_key(|m| (m.position, m.series));
                lists.push(PublicList {
                    id,
                    name,
                    updated_at,
                    members,
                });
            }
        }
        self.verify_view(&view).await?;
        Ok(PublicListSnapshot {
            publisher,
            head: Some(view.event.id),
            root: Some(view.root_hash),
            total,
            lists,
        })
    }
    pub async fn next_public_library_owner(&self, after: Option<Id>) -> Result<Option<Id>> {
        let rows=match after{Some(after)=>self.repo.pool.get().await?.query("SELECT publisher FROM account_heads WHERE kind=11999 AND event_id IS NOT NULL AND publisher>$1 ORDER BY publisher LIMIT 1",&[&after.as_slice()]).await?,None=>self.repo.pool.get().await?.query("SELECT publisher FROM account_heads WHERE kind=11999 AND event_id IS NOT NULL ORDER BY publisher LIMIT 1",&[]).await?};
        rows.first()
            .map(|r| {
                r.get::<_, Vec<u8>>(0)
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("stored library owner"))
            })
            .transpose()
    }
    pub async fn public_list_alias_page(
        self: &Arc<Self>,
        publisher: Id,
        expected: Option<&str>,
        offset: u64,
    ) -> Result<ListAliasPage> {
        let _read = crate::library_admission::wait(self.reads.clone()).await?;
        let Some(view) = self.public_view(publisher, expected).await? else {
            return Ok(ListAliasPage {
                publisher,
                head: None,
                lists: vec![],
                next_offset: None,
            });
        };
        let page = self.view_page(&view, Family::Lists, offset, 100).await?;
        let consumed = offset
            .checked_add(page.records.len() as u64)
            .ok_or(ViewBudget("list_cursor"))?;
        ensure!(
            consumed > offset || consumed >= page.total,
            ViewBudget("list_cursor")
        );
        let lists = page
            .records
            .into_iter()
            .filter_map(|r| match r {
                Record::List { id, .. } => Some(id),
                _ => None,
            })
            .collect();
        self.verify_view(&view).await?;
        Ok(ListAliasPage {
            publisher,
            head: Some(view.event.id),
            lists,
            next_offset: (consumed < page.total).then_some(consumed),
        })
    }
    pub async fn verify_public_list_snapshot(&self, snapshot: &PublicListSnapshot) -> Result<()> {
        let current = self.repo.current_library_head(snapshot.publisher).await?;
        if current.as_ref().map(|h| h.event.id.as_str()) != snapshot.head.as_deref() {
            return Err(SnapshotChanged.into());
        }
        Ok(())
    }
}
