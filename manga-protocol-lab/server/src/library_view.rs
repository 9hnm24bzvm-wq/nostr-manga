//! Bounded convenience views over the existing signed public-library graph.
//! No second authoritative library, private library, or unsigned mutation path.
use crate::{
    auth::Event,
    db::Id,
    library_manifest::{Chunk, Family, Limits, Record, Root},
    library_storage::LibraryObjects,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("public library snapshot changed")]
pub struct SnapshotChanged;

#[derive(Debug, thiserror::Error)]
#[error("public library view budget: {0}")]
pub struct ViewBudget(pub &'static str);

pub struct View {
    pub publisher: Id,
    pub event: Event,
    pub root_hash: Id,
    pub root: Root,
}
pub struct Page {
    pub records: Vec<Record>,
    pub chunks: Vec<Id>,
    pub total: u64,
}
pub fn family(name: &str) -> Result<Family> {
    Family::ALL
        .into_iter()
        .find(|f| f.name() == name)
        .context("unknown library family")
}
pub fn record_json(record: &Record) -> Value {
    match record {
        Record::Chapter {
            id,
            completed,
            page,
            updated_at,
        } => {
            json!({"id":hex::encode(id),"completed":completed,"page":page,"updated_at":updated_at})
        }
        Record::Series {
            id,
            status,
            updated_at,
        } => json!({"id":hex::encode(id),"status":status,"updated_at":updated_at}),
        Record::List {
            id,
            name,
            updated_at,
        } => json!({"id":hex::encode(id),"name":name,"updated_at":updated_at}),
        Record::Member {
            list,
            series,
            position,
            updated_at,
        } => {
            json!({"list":hex::encode(list),"series":hex::encode(series),"position":position,"updated_at":updated_at})
        }
    }
}
impl LibraryObjects {
    pub async fn public_view(
        self: &Arc<Self>,
        publisher: Id,
        expected: Option<&str>,
    ) -> Result<Option<View>> {
        let Some(head) = self.repo.current_library_head(publisher).await? else {
            if expected.is_some() {
                return Err(SnapshotChanged.into());
            }
            return Ok(None);
        };
        let event = (*head.event).clone();
        if expected.is_some_and(|id| id != event.id) {
            return Err(SnapshotChanged.into());
        }
        let root_hash = crate::library_manifest::pointer(&event)?;
        let bytes = self.public_object(root_hash).await?;
        let root = crate::read_diagnostics::during(
            crate::read_diagnostics::Phase::DecodeResponse,
            crate::node::spawn_blocking_owned_scope(move || Root::decode(&bytes, &Limits::default())),
        )
        .await??;
        let view = View {
            publisher,
            event,
            root_hash,
            root,
        };
        self.verify_view(&view).await?;
        Ok(Some(view))
    }
    pub async fn verify_view(&self, view: &View) -> Result<()> {
        if self
            .repo
            .current_library_head(view.publisher)
            .await?
            .is_none_or(|head| head.event.id != view.event.id)
        {
            return Err(SnapshotChanged.into());
        }
        Ok(())
    }
    async fn view_chunk(
        self: &Arc<Self>,
        view: &View,
        family: Family,
        index: usize,
    ) -> Result<Vec<Record>> {
        let f = view
            .root
            .families
            .iter()
            .find(|f| f.family == family)
            .context("missing family")?;
        let d = f.chunks.get(index).context("bucket index")?;
        if d.is_empty() {
            return Ok(vec![]);
        }
        let bytes = self.public_object(d.hash).await?;
        ensure!(bytes.len() as u64 == d.size, "chunk size mismatch");
        let (chunk, summary) = crate::read_diagnostics::during(
            crate::read_diagnostics::Phase::DecodeResponse,
            crate::node::spawn_blocking_owned_scope(move || Chunk::decode(&bytes, &Limits::default())),
        )
        .await??;
        ensure!(
            summary.hash() == d.hash
                && summary.records() == d.records
                && summary.fits(family, f.chunks.len(), index),
            "chunk descriptor mismatch"
        );
        Ok(chunk.records().to_vec())
    }
    /// Stable bucket/key order within an explicitly identified current snapshot.
    pub async fn view_page(
        self: &Arc<Self>,
        view: &View,
        family: Family,
        offset: u64,
        limit: usize,
    ) -> Result<Page> {
        if limit > 100 {
            return Err(ViewBudget("page_limit").into());
        }
        let f = view
            .root
            .families
            .iter()
            .find(|f| f.family == family)
            .context("missing family")?;
        let total = f.chunks.iter().map(|d| d.records).sum();
        let mut page = Page {
            records: vec![],
            chunks: vec![],
            total,
        };
        let mut skip = offset;
        let mut bytes = 0u64;
        for (index, d) in f.chunks.iter().enumerate() {
            if page.records.len() == limit {
                break;
            }
            if skip >= d.records {
                skip -= d.records;
                continue;
            }
            bytes = bytes.checked_add(d.size).context("view size overflow")?;
            if bytes > 16 * 1024 * 1024 {
                return Err(ViewBudget("scan_bytes").into());
            }
            let records = self.view_chunk(view, family, index).await?;
            page.chunks.push(d.hash);
            page.records.extend(
                records
                    .into_iter()
                    .skip(skip as usize)
                    .take(limit - page.records.len()),
            );
            skip = 0;
        }
        self.verify_view(view).await?;
        Ok(page)
    }
    /// Bounded filtered view for public list membership and reading-state adapters.
    pub async fn view_filtered<F>(
        self: &Arc<Self>,
        view: &View,
        family: Family,
        predicate: F,
        offset: u64,
        limit: usize,
    ) -> Result<Page>
    where
        F: Fn(&Record) -> bool + Send + Sync,
    {
        if limit > 20000 {
            return Err(ViewBudget("filtered_limit").into());
        }
        let f = view
            .root
            .families
            .iter()
            .find(|f| f.family == family)
            .context("missing family")?;
        let mut page = Page {
            records: vec![],
            chunks: vec![],
            total: 0,
        };
        let mut bytes = 0u64;
        for (index, d) in f.chunks.iter().enumerate() {
            if d.is_empty() {
                continue;
            }
            bytes = bytes.checked_add(d.size).context("view size overflow")?;
            if bytes > 64 * 1024 * 1024 {
                return Err(ViewBudget("scan_bytes").into());
            }
            page.chunks.push(d.hash);
            for record in self.view_chunk(view, family, index).await? {
                if predicate(&record) {
                    if page.total >= offset && page.records.len() < limit {
                        page.records.push(record)
                    }
                    page.total += 1;
                }
            }
        }
        self.verify_view(view).await?;
        Ok(page)
    }
    /// Exact-key lookups touch only the relevant hash buckets, never infer aliases.
    pub async fn view_lookup(
        self: &Arc<Self>,
        view: &View,
        family: Family,
        keys: &[Vec<u8>],
    ) -> Result<Vec<Record>> {
        if keys.len() > 5000 {
            return Err(ViewBudget("lookup_keys").into());
        }
        let f = view
            .root
            .families
            .iter()
            .find(|f| f.family == family)
            .context("missing family")?;
        let wanted: std::collections::BTreeSet<_> = keys.iter().cloned().collect();
        let buckets = keys
            .iter()
            .map(|key| crate::library_manifest::bucket_for(key, f.chunks.len()))
            .collect::<Result<std::collections::BTreeSet<_>>>()?;
        let mut bytes = 0u64;
        let mut found = vec![];
        for index in buckets {
            bytes = bytes
                .checked_add(f.chunks[index].size)
                .context("lookup size overflow")?;
            if bytes > 64 * 1024 * 1024 {
                return Err(ViewBudget("scan_bytes").into());
            }
            found.extend(
                self.view_chunk(view, family, index)
                    .await?
                    .into_iter()
                    .filter(|record| wanted.contains(&record.key())),
            );
        }
        self.verify_view(view).await?;
        Ok(found)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn family_and_records_preserve_exact_public_identifiers() {
        assert_eq!(family("chapters").unwrap(), Family::Chapters);
        assert!(family("Chapters").is_err());
        let value = record_json(&Record::Chapter {
            id: [7; 32],
            completed: false,
            page: Some(3),
            updated_at: 8,
        });
        assert_eq!(value["id"], hex::encode([7; 32]));
        assert_eq!(value["completed"], false);
        assert_eq!(value["page"], 3);
    }
}
