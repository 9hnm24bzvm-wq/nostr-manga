mod support;
use anyhow::{Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::json;
use std::{
    process::{Child, Command},
    time::Duration,
};
use support::*;
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
use tokio::io::{AsyncReadExt, AsyncWriteExt};
async fn stalled_download(
    bind: std::net::SocketAddr,
    origin: &str,
    hash: &str,
) -> Result<(tokio::net::TcpStream, String)> {
    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_recv_buffer_size(1024)?;
    let mut socket = socket.connect(bind).await?;
    let token = blossom(2, origin, "get", hash);
    socket.write_all(format!("GET /blobs/{hash} HTTP/1.1\r\nHost: {}\r\nAuthorization: {token}\r\nConnection: close\r\n\r\n", bind).as_bytes()).await?;
    let mut header = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !header.ends_with(b"\r\n\r\n") {
            header.push(socket.read_u8().await?);
            ensure!(header.len() < 16384, "response header budget");
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok((socket, String::from_utf8(header)?))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL/strfry and 120-second response deadline"]
async fn downloads_are_bounded_until_disconnect_or_deadline_and_do_not_block_uploads() -> Result<()>
{
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = l.local_addr()?;
    drop(l);
    let origin = format!("http://{bind}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let cfg = dir.path().join("node.json");
    std::fs::write(
        &cfg,
        serde_json::to_vec(
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"groupprofile","physical_bytes":536870912,"staging_bytes":536870912,"max_blob_bytes":67108864,"upload_slots":1},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(
            std::env::var_os("MANGA_TEST_NODE_BIN")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_manga-node").into()),
        )
        .args(["serve", cfg.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?,
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let mut ready = false;
    for _ in 0..100 {
        if c.get(&origin)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    ensure!(
        ready,
        "node failed: {}",
        std::fs::read_to_string(dir.path().join("node.log"))?
    );

    let publisher = hex::decode(public(2))?.try_into().unwrap();
    repo.set_publisher_admission(publisher, true).await?;
    let byte_case = std::env::var("MANGA_DOWNLOAD_CASE").as_deref() == Ok("bytes");
    let bytes = vec![
        37u8;
        if byte_case {
            40 * 1024 * 1024
        } else {
            8 * 1024 * 1024
        }
    ];
    let hash = auth::hash(&bytes);
    let response = c
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(2, &origin, "upload", &hash))
        .body(bytes.clone())
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "seed upload: {}",
        response.text().await?
    );
    let mut held = Vec::new();
    for _ in 0..if byte_case { 3 } else { 4 } {
        let (socket, header) = stalled_download(bind, &origin, &hash).await?;
        ensure!(
            header.starts_with("HTTP/1.1 200"),
            "admitted download: {header}"
        );
        held.push(socket);
    }
    let (overflow, header) = stalled_download(bind, &origin, &hash).await?;
    drop(overflow);
    assert!(
        header.starts_with("HTTP/1.1 429"),
        "excess slow response must fail bounded admission: {header}"
    );
    let probe = b"upload remains independent of blocked downloads";
    let probe_hash = auth::hash(probe);
    let response = c
        .put(format!("{origin}/upload/{probe_hash}"))
        .header("authorization", blossom(2, &origin, "upload", &probe_hash))
        .body(probe.to_vec())
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "upload under download saturation: {}",
        response.text().await?
    );
    drop(held.pop());
    let mut replacement = None;
    for _ in 0..100 {
        let (socket, header) = stalled_download(bind, &origin, &hash).await?;
        if header.starts_with("HTTP/1.1 200") {
            replacement = Some(socket);
            break;
        }
        ensure!(
            header.starts_with("HTTP/1.1 429"),
            "unexpected retry: {header}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    held.push(replacement.expect("disconnect must release download credits"));
    if byte_case {
        drop(held);
        return Ok(());
    }
    tokio::time::sleep(Duration::from_secs(122)).await;
    let url = format!("{origin}/blobs/{hash}");
    let response = c
        .get(&url)
        .header("authorization", blossom(2, &origin, "get", &hash))
        .send()
        .await?;
    assert_eq!(
        response.status(),
        200,
        "deadline must release credits while old sockets remain open"
    );
    assert_eq!(response.bytes().await?.as_ref(), bytes);
    drop(held);
    Ok(())
}
