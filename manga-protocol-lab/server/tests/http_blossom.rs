mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
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
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
async fn post(c: &reqwest::Client, key: u8, url: &str, body: &[u8]) -> Result<reqwest::Response> {
    Ok(c.post(url)
        .header("authorization", http(key, url, body))
        .body(body.to_vec())
        .send()
        .await?)
}
async fn upload(c: &reqwest::Client, key: u8, origin: &str, bytes: &[u8]) -> Result<String> {
    let hash = auth::hash(bytes);
    let r = c
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(key, origin, "upload", &hash))
        .body(bytes.to_vec())
        .send()
        .await?;
    ensure!(r.status().is_success(), "upload: {}", r.text().await?);
    Ok(hash)
}
#[allow(dead_code)]
async fn delete(
    c: &reqwest::Client,
    key: u8,
    origin: &str,
    hash: &str,
) -> Result<reqwest::Response> {
    Ok(c.delete(format!("{origin}/blobs/{hash}"))
        .header("authorization", blossom(key, origin, "delete", hash))
        .send()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and real authenticated HTTP node"]
async fn blossom_routes_preserve_scopes_grants_and_retention() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
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
    let key = dir.path().join("key");
    write_key(&key, 11)?;
    let sock = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = sock.local_addr()?.to_string();
    drop(sock);
    let origin = format!("http://{bind}");
    let config = dir.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":bind,"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"cors_origins":["https://reader.example"],"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"deletehttp","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":1048576},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(&origin).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("node readiness")?;
    for who in [2, 3] {
        repo.set_publisher_admission(id(&public(who)), true).await?;
    }
    let bytes = b"private disposable staging";
    let hash = auth::hash(bytes);
    let endpoint = format!("{origin}/upload");
    let url = format!("{origin}/{hash}.jpg");
    let domain = "127.0.0.1";
    for bad_domain in [None, Some("another.example"), Some(origin.as_str())] {
        let r = c
            .put(&endpoint)
            .header("authorization", bud(2, bad_domain, "upload", Some(&hash)))
            .body(bytes.to_vec())
            .send()
            .await?;
        assert_eq!(r.status(), 401);
        assert_eq!(r.headers()["access-control-allow-origin"], "*");
    }
    let r = c
        .put(&endpoint)
        .header("authorization", bud(2, Some(domain), "upload", Some(&hash)))
        .body(bytes.to_vec())
        .send()
        .await?;
    assert_eq!(r.status(), 201, "{}", r.text().await?);
    let r = c
        .put(&endpoint)
        .header("x-sha-256", &hash)
        .header("authorization", bud(2, Some(domain), "upload", Some(&hash)))
        .body(bytes.to_vec())
        .send()
        .await?;
    assert_eq!(r.status(), 200);
    assert!(
        r.headers()["access-control-expose-headers"]
            .to_str()?
            .contains("x-manga-response-proof")
    );
    let descriptor: Value = r.json().await?;
    assert_eq!(descriptor["url"], format!("{origin}/{hash}.bin"));
    assert_eq!(descriptor["type"], "application/octet-stream");
    assert_eq!(descriptor["size"], bytes.len());
    assert!(descriptor["uploaded"].as_u64().unwrap() <= auth::now());
    assert_eq!(c.get(&url).send().await?.status(), 404);
    let get = bud(2, Some(domain), "get", None);
    let r = c.get(&url).header("authorization", &get).send().await?;
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "application/octet-stream");
    assert_eq!(r.bytes().await?.as_ref(), bytes);
    let r = c.head(&url).header("authorization", &get).send().await?;
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-length"], bytes.len().to_string());
    assert!(r.bytes().await?.is_empty());
    let r = c
        .head(&url)
        .header("authorization", &get)
        .header("range", "bytes=1-4")
        .send()
        .await?;
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-length"], bytes.len().to_string());
    assert!(r.bytes().await?.is_empty());
    let r = c
        .get(&url)
        .header("authorization", &get)
        .header("range", "bytes=1-4")
        .send()
        .await?;
    assert_eq!(r.status(), 206);
    assert_eq!(
        r.headers()["content-range"],
        format!("bytes 1-4/{}", bytes.len())
    );
    assert_eq!(r.bytes().await?.as_ref(), &bytes[1..5]);
    assert_eq!(
        c.get(&url)
            .header("authorization", &get)
            .header("range", "bytes=999-")
            .send()
            .await?
            .status(),
        416
    );
    assert_eq!(
        c.get(&url)
            .header("authorization", bud(3, Some(domain), "get", None))
            .send()
            .await?
            .status(),
        404
    );
    let wrong = "00".repeat(32);
    assert_eq!(
        c.put(&endpoint)
            .header("x-sha-256", &wrong)
            .header(
                "authorization",
                bud(2, Some(domain), "upload", Some(&wrong))
            )
            .body(bytes.to_vec())
            .send()
            .await?
            .status(),
        409
    );
    assert_eq!(
        c.delete(&url)
            .header("authorization", &get)
            .send()
            .await?
            .status(),
        401
    );
    assert_eq!(
        c.delete(&url)
            .header("authorization", bud(3, Some(domain), "delete", Some(&hash)))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.delete(&url)
            .header("authorization", bud(2, Some(domain), "delete", Some(&hash)))
            .send()
            .await?
            .status(),
        200
    );
    assert!(repo.blob_intent(id(&hash)).await?.is_none());
    let parallel = b"concurrent exact-byte admission";
    let ph = auth::hash(parallel);
    let first = c
        .put(&endpoint)
        .header("authorization", bud(2, Some(domain), "upload", Some(&ph)))
        .body(parallel.to_vec());
    let second = c
        .put(&endpoint)
        .header("authorization", bud(2, Some(domain), "upload", Some(&ph)))
        .body(parallel.to_vec());
    let (one, two) = tokio::join!(first.send(), second.send());
    let mut statuses = vec![one?.status().as_u16(), two?.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![200, 201]);
    // Shared publisher interests remain protected on the standard delete route.
    for who in [2, 3] {
        assert!(
            c.put(&endpoint)
                .header(
                    "authorization",
                    bud(who, Some(domain), "upload", Some(&hash))
                )
                .body(bytes.to_vec())
                .send()
                .await?
                .status()
                .is_success()
        );
    }
    assert_eq!(
        c.delete(&url)
            .header("authorization", bud(2, Some(domain), "delete", Some(&hash)))
            .send()
            .await?
            .status(),
        403
    );
    // Hosted originals are public according to read policy, regardless of suffix.
    let image = png();
    let ih = upload(&c, 2, &origin, &image).await?;
    let manifest = cbor(
        json!({"version":1,"series":"77".repeat(32),"language":"en","chapter":"1","pages":[page(&image)]}),
    );
    upload(&c, 2, &origin, &manifest).await?;
    let event = publication(2, 1001, &manifest);
    let r = post(
        &c,
        2,
        &format!("{origin}/publications"),
        &serde_json::to_vec(&event)?,
    )
    .await?;
    assert!(r.status().is_success(), "{}", r.text().await?);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !repo.projection_complete(id(&event.id)).await? {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    let r = c.get(format!("{origin}/{ih}.pdf")).send().await?;
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "image/png");
    assert_eq!(r.bytes().await?.as_ref(), image);
    assert_eq!(
        c.delete(format!("{origin}/{ih}"))
            .header("authorization", bud(2, Some(domain), "delete", Some(&ih)))
            .send()
            .await?
            .status(),
        403
    );
    for path in ["/libraries", "/upload", "/missing/nested", "/admin/status"] {
        let r = c
            .request(reqwest::Method::OPTIONS, format!("{origin}{path}"))
            .header("origin", "https://reader.example")
            .header("access-control-request-method", "POST")
            .header(
                "access-control-request-headers",
                "authorization,content-type",
            )
            .send()
            .await?;
        assert_eq!(r.status(), 204);
        assert_eq!(
            r.headers()["access-control-allow-origin"],
            if path == "/upload" {
                "*"
            } else {
                "https://reader.example"
            }
        );
        assert!(
            r.headers()["access-control-allow-headers"]
                .to_str()?
                .contains("Authorization")
        );
    }
    let r = c
        .post(format!("{origin}/libraries"))
        .header("origin", "https://reader.example")
        .body("{}")
        .send()
        .await?;
    assert!(!r.status().is_success());
    assert_eq!(
        r.headers()["access-control-allow-origin"],
        "https://reader.example"
    );
    let denied = c
        .request(reqwest::Method::OPTIONS, format!("{origin}/libraries"))
        .header("origin", "https://unapproved.example")
        .send()
        .await?;
    assert_eq!(denied.status(), 403);
    assert!(!denied.headers().contains_key("access-control-allow-origin"));
    // A real authorized library staging request works cross-origin and carries a verifiable proof.
    repo.set_account_admission(id(&public(2)), true).await?;
    let limits = manga_node::library_manifest::Limits::default();
    let chunk = manga_node::library_manifest::Chunk::new(
        manga_node::library_manifest::Family::Chapters,
        vec![manga_node::library_manifest::Record::Chapter {
            id: [6; 32],
            completed: true,
            page: None,
            updated_at: 1,
        }],
        &limits,
    )?
    .encode(&limits)?;
    let library_url = format!("{origin}/library/objects/chunk");
    let authorization = http(2, &library_url, &chunk);
    let auth_event = auth::token(&authorization).map_err(anyhow::Error::msg)?;
    let r = c
        .post(&library_url)
        .header("origin", "https://reader.example")
        .header("authorization", authorization)
        .body(chunk)
        .send()
        .await?;
    assert_eq!(r.status(), 200, "{}", r.text().await?);
    assert_eq!(
        r.headers()["access-control-allow-origin"],
        "https://reader.example"
    );
    let proof = r.headers()[manga_node::service_proof::HEADER]
        .to_str()?
        .to_owned();
    let body = r.bytes().await?;
    manga_node::service_proof::verify(
        &proof,
        id(&public(11)),
        &manga_node::service_proof::Scope {
            url: &library_url,
            method: "POST",
            request: &auth_event.id,
            status: 200,
        },
        &body,
    )?;
    // Operational lookup failures are not permanent authorization denials.
    let probe = b"publisher permission database error fixture";
    let probe_hash = auth::hash(probe);
    let db = repo.pool.get().await?;
    db.batch_execute(
        "ALTER TABLE publisher_permissions RENAME TO publisher_permissions_fixture_unavailable",
    )
    .await?;
    let standard = c
        .put(&endpoint)
        .header(
            "authorization",
            bud(2, Some(domain), "upload", Some(&probe_hash)),
        )
        .body(probe.to_vec())
        .send()
        .await;
    let legacy = c
        .put(format!("{origin}/upload/{probe_hash}"))
        .header("authorization", blossom(2, &origin, "upload", &probe_hash))
        .body(probe.to_vec())
        .send()
        .await;
    db.batch_execute(
        "ALTER TABLE publisher_permissions_fixture_unavailable RENAME TO publisher_permissions",
    )
    .await?;
    let statuses = vec![standard?.status().as_u16(), legacy?.status().as_u16()];
    let expected = std::env::var("MANGA_DB_ERROR_EXPECTED")
        .unwrap_or_else(|_| "503".into())
        .parse::<u16>()?;
    let report = std::env::var("MANGA_DB_ERROR_REPORT")
        .unwrap_or_else(|_| "publisher-db-error-after.json".into());
    std::fs::write(
        root.join("results").join(report),
        serde_json::to_vec_pretty(
            &json!({"standard_upload":statuses[0],"legacy_upload":statuses[1],"expected":expected}),
        )?,
    )?;
    assert_eq!(statuses, vec![expected, expected]);
    assert!(repo.blob_intent(id(&probe_hash)).await?.is_none());
    db.batch_execute(
        "ALTER TABLE publisher_blob_grants RENAME TO publisher_blob_grants_fixture_unavailable",
    )
    .await?;
    let failed_read = c
        .get(&url)
        .header("authorization", bud(2, Some(domain), "get", None))
        .send()
        .await;
    db.batch_execute(
        "ALTER TABLE publisher_blob_grants_fixture_unavailable RENAME TO publisher_blob_grants",
    )
    .await?;
    assert_eq!(
        failed_read?.status(),
        503,
        "database lookup failure must not masquerade as missing bytes"
    );
    assert_eq!(
        c.get(format!("{origin}/{}", "ff".repeat(32)))
            .header("authorization", bud(1, Some(domain), "get", None))
            .send()
            .await?
            .status(),
        404
    );
    // Restart the same instance with an admitted-reader policy. New root blob
    // routes must not become a bypass around the ordinary application guard.
    drop(node);
    let mut restricted: Value = serde_json::from_slice(&std::fs::read(&config)?)?;
    restricted["read_policy"] = json!("admitted");
    std::fs::write(&config, serde_json::to_vec(&restricted)?)?;
    let log = std::fs::File::create(dir.path().join("restricted.log"))?;
    let _restricted_node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(&origin).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("restricted readiness")?;
    let hosted = format!("{origin}/{ih}");
    assert_eq!(c.get(&hosted).send().await?.status(), 401);
    assert_eq!(
        c.get(&hosted)
            .header("authorization", bud(3, Some(domain), "get", None))
            .send()
            .await?
            .status(),
        403
    );
    assert_eq!(
        c.get(&hosted)
            .header("authorization", bud(2, Some(domain), "get", None))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        c.get(&hosted)
            .header("authorization", bud(2, None, "get", None))
            .send()
            .await?
            .status(),
        401
    );
    std::fs::write(
        root.join("results/http-blossom.json"),
        serde_json::to_vec_pretty(
            &json!({"mandatory_server_scope":true,"cross_domain_replay_rejected":true,"optional_upload_hash_header":true,"created_then_existing_status":true,"staged_privacy":true,"head_and_range":true,"shared_and_hosted_delete_protected":true,"cors_on_success_errors_and_preflight":true,"descriptor":descriptor}),
        )?,
    )?;
    Ok(())
}
fn bud(key: u8, server: Option<&str>, action: &str, hash: Option<&str>) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let mut tags = vec![
        vec!["t".into(), action.into()],
        vec!["expiration".into(), (auth::now() + 120).to_string()],
    ];
    if let Some(server) = server {
        tags.push(vec!["server".into(), server.into()]);
    }
    if let Some(hash) = hash {
        tags.push(vec!["x".into(), hash.into()]);
    }
    format!(
        "Nostr {}",
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&sign_at(
                key,
                24242,
                tags,
                "Authorize fixture blob action".into(),
                auth::now() - 1
            ))
            .unwrap()
        )
    )
}
