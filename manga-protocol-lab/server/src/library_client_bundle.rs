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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_manifest::{Descriptor, Family, FamilyRoot, Record};
    fn payload() -> (Event, Vec<u8>, BTreeMap<Id, Vec<u8>>) {
        let limits = Limits::default();
        let chunk = Chunk::new(
            Family::Chapters,
            vec![Record::Chapter {
                id: [7; 32],
                completed: true,
                page: None,
                updated_at: 1,
            }],
            &limits,
        )
        .unwrap()
        .encode(&limits)
        .unwrap();
        let summary = Chunk::decode(&chunk, &limits).unwrap().1;
        let root = Root {
            families: Family::ALL
                .into_iter()
                .map(|family| FamilyRoot {
                    family,
                    chunks: vec![if family == Family::Chapters {
                        Descriptor::from(&summary)
                    } else {
                        Descriptor::empty()
                    }],
                })
                .collect(),
        }
        .encode(&limits)
        .unwrap();
        let event = auth::sign(
            &format!("{:064x}", 1),
            11999,
            vec![],
            json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        )
        .unwrap();
        (event, root, BTreeMap::from([(summary.hash(), chunk)]))
    }
    #[test]
    fn exact_graph_and_wire_roundtrip() {
        let (event, root, chunks) = payload();
        let bundle = SaveBundle::validate(event.clone(), root, chunks).unwrap();
        assert_eq!(bundle.object_count(), 2);
        let batch = bundle.batch(0).unwrap().unwrap();
        assert_eq!(batch.next_object, 2);
        assert_eq!(library_wire::batch(&batch.body).unwrap().len(), 2);
        assert!(bundle.batch(2).unwrap().is_none());
        assert!(bundle.batch(3).is_err());
        let commit = library_wire::commit(&bundle.commit_body().unwrap()).unwrap();
        assert_eq!(commit.event.id, event.id);
        assert!(commit.objects.is_empty());
    }
    #[test]
    fn corrupt_missing_extra_and_wrong_root_are_rejected() {
        let (event, root, chunks) = payload();
        assert!(SaveBundle::validate(event.clone(), root.clone(), BTreeMap::new()).is_err());
        let mut extra = chunks.clone();
        extra.insert([8; 32], vec![0]);
        assert!(SaveBundle::validate(event.clone(), root.clone(), extra).is_err());
        let mut corrupt = chunks.clone();
        corrupt.values_mut().next().unwrap()[0] ^= 1;
        assert!(SaveBundle::validate(event.clone(), root.clone(), corrupt).is_err());
        let mut wrong = root;
        wrong[0] ^= 1;
        assert!(SaveBundle::validate(event, wrong, chunks).is_err());
    }
    #[test]
    fn nip65_selects_only_author_write_routes() {
        let e = auth::sign(
            &format!("{:064x}", 1),
            10002,
            vec![
                vec!["r".into(), "wss://a.example".into()],
                vec!["r".into(), "wss://a.example/".into(), "write".into()],
                vec!["r".into(), "wss://b.example/relay".into(), "write".into()],
                vec!["r".into(), "wss://c.example".into(), "read".into()],
            ],
            String::new(),
        )
        .unwrap();
        let owner: Id = hex::decode(&e.pubkey).unwrap().try_into().unwrap();
        assert_eq!(
            write_relays(&e, owner).unwrap(),
            ["wss://a.example/", "wss://b.example/relay"]
        );
        assert!(write_relays(&e, [9; 32]).is_err());
        let mut bad = e;
        bad.tags.push(vec!["r".into(), "wss://evil.example".into()]);
        assert!(write_relays(&bad, owner).is_err());
    }
}
