//! Signer-free validated payload preparation for multi-recipient library delivery.
//! Transport and durable retry state are deliberately separate.
use crate::{
    auth::{self, Event},
    db::Id,
    library_manifest::{Chunk, Limits, Root},
    library_wire,
};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct SaveBundle {
    event: Event,
    objects: Vec<(&'static str, Id, Vec<u8>)>,
}
pub struct ObjectBatch {
    pub body: Vec<u8>,
    pub hashes: Vec<Id>,
    pub next_object: usize,
}
impl SaveBundle {
    /// Exact complete graph only. No unreferenced object can piggyback on a save.
    pub fn validate(
        event: Event,
        root_bytes: Vec<u8>,
        chunks: BTreeMap<Id, Vec<u8>>,
    ) -> Result<Self> {
        ensure!(
            serde_json::to_vec(&event)?.len() <= 16384,
            "library event budget"
        );
        let root_hash = crate::library_manifest::pointer(&event)?;
        ensure!(
            auth::hash(&root_bytes) == hex::encode(root_hash),
            "root hash mismatch"
        );
        let limits = Limits::default();
        let root = Root::decode(&root_bytes, &limits)?;
        let required: BTreeSet<Id> = root
            .families
            .iter()
            .flat_map(|f| f.chunks.iter())
            .filter(|d| !d.is_empty())
            .map(|d| d.hash)
            .collect();
        ensure!(
            chunks.len() == required.len() && chunks.keys().all(|h| required.contains(h)),
            "missing or unreferenced library chunks"
        );
        let mut bytes = root_bytes.len() as u64;
        let mut verified = HashMap::new();
        for (hash, body) in &chunks {
            bytes = bytes
                .checked_add(body.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("byte total overflow"))?;
            ensure!(
                bytes <= limits.max_total_bytes + limits.max_root_bytes as u64,
                "complete graph byte budget"
            );
            ensure!(auth::hash(body) == hex::encode(hash), "chunk hash mismatch");
            let (_, summary) = Chunk::decode(body, &limits)?;
            verified.insert(*hash, summary);
        }
        root.validate_chunks(&verified, &limits)?;
        let mut objects: Vec<_> = chunks
            .into_iter()
            .map(|(hash, bytes)| ("chunk", hash, bytes))
            .collect();
        objects.push(("root", root_hash, root_bytes));
        Ok(Self { event, objects })
    }
    pub fn event(&self) -> &Event {
        &self.event
    }
    pub fn objects(&self) -> impl Iterator<Item = (&str, Id, &[u8])> {
        self.objects
            .iter()
            .map(|(kind, hash, bytes)| (*kind, *hash, bytes.as_slice()))
    }
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
    pub fn commit_body(&self) -> Result<Vec<u8>> {
        // Omitted expected_head uses deterministic Nostr replacement semantics.
        Ok(serde_json::to_vec(&json!({"event":self.event}))?)
    }
    /// At most one bounded encoded batch is allocated at a time. Retry with the
    /// same cursor for identical object content, but fresh request authorization.
    pub fn batch(&self, start: usize) -> Result<Option<ObjectBatch>> {
        ensure!(start <= self.objects.len(), "object cursor out of range");
        if start == self.objects.len() {
            return Ok(None);
        }
        let mut objects = vec![];
        let mut hashes = vec![];
        let mut raw = 0usize;
        for (kind, hash, bytes) in self
            .objects
            .iter()
            .skip(start)
            .take(library_wire::MAX_OBJECTS)
        {
            if !objects.is_empty() && raw + bytes.len() > 4 * 1024 * 1024 {
                break;
            }
            raw += bytes.len();
            hashes.push(*hash);
            objects.push(json!({"type":kind,"data":STANDARD.encode(bytes)}));
        }
        let body = serde_json::to_vec(&json!({"objects":objects}))?;
        ensure!(body.len() <= library_wire::MAX_WIRE, "encoded batch budget");
        Ok(Some(ObjectBatch {
            body,
            next_object: start + hashes.len(),
            hashes,
        }))
    }
}

/// NIP65 write routes, not inferred HTTP API origins or proof of service identity.
/// An explicit route-to-service binding is still required by the transport.
pub fn write_relays(event: &Event, owner: Id) -> Result<Vec<String>> {
    ensure!(
        serde_json::to_vec(event)?.len() <= 16384,
        "relay-list event budget"
    );
    event.verify().map_err(anyhow::Error::msg)?;
    ensure!(
        event.kind == 10002 && event.pubkey == hex::encode(owner),
        "relay-list identity/kind"
    );
    ensure!(
        event.created_at <= auth::now().saturating_add(60),
        "relay-list future timestamp"
    );
    let mut seen = BTreeSet::new();
    let mut routes = vec![];
    for tag in &event.tags {
        if tag.first().is_none_or(|t| t != "r") {
            continue;
        }
        if !(tag.len() == 2 || (tag.len() == 3 && tag[2] == "write")) {
            continue;
        }
        let url = url::Url::parse(&tag[1])?;
        ensure!(
            matches!(url.scheme(), "ws" | "wss")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none(),
            "invalid relay route"
        );
        let normalized = url.to_string();
        if seen.insert(normalized.clone()) {
            routes.push(normalized);
        }
        ensure!(routes.len() <= 32, "write relay budget");
    }
    Ok(routes)
}
