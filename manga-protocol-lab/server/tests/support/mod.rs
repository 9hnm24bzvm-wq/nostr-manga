#![allow(dead_code)]
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::auth;
use serde_json::{Value, json};
pub fn sign_at(
    key: u8,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    created: u64,
) -> auth::Event {
    let mut e = auth::sign(&format!("{key:064x}"), kind, tags, content).unwrap();
    e.created_at = created;
    let digest = e.digest();
    e.id = hex::encode(&digest);
    let secp = secp256k1::Secp256k1::new();
    let kp = secp256k1::Keypair::from_secret_key(
        &secp,
        &secp256k1::SecretKey::from_slice(&hex::decode(format!("{key:064x}")).unwrap()).unwrap(),
    );
    e.sig = secp
        .sign_schnorr_no_aux_rand(
            &secp256k1::Message::from_digest_slice(&digest).unwrap(),
            &kp,
        )
        .to_string();
    e
}
pub fn token(e: &auth::Event) -> String {
    format!("Nostr {}", STANDARD.encode(serde_json::to_vec(e).unwrap()))
}
pub fn http(key: u8, url: &str, body: &[u8]) -> String {
    token(
        &auth::sign(
            &format!("{key:064x}"),
            27235,
            vec![
                vec!["u".into(), url.into()],
                vec!["method".into(), "POST".into()],
                vec!["payload".into(), auth::hash(body)],
                vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
            ],
            String::new(),
        )
        .unwrap(),
    )
}
pub fn blossom(key: u8, origin: &str, action: &str, hash: &str) -> String {
    token(&sign_at(
        key,
        24242,
        vec![
            vec!["t".into(), action.into()],
            vec!["server".into(), origin.into()],
            vec!["x".into(), hash.into()],
            vec!["expiration".into(), (auth::now() + 120).to_string()],
        ],
        "Authorize fixture transfer".into(),
        auth::now() - 1,
    ))
}
pub fn public(key: u8) -> String {
    auth::sign(&format!("{key:064x}"), 1, vec![], String::new())
        .unwrap()
        .pubkey
}
pub fn cbor(value: Value) -> Vec<u8> {
    fn sorted(v: ciborium::Value) -> ciborium::Value {
        match v {
            ciborium::Value::Map(items) => {
                let mut items: Vec<_> = items
                    .into_iter()
                    .map(|(k, v)| {
                        let mut encoded = vec![];
                        ciborium::into_writer(&k, &mut encoded).unwrap();
                        (encoded, k, sorted(v))
                    })
                    .collect();
                items.sort_by(|a, b| a.0.cmp(&b.0));
                ciborium::Value::Map(items.into_iter().map(|(_, k, v)| (k, v)).collect())
            }
            ciborium::Value::Array(v) => {
                ciborium::Value::Array(v.into_iter().map(sorted).collect())
            }
            other => other,
        }
    }
    let mut raw = vec![];
    ciborium::into_writer(&value, &mut raw).unwrap();
    let value: ciborium::Value = ciborium::from_reader(raw.as_slice()).unwrap();
    let mut result = vec![];
    ciborium::into_writer(&sorted(value), &mut result).unwrap();
    result
}
pub fn png() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([23u8, 47, 79, 255]));
    let mut data = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut data, image::ImageFormat::Png)
        .unwrap();
    data.into_inner()
}
pub fn page(bytes: &[u8]) -> Value {
    json!({"sha256":auth::hash(bytes),"size":bytes.len(),"mime":"image/png","width":2,"height":2})
}
pub fn publication(key: u8, kind: u32, manifest: &[u8]) -> auth::Event {
    auth::sign(
        &format!("{key:064x}"),
        kind,
        vec![],
        json!({"manifest":auth::hash(manifest),"version":1}).to_string(),
    )
    .unwrap()
}
pub fn write_key(path: &std::path::Path, key: u8) -> anyhow::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    write!(options.open(path)?, "{key:064x}")?;
    Ok(())
}
pub fn http_get(key: u8, url: &str) -> String {
    token(
        &auth::sign(
            &format!("{key:064x}"),
            27235,
            vec![
                vec!["u".into(), url.into()],
                vec!["method".into(), "GET".into()],
                vec!["nonce".into(), hex::encode(rand::random::<[u8; 16]>())],
            ],
            String::new(),
        )
        .unwrap(),
    )
}
pub async fn post_json(
    client: &reqwest::Client,
    key: u8,
    url: &str,
    value: &serde_json::Value,
) -> anyhow::Result<reqwest::Response> {
    let body = serde_json::to_vec(value)?;
    Ok(client
        .post(url)
        .header("authorization", http(key, url, &body))
        .body(body)
        .send()
        .await?)
}
pub async fn upload_to(
    client: &reqwest::Client,
    key: u8,
    origin: &str,
    data: &[u8],
) -> anyhow::Result<String> {
    let hash = auth::hash(data);
    let r = client
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(key, origin, "upload", &hash))
        .body(data.to_vec())
        .send()
        .await?;
    anyhow::ensure!(r.status() == 200, "upload failed: {}", r.text().await?);
    Ok(hash)
}
pub async fn wait_projected(
    client: &reqwest::Client,
    origin: &str,
    id: &str,
) -> anyhow::Result<()> {
    for _ in 0..200 {
        if client
            .get(format!("{origin}/events/{id}"))
            .send()
            .await?
            .status()
            == 200
        {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    anyhow::bail!("projection did not complete")
}

/// Two sequential public import acknowledgements establish a preceding batch
/// boundary. One event's acknowledgement does not imply its sibling retirements
/// have completed: verified imports may progress while an author is blocked.
pub async fn drain_backend(repo: &manga_node::db::Repository) -> anyhow::Result<()> {
    for _ in 0..2 {
        let marker = sign_at(
            250,
            1,
            vec![],
            format!(
                "projection drain {}",
                hex::encode(rand::random::<[u8; 16]>())
            ),
            auth::now(),
        );
        repo.stage_regular_event(&marker).await?;
        let id: [u8; 32] = hex::decode(&marker.id)?.try_into().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                if repo.projection_complete(id).await? {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await??;
    }
    Ok(())
}

/// Observe exact accounted capacity through real upload admission and receipt-
/// backed deletion, never grant-table counters. Use fixtures whose staging limit
/// is at least the physical limit; existing retained content remains untouched.
pub async fn assert_storage_used(
    repo: &manga_node::db::Repository,
    store: &std::sync::Arc<manga_node::upload::UploadStore>,
    physical_limit: u64,
    expected_used: u64,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        expected_used <= physical_limit,
        "invalid fixture expectation"
    );
    let publisher = [249; 32];
    let permit = match repo.publisher_permit(publisher).await {
        Ok(permit) => permit,
        Err(_) => {
            repo.set_publisher_admission(publisher, true).await?;
            repo.publisher_permit(publisher).await?
        }
    };
    let mut free = physical_limit - expected_used;
    let mut fillers = Vec::new();
    while free > 0 {
        let size = free.min(store.max_blob_bytes) as usize;
        let mut bytes = vec![0; size];
        let nonce: [u8; 32] = rand::random();
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = nonce[index % nonce.len()];
        }
        let hash: [u8; 32] = hex::decode(auth::hash(&bytes))?.try_into().unwrap();
        store.put_for_publisher(&permit, hash, &bytes).await?;
        fillers.push(hash);
        free -= size as u64;
    }
    let mut probe = None;
    for value in 0..=255 {
        let byte = [value];
        let hash: [u8; 32] = hex::decode(auth::hash(&byte))?.try_into().unwrap();
        if repo.blob_intent(hash).await?.is_none() {
            probe = Some((hash, byte));
            break;
        }
    }
    let (hash, byte) = probe.expect("fixture has an unused one-byte object");
    assert!(
        store.put_for_publisher(&permit, hash, &byte).await.is_err(),
        "physical admission exceeded the expected remaining capacity"
    );
    for hash in fillers {
        store
            .delete_staged(
                manga_node::staged_deletion::DeleteAuthority::Publisher(permit.clone()),
                hash,
            )
            .await?;
    }
    Ok(())
}

/// Synchronize an external database fault without depending on application SQL
/// spelling, private counters, or scheduler sleeps as the completion oracle.
pub async fn wait_for_blocked_session(
    repo: &manga_node::db::Repository,
    blocker: i32,
) -> anyhow::Result<i32> {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if let Some(row) = repo.pool.get().await?.query_opt(
                "SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1", &[&blocker],
            ).await? { return Ok::<_, anyhow::Error>(row.get(0)); }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await?
}

/// Locate a synthetic group key for external backup/corruption fixture setup.
/// This duplicates the documented key-file layout, not a production test API.
pub fn recovered_key_path(root: &std::path::Path, group: [u8; 32]) -> std::path::PathBuf {
    root.join(format!("{}.key", hex::encode(group)))
}
