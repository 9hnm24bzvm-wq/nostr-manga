//! Bounded read-only reading statuses over the single original signed graph.
use crate::{
    db::Id,
    http_auth::AdmittedHttpCapability,
    library_manifest::{Family, Record},
    library_storage::LibraryObjects,
    library_view::ViewBudget,
};
use anyhow::{Result, ensure};
use std::sync::Arc;
pub struct ReadingStatusSnapshot {
    pub publisher: Id,
    pub head: Option<String>,
    pub root: Option<Id>,
    pub entries: Vec<SeriesStatus>,
}
pub struct SeriesStatus {
    pub series: Id,
    pub status: String,
    pub updated_at: u64,
}
impl LibraryObjects {
    pub async fn public_reading_statuses(
        self: &Arc<Self>,
        capability: &AdmittedHttpCapability,
        series: Option<Id>,
        status: Option<&str>,
        expected_head: Option<&str>,
    ) -> Result<ReadingStatusSnapshot> {
        ensure!(
            status.is_none_or(|s| [
                "reading",
                "on_hold",
                "plan_to_read",
                "dropped",
                "re_reading",
                "completed"
            ]
            .contains(&s)),
            ViewBudget("reading_status")
        );
        let _read = crate::library_admission::wait(self.reads.clone()).await?;
        let publisher = capability.principal();
        let Some(view) = self.public_view(publisher, expected_head).await? else {
            return Ok(ReadingStatusSnapshot {
                publisher,
                head: None,
                root: None,
                entries: vec![],
            });
        };
        let records = if let Some(series) = series {
            self.view_lookup(&view, Family::Series, &[series.to_vec()])
                .await?
        } else {
            let page=self.view_filtered(&view,Family::Series,move|r|matches!(r,Record::Series{status:s,..}if status.is_none_or(|wanted|wanted==s)),0,10001).await?;
            ensure!(page.total <= 10000, ViewBudget("statuses_count"));
            page.records
        };
        let entries = records
            .into_iter()
            .filter_map(|r| match r {
                Record::Series {
                    id,
                    status,
                    updated_at,
                } => Some(SeriesStatus {
                    series: id,
                    status,
                    updated_at,
                }),
                _ => None,
            })
            .collect();
        self.verify_view(&view).await?;
        Ok(ReadingStatusSnapshot {
            publisher,
            head: Some(view.event.id),
            root: Some(view.root_hash),
            entries,
        })
    }
}
