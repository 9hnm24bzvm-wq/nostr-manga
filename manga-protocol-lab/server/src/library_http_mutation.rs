//! Client-signed, explicit-intent extension over the original library pipeline.
//! No server/user signing, parallel state, full-record scan or implicit Any CAS.
use crate::{
    auth::{self, Event},
    db::Id,
    http_auth::{AdmittedHttpCapability, HttpAuthError, HttpRequestPreflight},
    library::{ExpectedHead, HeadConflict},
    library_admission::Receipt,
    library_intent_delta::{DeltaPlan, DeltaStats, Intent},
    library_manifest::{Limits, Root},
    library_storage::{LibraryObjects, ObjectType},
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::{collections::BTreeMap, sync::Arc};

pub const MAX_HTTP_BODY: usize = 1024 * 1024;
#[derive(Debug, thiserror::Error)]
#[error("invalid client-signed library intent: {0}")]
pub struct IntentRejected(pub String);
#[derive(Debug, thiserror::Error)]
#[error("this operation requires the version1 client-signed Nostr library commit extension")]
pub struct SignedCommitRequired;
#[derive(Debug, thiserror::Error)]
#[error("signed library author differs from request principal")]
pub struct PrincipalMismatch;
#[derive(Debug, thiserror::Error)]
#[error("requested manga/chapter is unavailable or belongs to another manga")]
pub struct TargetUnavailable;
fn invalid(error: impl std::fmt::Display) -> anyhow::Error {
    IntentRejected(error.to_string()).into()
}

/// UUID mapping is presentation-only and supplied by the wrapper's typed
/// resolver. Every input spelling must match the signed logical body exactly;
/// original indexed live manga/chapter relationships are independently checked.
pub enum MappedHttpIntent {
    ReadMarkers {
        series: Id,
        chapters: BTreeMap<String, Id>,
    },
    Status {
        series: Id,
    },
}
pub struct HttpMutationReceipt {
    pub event: Event,
    pub projected: bool,
    pub delta: DeltaStats,
}
pub(crate) struct MutationTargets {
    series: Id,
    chapters: Vec<Id>,
}
impl MutationTargets {
    pub(crate) async fn check_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
    ) -> Result<()> {
        let visible = crate::catalogue::VISIBLE;
        let parent = tx.query_opt(&format!("SELECT e.id FROM events e WHERE e.id=$1 AND e.kind=1002 AND {visible} FOR SHARE OF e"), &[&self.series.as_slice()]).await?;
        if parent.is_none() {
            return Err(TargetUnavailable.into());
        }
        if !self.chapters.is_empty() {
            let ids: Vec<Vec<u8>> = self.chapters.iter().map(|id| id.to_vec()).collect();
            let rows = tx.query(&format!("SELECT e.id FROM events e WHERE e.id=ANY($1) AND e.series=$2 AND e.kind=1001 AND {visible} FOR SHARE OF e"), &[&ids, &self.series.as_slice()]).await?;
            if rows.len() != self.chapters.len() {
                return Err(TargetUnavailable.into());
            }
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Extension<'a> {
    version: u32,
    #[serde(borrow)]
    commit: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadBody<'a> {
    #[serde(rename = "chapterIdsRead")]
    read: Option<Vec<String>>,
    #[serde(rename = "chapterIdsUnread")]
    unread: Option<Vec<String>>,
    #[serde(borrow)]
    nostr: Option<Extension<'a>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusBody<'a> {
    // RawValue distinguishes an explicit null from a missing required field.
    #[serde(borrow)]
    status: &'a RawValue,
    #[serde(borrow)]
    nostr: Option<Extension<'a>>,
}
struct Prepared {
    event: Event,
    expected: ExpectedHead,
    objects: Vec<(ObjectType, Vec<u8>)>,
    root: Root,
    chunks: BTreeMap<Id, Vec<u8>>,
    intent: Intent,
    targets: MutationTargets,
}
fn prepare(body: &[u8], mapped: MappedHttpIntent) -> Result<Prepared> {
    ensure!(
        body.len() <= MAX_HTTP_BODY,
        IntentRejected("HTTP body limit".into())
    );
    let (extension, intent, targets) = match mapped {
        MappedHttpIntent::ReadMarkers { series, chapters } => {
            let parsed: ReadBody<'_> = serde_json::from_slice(body).map_err(invalid)?;
            ensure!(
                parsed.read.is_some() || parsed.unread.is_some(),
                IntentRejected("read or unread field required".into())
            );
            let read = parsed.read.unwrap_or_default();
            let unread = parsed.unread.unwrap_or_default();
            let logical = serde_json::to_vec(
                &serde_json::json!({"chapterIdsRead":read,"chapterIdsUnread":unread}),
            )?;
            ensure!(
                logical.len() <= 10240,
                IntentRejected("logical read-marker body exceeds10KB".into())
            );
            let mut requested = read.iter().chain(&unread).cloned().collect::<Vec<_>>();
            requested.sort();
            ensure!(
                requested.len() == chapters.len() && requested.iter().eq(chapters.keys()),
                IntentRejected("UUID mapping differs from signed intent or duplicates".into())
            );
            let read = read.iter().map(|id| chapters[id]).collect::<Vec<_>>();
            let unread = unread.iter().map(|id| chapters[id]).collect::<Vec<_>>();
            let intent = Intent::read_markers(&read, &unread).map_err(invalid)?;
            let mut ids = read.into_iter().chain(unread).collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            (
                parsed.nostr,
                intent,
                MutationTargets {
                    series,
                    chapters: ids,
                },
            )
        }
        MappedHttpIntent::Status { series } => {
            let parsed: StatusBody<'_> = serde_json::from_slice(body).map_err(invalid)?;
            ensure!(
                parsed.status.get().len() + 11 <= 2048,
                IntentRejected("logical status body exceeds2KB".into())
            );
            let status: Option<String> =
                serde_json::from_str(parsed.status.get()).map_err(invalid)?;
            (
                parsed.nostr,
                Intent::status(series, status).map_err(invalid)?,
                MutationTargets {
                    series,
                    chapters: vec![],
                },
            )
        }
    };
    let extension = extension.ok_or(SignedCommitRequired)?;
    ensure!(
        extension.version == 1,
        IntentRejected("unsupported extension version".into())
    );
    // Keep original canonical object and signed event bytes. Its decoder rejects
    // duplicate/unknown fields. Any means missing expected_head and is forbidden.
    let decoded =
        crate::library_wire::commit(extension.commit.get().as_bytes()).map_err(invalid)?;
    ensure!(
        !matches!(decoded.expected, ExpectedHead::Any),
        IntentRejected("explicit expected_head required".into())
    );
    let root_hash = crate::library_manifest::pointer(&decoded.event).map_err(invalid)?;
    let mut root = None;
    let mut chunks = BTreeMap::new();
    for (kind, bytes) in &decoded.objects {
        let hash: Id = hex::decode(auth::hash(bytes))?.try_into().unwrap();
        match kind {
            ObjectType::Root => {
                ensure!(
                    root.is_none() && hash == root_hash,
                    IntentRejected("exactly one signed root object required".into())
                );
                root = Some(Root::decode(bytes, &Limits::default()).map_err(invalid)?);
            }
            ObjectType::Chunk => {
                ensure!(
                    chunks.insert(hash, bytes.clone()).is_none(),
                    IntentRejected("duplicate changed chunk".into())
                );
            }
        }
    }
    Ok(Prepared {
        event: decoded.event,
        expected: decoded.expected,
        objects: decoded.objects,
        root: root.ok_or_else(|| invalid("signed root object required"))?,
        chunks,
        intent,
        targets,
    })
}
impl LibraryObjects {
    /// Preflight permits only bounded receipt allocation. It never signs, stages,
    /// consumes replay, or becomes a write capability.
    pub fn receive_http_intent(
        self: &Arc<Self>,
        preflight: &HttpRequestPreflight,
        declared: Option<usize>,
    ) -> Result<Receipt> {
        let owner = self
            .embedded_operations
            .get()
            .context("original embedded library owner unavailable")?;
        owner.check_receipt(self, preflight.service())?;
        self.admission.receive(declared)
    }
    pub async fn apply_http_intent(
        self: &Arc<Self>,
        capability: AdmittedHttpCapability,
        mapped: MappedHttpIntent,
        body: Vec<u8>,
        receipt: Receipt,
    ) -> Result<HttpMutationReceipt> {
        ensure!(
            capability.method() == "POST"
                && auth::hash(&body) == hex::encode(capability.body_hash()),
            HttpAuthError::Invalid
        );
        let path = url::Url::parse(capability.request_url())?.path().to_owned();
        let suffix = match &mapped {
            MappedHttpIntent::ReadMarkers { .. } => "/read",
            MappedHttpIntent::Status { .. } => "/status",
        };
        ensure!(
            path.starts_with("/api/v1/manga/") && path.ends_with(suffix),
            HttpAuthError::Invalid
        );
        let owner = self
            .embedded_operations
            .get()
            .context("original embedded library owner unavailable")?
            .clone();
        ensure!(
            body.len() <= receipt.limit,
            IntentRejected("declared body receipt exceeded".into())
        );
        owner.check_receipt(self, capability.service())?;
        let this = self.clone();
        owner
            .run(self, async move {
                let processing = this.admission.process(receipt).await?;
                let (prepared, _processing) = crate::node::spawn_blocking_owned_scope(move || {
                    prepare(&body, mapped).map(|prepared| (prepared, processing))
                })
                .await??;
                ensure!(
                    prepared.event.pubkey == hex::encode(capability.principal()),
                    PrincipalMismatch
                );
                let expected = match prepared.expected {
                    ExpectedHead::Exact(id) => Some(hex::encode(id)),
                    ExpectedHead::Absent => None,
                    ExpectedHead::Any => unreachable!(),
                };
                // Compare both raw CAS and current public health before staging.
                let old = this
                    .public_view(capability.principal(), expected.as_deref())
                    .await?;
                this.check_http_mutation_snapshot(
                    &capability,
                    prepared.expected,
                    &prepared.targets,
                )
                .await?;
                let plan = DeltaPlan::prepare(
                    old.as_ref().map(|view| &view.root),
                    &prepared.root,
                    &prepared.intent,
                )
                .map_err(invalid)?;
                let mut old_objects = BTreeMap::new();
                for descriptor in plan.old_descriptors() {
                    old_objects.insert(descriptor.hash, this.public_object(descriptor.hash).await?);
                }
                let delta = crate::node::spawn_blocking_owned_scope(move || {
                    plan.validate(&prepared.chunks, |d| {
                        old_objects
                            .remove(&d.hash)
                            .context("missing affected old chunk")
                    })
                })
                .await?
                .map_err(invalid)?;
                let permit = capability.account_permit().clone();
                for (kind, bytes) in prepared.objects {
                    this.stage(permit.clone(), kind, bytes).await?;
                }
                let event = prepared.event.clone();
                let outcome = this
                    .commit_admitted_http(
                        capability,
                        prepared.event,
                        prepared.expected,
                        prepared.targets,
                    )
                    .await?;
                ensure!(
                    matches!(outcome, crate::accounts::StageOutcome::Accepted),
                    HeadConflict
                );
                let event_id: Id = hex::decode(&event.id)?.try_into().unwrap();
                let projected = this.repo.projection_complete(event_id).await?;
                Ok(HttpMutationReceipt {
                    event,
                    projected,
                    delta,
                })
            })
            .await
    }
    async fn check_http_mutation_snapshot(
        &self,
        capability: &AdmittedHttpCapability,
        expected: ExpectedHead,
        targets: &MutationTargets,
    ) -> Result<()> {
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL lock_timeout='500ms'")
            .await?;
        capability.recheck_transaction(&tx).await?;
        let row = tx
            .query_opt(
                "SELECT event_id FROM account_heads WHERE publisher=$1 AND kind=11999",
                &[&capability.principal().as_slice()],
            )
            .await?;
        let raw: Option<Vec<u8>> = row.and_then(|row| row.get(0));
        let matches = match expected {
            ExpectedHead::Absent => raw.is_none(),
            ExpectedHead::Exact(id) => raw.as_deref() == Some(id.as_slice()),
            ExpectedHead::Any => false,
        };
        if !matches {
            return Err(HeadConflict.into());
        }
        targets.check_transaction(&tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
