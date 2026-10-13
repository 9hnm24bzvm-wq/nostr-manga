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
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use rand::{RngCore, SeedableRng};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[derive(Clone)]
enum Access {
    Public,
    Bud(String),
    Http(u8),
}
#[derive(Clone)]
struct Route {
    name: &'static str,
    path: String,
    access: Access,
}
impl Route {
    fn token(&self, origin: &str) -> Option<String> {
        match &self.access {
            Access::Public => None,
            Access::Http(actor) => Some(http_get(*actor, &format!("{origin}{}", self.path))),
            Access::Bud(hash) => Some(bud("127.0.0.1", hash)),
        }
    }
    async fn get(&self, c: &reqwest::Client, origin: &str) -> Result<reqwest::Response> {
        let mut request = c.get(format!("{origin}{}", self.path));
        if let Some(token) = self.token(origin) {
            request = request.header("authorization", token);
        }
        Ok(request.send().await?)
    }
}
fn bud(domain: &str, hash: &str) -> String {
    let event = sign_at(
        2,
        24242,
        vec![
            vec!["t".into(), "get".into()],
            vec!["server".into(), domain.into()],
            vec!["x".into(), hash.into()],
            vec!["expiration".into(), (auth::now() + 120).to_string()],
        ],
        "Read fixture original".into(),
        auth::now() - 1,
    );
    format!(
        "Nostr {}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&event).unwrap())
    )
}
async fn stalled(
    bind: std::net::SocketAddr,
    origin: &str,
    route: &Route,
) -> Result<(tokio::net::TcpStream, String)> {
    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_recv_buffer_size(1024)?;
    let mut socket = socket.connect(bind).await?;
    let authorization = route
        .token(origin)
        .map(|v| format!("Authorization: {v}\r\n"))
        .unwrap_or_default();
    socket
        .write_all(
            format!(
                "GET {} HTTP/1.1\r\nHost: {bind}\r\n{authorization}Connection: close\r\n\r\n",
                route.path
            )
            .as_bytes(),
        )
        .await?;
    let mut header = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), async {
        while !header.ends_with(b"\r\n\r\n") {
            header.push(socket.read_u8().await?);
            ensure!(header.len() < 16384, "header budget");
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok((socket, String::from_utf8(header)?))
}
async fn hold(
    bind: std::net::SocketAddr,
    origin: &str,
    route: &Route,
) -> Result<tokio::net::TcpStream> {
    for _ in 0..100 {
        let (socket, header) = stalled(bind, origin, route).await?;
        if header.starts_with("HTTP/1.1 200") {
            eprintln!("holding {}", route.name);
            return Ok(socket);
        }
        ensure!(
            header.starts_with("HTTP/1.1 429"),
            "{}: {header}",
            route.name
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    anyhow::bail!("{} failed to reuse released credits", route.name)
}
async fn upload(c: &reqwest::Client, origin: &str, bytes: &[u8]) -> Result<String> {
    let hash = auth::hash(bytes);
    let response = c
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(2, origin, "upload", &hash))
        .body(bytes.to_vec())
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "upload: {}",
        response.text().await?
    );
    Ok(hash)
}
fn image_bytes(byte_case: bool, seed: u8) -> Result<(Vec<u8>, u32, u32)> {
    let (width, height) = if byte_case {
        (4096, 2560)
    } else {
        (2048, 1024)
    };
    let mut raw = vec![0u8; width as usize * height as usize * 4];
    rand::rngs::StdRng::from_seed([seed; 32]).fill_bytes(&mut raw);
    let image = image::RgbaImage::from_raw(width, height, raw).unwrap();
    let mut output = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image).write_to(&mut output, image::ImageFormat::Png)?;
    Ok((output.into_inner(), width, height))
}
fn chapter(label: &str, bytes: &[u8], width: u32, height: u32) -> (auth::Event, Vec<u8>) {
    let manifest = cbor(
        json!({"version":1,"series":"ad".repeat(32),"language":"en","chapter":label,
        "pages":[{"sha256":auth::hash(bytes),"size":bytes.len(),"mime":"image/png","width":width,"height":height}]}),
    );
    (publication(2, 1001, &manifest), manifest)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL/strfry; mixed public/private HTTP slow readers"]
async fn mixed_routes_share_download_capacity_without_bypassing_policy_or_uploads() -> Result<()> {
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
        .timeout(Duration::from_secs(60))
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

    let byte_case = std::env::var("MANGA_DOWNLOAD_CASE").as_deref() == Ok("bytes");
    for actor in [2, 3] {
        repo.set_account_admission(hex::decode(public(actor))?.try_into().unwrap(), true)
            .await?;
    }
    repo.set_publisher_admission(hex::decode(public(2))?.try_into().unwrap(), true)
        .await?;
    repo.set_peer(hex::decode(public(12))?.try_into().unwrap(), &origin, true)
        .await?;
    let (public_bytes, width, height) = image_bytes(byte_case, 43)?;
    let hash = upload(&c, &origin, &public_bytes).await?;
    let (event, manifest) = chapter("public-original", &public_bytes, width, height);
    upload(&c, &origin, &manifest).await?;
    let response = post_json(
        &c,
        2,
        &format!("{origin}/publications"),
        &serde_json::to_value(&event)?,
    )
    .await?;
    ensure!(
        response.status().is_success(),
        "publish: {}",
        response.text().await?
    );
    let public_url = format!("{origin}/originals/{hash}");
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let response = c.get(&public_url).send().await?;
            if response.status() == 200 {
                assert_eq!(response.bytes().await?.as_ref(), public_bytes);
                break;
            }
            ensure!(
                response.status() == 404,
                "publication readiness: {}",
                response.status()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    let (private_bytes, pw, ph) = image_bytes(byte_case, 47)?;
    let private_hash = auth::hash(&private_bytes);
    let (draft, draft_manifest) = chapter("private-review", &private_bytes, pw, ph);
    let response = post_json(
        &c,
        2,
        &format!("{origin}/submissions"),
        &json!({"event":draft,"manifest":STANDARD.encode(&draft_manifest)}),
    )
    .await?;
    ensure!(
        response.status().is_success(),
        "review declaration: {}",
        response.text().await?
    );
    let private_path = format!("/submissions/{}/blobs/{private_hash}", draft.id);
    let private_url = format!("{origin}{private_path}");
    let response = c
        .post(&private_url)
        .header("authorization", http(2, &private_url, &private_bytes))
        .body(private_bytes.clone())
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "review upload: {}",
        response.text().await?
    );
    let routes = [
        Route {
            name: "standard Blossom",
            path: format!("/{hash}.png"),
            access: Access::Bud(hash.clone()),
        },
        Route {
            name: "public original",
            path: format!("/originals/{hash}"),
            access: Access::Public,
        },
        Route {
            name: "approved peer",
            path: format!("/federation/blobs/{hash}"),
            access: Access::Http(12),
        },
        Route {
            name: "private review",
            path: private_path,
            access: Access::Http(2),
        },
        Route {
            name: "compatibility alias",
            path: format!("/compat/mangadex/data/{}/{}.png", event.id, hash),
            access: Access::Public,
        },
    ];
    // Direct policy checks occur before and during saturation, never inferred
    // from fixtures' database state.
    let peer_url = format!("{origin}{}", routes[2].path);
    assert_eq!(
        c.get(&peer_url)
            .header("authorization", http_get(3, &peer_url))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.get(&private_url)
            .header("authorization", http_get(3, &private_url))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{origin}/{private_hash}.png"))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{origin}/originals/{private_hash}"))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{origin}/{hash}.png"))
            .header("authorization", bud("wrong.example", &hash))
            .send()
            .await?
            .status(),
        401
    );
    for (index, route) in routes.iter().enumerate() {
        let response = route.get(&c, &origin).await?;
        ensure!(
            response.status() == 200,
            "{} ordinary read: {}",
            route.name,
            response.status()
        );
        if index == 3 {
            assert_eq!(response.headers()["cache-control"], "private, no-store");
        }
        assert_eq!(
            response.bytes().await?.as_ref(),
            if index == 3 {
                private_bytes.as_slice()
            } else {
                public_bytes.as_slice()
            },
            "{} byte identity",
            route.name
        );
    }
    let order: &[usize] = if byte_case { &[0, 1, 3] } else { &[0, 1, 2, 3] };
    let mut held = Vec::new();
    for &index in order {
        held.push(hold(bind, &origin, &routes[index]).await?);
    }
    for route in &routes {
        let (socket, header) = stalled(bind, &origin, route).await?;
        drop(socket);
        assert!(
            header.starts_with("HTTP/1.1 429"),
            "{} must share saturated credits: {header}",
            route.name
        );
    }
    assert_eq!(
        c.get(&peer_url)
            .header("authorization", http_get(3, &peer_url))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.get(&private_url)
            .header("authorization", http_get(3, &private_url))
            .send()
            .await?
            .status(),
        404
    );
    upload(
        &c,
        &origin,
        b"upload progresses while every route shares download saturation",
    )
    .await?;
    if byte_case {
        drop(held.remove(0));
        held.push(hold(bind, &origin, &routes[2]).await?);
        drop(held.remove(0));
        held.push(hold(bind, &origin, &routes[4]).await?);
    } else {
        drop(held.pop());
        held.push(hold(bind, &origin, &routes[4]).await?);
    }
    for route in &routes {
        let (socket, header) = stalled(bind, &origin, route).await?;
        drop(socket);
        assert!(
            header.starts_with("HTTP/1.1 429"),
            "{} did not retain replacement credits: {header}",
            route.name
        );
    }
    drop(held);
    // All sockets are released. Every route must recover its exact representation.
    for (index, route) in routes.iter().enumerate() {
        let socket = hold(bind, &origin, route).await?;
        drop(socket);
        let response = route.get(&c, &origin).await?;
        ensure!(
            response.status() == 200,
            "{} recovery: {}",
            route.name,
            response.status()
        );
        assert_eq!(
            response.bytes().await?.as_ref(),
            if index == 3 {
                private_bytes.as_slice()
            } else {
                public_bytes.as_slice()
            }
        );
    }
    eprintln!(
        "mixed-route {} saturation, policy, upload independence and disconnect recovery passed",
        if byte_case { "byte" } else { "count" }
    );
    Ok(())
}
