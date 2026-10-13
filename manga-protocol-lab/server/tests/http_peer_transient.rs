//! Real HTTP peers under bounded, externally synchronized source contention.
//! SQL only arranges the fault; replies, signed events and originals are the oracle.
mod support;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_manifest::{Descriptor, Family, FamilyRoot, Limits, Root},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{Arc, Mutex},
    time::Duration,
};
use support::*;

/// A loopback wire fault in front of a real node. Healthy traffic, including
/// nonce identity and service proofs, passes unchanged; only one selected path
/// gets an intentionally untrusted status/body with no service proof.
#[derive(Clone)]
enum WireReply {
    Status(u16),
    Interrupted(u16, Vec<u8>),
    CompleteShort(Vec<u8>),
    Corrupt(Vec<u8>),
    Oversize,
}
#[derive(Clone)]
struct WireFault {
    upstream: String,
    client: reqwest::Client,
    fault: Arc<Mutex<Option<(String, WireReply)>>>,
}
async fn proxy(
    axum::extract::State(state): axum::extract::State<WireFault>,
    request: axum::extract::Request,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    use axum::{body::Body, http::StatusCode, response::IntoResponse};
    let selected = state.fault.lock().unwrap().clone();
    if let Some((path, reply)) = selected
        && request.uri().path() == path
    {
        use axum::body::Bytes;
        use futures_util::{StreamExt, stream};
        return Ok(match reply {
            WireReply::Status(status) => (
                StatusCode::from_u16(status).unwrap(),
                "untrusted upstream fault detail",
            )
                .into_response(),
            WireReply::Interrupted(status, bytes) => {
                let length = bytes.len();
                let chunks = stream::once(async move {
                    Ok::<_, std::io::Error>(Bytes::copy_from_slice(&bytes[..length / 2]))
                })
                .chain(stream::once(async {
                    // Delay the deliberately broken body until the valid
                    // response headers and first chunk have reached the wire.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "fixture body interrupted",
                    ))
                }));
                (
                    StatusCode::from_u16(status).unwrap(),
                    [(axum::http::header::CONTENT_LENGTH, length.to_string())],
                    Body::from_stream(chunks),
                )
                    .into_response()
            }
            WireReply::CompleteShort(bytes) => {
                let half = bytes.len() / 2;
                (
                    StatusCode::OK,
                    Body::from_stream(stream::iter([Ok::<_, std::io::Error>(
                        Bytes::copy_from_slice(&bytes[..half]),
                    )])),
                )
                    .into_response()
            }
            WireReply::Corrupt(mut bytes) => {
                bytes[0] ^= 1;
                (StatusCode::OK, Body::from(bytes)).into_response()
            }
            WireReply::Oversize => (
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_LENGTH,
                    (32 * 1024 * 1024).to_string(),
                )],
                Body::from_stream(stream::pending::<Result<Bytes, std::io::Error>>()),
            )
                .into_response(),
        });
    }
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, 1_048_576)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let reply = state
        .client
        .request(parts.method, format!("{}{}", state.upstream, parts.uri))
        .headers(parts.headers)
        .body(body)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let status = reply.status();
    let headers = reply.headers().clone();
    let body = reply.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let mut response = (status, Body::from(body)).into_response();
    *response.headers_mut() = headers;
    Ok(response)
}

struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct BusyRead {
    locker: deadpool_postgres::Object,
    request: tokio::task::JoinHandle<Result<reqwest::Response, reqwest::Error>>,
}
impl BusyRead {
    async fn begin(repo: &Repository, c: &reqwest::Client, url: &str) -> Result<Self> {
        let locker = repo.pool.get().await?;
        locker
            .batch_execute("BEGIN; LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
            .await?;
        let pid = locker
            .query_one("SELECT pg_backend_pid()", &[])
            .await?
            .get(0);
        let c = c.clone();
        let url = url.to_owned();
        let request = tokio::spawn(async move {
            c.get(&url)
                .header("authorization", http_get(1, &url))
                .send()
                .await
        });
        wait_for_blocked_session(repo, pid).await?;
        Ok(Self { locker, request })
    }
    async fn release(self) -> Result<()> {
        self.locker.batch_execute("ROLLBACK").await?;
        let reply = self.request.await??;
        ensure!(
            reply.status() == 200,
            "faulted control read: {}",
            reply.text().await?
        );
        Ok(())
    }
}
async fn repo(dsn: String) -> Result<Repository> {
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1_048_576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    Ok(repo)
}
fn spawn(config: &Path, dsn: &str) -> Result<Node> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(config.with_extension("log"))?;
    Ok(Node(
        Command::new(
            std::env::var_os("MANGA_TEST_NODE_BIN")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_manga-node").into()),
        )
        .args(["serve", config.to_str().context("configuration path")?])
        .env("MANGA_DATABASE_URL", dsn)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?,
    ))
}
async fn ready(c: &reqwest::Client, origin: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(origin).send().await.is_ok_and(|r| r.status() == 200) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    Ok(())
}
async fn get(c: &reqwest::Client, origin: &str, path: &str) -> Result<reqwest::Response> {
    let url = format!("{origin}{path}");
    Ok(c.get(&url)
        .header("authorization", http_get(1, &url))
        .send()
        .await?)
}
async fn exact_event(c: &reqwest::Client, origin: &str, event: &auth::Event) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let reply = get(c, origin, &format!("/events/{}", event.id)).await?;
            if reply.status() == 200 {
                assert_eq!(reply.json::<Value>().await?, serde_json::to_value(event)?);
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    Ok(())
}
async fn policy(
    c: &reqwest::Client,
    origin: &str,
    admin: u8,
    key: u8,
    endpoint: &str,
    allowed: bool,
) -> Result<()> {
    let reply = post_json(
        c,
        admin,
        &format!("{origin}/admin/peers"),
        &json!({"key":public(key),"endpoint":endpoint,"allowed":allowed}),
    )
    .await?;
    ensure!(
        reply.status() == 200,
        "peer policy: {}",
        reply.text().await?
    );
    Ok(())
}
fn chapter_manifest(image: &[u8], chapter: &str) -> Vec<u8> {
    cbor(
        json!({"version":1,"series":"ab".repeat(32),"language":"en","chapter":chapter,"pages":[page(image)]}),
    )
}
async fn publish(
    c: &reqwest::Client,
    origin: &str,
    image: &[u8],
    chapter: &str,
) -> Result<auth::Event> {
    upload_to(c, 1, origin, image).await?;
    let manifest = chapter_manifest(image, chapter);
    upload_to(c, 1, origin, &manifest).await?;
    let event = publication(1, 1001, &manifest);
    let reply = post_json(
        c,
        1,
        &format!("{origin}/publications"),
        &serde_json::to_value(&event)?,
    )
    .await?;
    ensure!(
        reply.status().is_success(),
        "publication: {}",
        reply.text().await?
    );
    exact_event(c, origin, &event).await?;
    Ok(event)
}
async fn sync(
    c: &reqwest::Client,
    origin: &str,
    admin: u8,
    peer: u8,
    event: &auth::Event,
    fallbacks: &[String],
) -> Result<reqwest::Response> {
    post_json(
        c,
        admin,
        &format!("{origin}/admin/sync"),
        &json!({"peer":public(peer),"event":event.id,"fallbacks":fallbacks}),
    )
    .await
}
async fn observation(label: &str, reply: reqwest::Response) -> Result<(String, u16, Value)> {
    let status = reply.status().as_u16();
    let body = reply.json::<Value>().await?;
    eprintln!("transient observation {label}: HTTP{status} {body}");
    Ok((label.into(), status, body))
}
async fn job_status(c: &reqwest::Client, receiver: &str, id: &str) -> Result<Value> {
    let url = format!("{receiver}/admin/sync-jobs/{id}");
    let reply = c
        .get(&url)
        .header("authorization", http_get(2, &url))
        .send()
        .await?;
    ensure!(reply.status() == 200, "job status unavailable");
    Ok(reply.json().await?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and three real strfry-backed nodes"]
async fn transient_sources_are_retryable_without_replaying_writes() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test")
            && dsn.contains("dbname=manga_test"),
        "fixture database only"
    );
    let source = repo(dsn.clone()).await?;
    for name in ["manga_transient_receiver", "manga_transient_mirror"] {
        source
            .pool
            .get()
            .await?
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .await?;
    }
    let dsns = [
        dsn.clone(),
        dsn.replace("dbname=manga_test", "dbname=manga_transient_receiver"),
        dsn.replace("dbname=manga_test", "dbname=manga_transient_mirror"),
    ];
    let receiver = repo(dsns[1].clone()).await?;
    let _mirror = repo(dsns[2].clone()).await?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let directory = tempfile::tempdir()?.keep();
    eprintln!("retained transient-peer fixture: {}", directory.display());
    let listeners = (0..3)
        .map(|_| std::net::TcpListener::bind("127.0.0.1:0"))
        .collect::<std::io::Result<Vec<_>>>()?;
    let binds = listeners
        .iter()
        .map(std::net::TcpListener::local_addr)
        .collect::<std::io::Result<Vec<_>>>()?;
    let origins: Vec<_> = binds.iter().map(|bind| format!("http://{bind}")).collect();
    drop(listeners);
    let mut configs: Vec<PathBuf> = Vec::new();
    let mut nodes = Vec::new();
    for (n, database) in dsns.iter().enumerate() {
        let bind = binds[n];
        let origin = &origins[n];
        let base = directory.join(format!("node-{n}"));
        std::fs::create_dir_all(&base)?;
        let key = base.join("service.key");
        write_key(&key, 11 + n as u8)?;
        let config = base.join("node.json");
        std::fs::write(
            &config,
            serde_json::to_vec(&json!({
                "bind":bind.to_string(),"base_url":origin,"service_key_file":key,
                "admin_pubkey":public(1+n as u8),"database_plaintext_local":true,
                "read_policy":if n==0 {"admitted"} else {"public"},
                "read_admission":{"overall":4,"wait_ms":200,"catalogue":{"active":1,"waiting":0},"library_head":{"active":1,"waiting":0},"live":{"active":1,"waiting":0}},
                "sync_worker_enabled":n==1,"discovery_private_overrides":origins,
                "storage":{"path":base.join("objects"),"volume":format!("transient{n}"),"physical_bytes":2097152,"staging_bytes":2097152,"max_blob_bytes":262144},
                "backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":base.join("backend"),"map_bytes":67108864}
            }))?,
        )?;
        nodes.push(Some(spawn(&config, database)?));
        configs.push(config);
    }
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()?;
    for origin in &origins {
        ready(&c, origin).await?;
    }
    let [a, b, m] = [&origins[0], &origins[1], &origins[2]];
    for account in [1, 21] {
        assert_eq!(
            post_json(
                &c,
                1,
                &format!("{a}/admin/accounts/{}", public(account)),
                &json!({"allowed":true})
            )
            .await?
            .status(),
            200
        );
    }
    for (origin, admin, peer, endpoint) in [
        (a, 1, 12, b),
        (a, 1, 13, m),
        (b, 2, 11, a),
        (b, 2, 13, m),
        (m, 3, 11, a),
        (m, 3, 12, b),
    ] {
        policy(&c, origin, admin, peer, endpoint, true).await?;
    }
    let bytes = png();
    let publication = publish(&c, a, &bytes, "recovery").await?;
    let fallback = publish(&c, a, &bytes, "fallback").await?;
    let durable = publish(&c, a, &bytes, "durable").await?;
    let mirrored = sync(&c, m, 3, 11, &fallback, &[]).await?;
    ensure!(
        mirrored.status().is_success(),
        "mirror transfer: {}",
        mirrored.text().await?
    );
    exact_event(&c, m, &fallback).await?;
    let limits = Limits::default();
    let library_root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![Descriptor::empty()],
            })
            .collect(),
    }
    .encode(&limits)?;
    let library_event = sign_at(
        21,
        11999,
        vec![],
        json!({"manifest":auth::hash(&library_root),"version":1}).to_string(),
        auth::now(),
    );
    let reply = post_json(&c,21,&format!("{a}/libraries"),&json!({"event":library_event,"expected_head":null,"objects":[{"type":"root","data":STANDARD.encode(&library_root)}]})).await?;
    ensure!(
        reply.status().is_success(),
        "source library: {}",
        reply.text().await?
    );
    exact_event(&c, a, &library_event).await?;
    let account_event = sign_at(
        21,
        0,
        vec![],
        json!({"name":"Transient source fixture"}).to_string(),
        auth::now(),
    );
    let reply = post_json(
        &c,
        21,
        &format!("{a}/account-metadata"),
        &serde_json::to_value(&account_event)?,
    )
    .await?;
    ensure!(
        reply.status().is_success(),
        "source account: {}",
        reply.text().await?
    );
    exact_event(&c, a, &account_event).await?;
    let library_body = json!({"peer":public(11),"publisher":public(21)});
    let account_body = json!({"peer":public(11),"event":account_event.id});
    let mut observed = Vec::new();
    // Each fault owns one public read. The PG barrier establishes that read is
    // blocked; an actual source429 establishes occupied admission, not a counter.
    for (label, route, body) in [
        (
            "publication identity429",
            "/admin/sync",
            json!({"peer":public(11),"event":publication.id}),
        ),
        (
            "library identity429",
            "/admin/sync-library",
            library_body.clone(),
        ),
        (
            "account identity429",
            "/admin/sync-account",
            account_body.clone(),
        ),
    ] {
        let fault = BusyRead::begin(&source, &c, &format!("{a}/query?order=asc")).await?;
        let busy = c
            .get(format!("{a}/identity?nonce={}", "a1".repeat(32)))
            .send()
            .await?;
        assert_eq!(busy.status(), 429, "real source Catalogue overflow");
        observed.push(
            observation(
                label,
                post_json(&c, 2, &format!("{b}{route}"), &body).await?,
            )
            .await?,
        );
        fault.release().await?;
    }
    // Here identity is healthy and only LibraryHead is saturated. This reaches
    // the real shared GET boundary after successful identity verification.
    let fault = BusyRead::begin(&source, &c, &format!("{a}/libraries/{}", public(21))).await?;
    assert_eq!(
        c.get(format!("{a}/identity?nonce={}", "b2".repeat(32)))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        get(&c, a, &format!("/libraries/{}", public(21)))
            .await?
            .status(),
        429
    );
    observed.push(
        observation(
            "library metadata429",
            post_json(&c, 2, &format!("{b}/admin/sync-library"), &library_body).await?,
        )
        .await?,
    );
    fault.release().await?;
    // An independently approved primary has the wrong service identity. That
    // permanent failure does not establish invalidity when a fallback is busy.
    policy(&c, b, 2, 14, m, true).await?;
    // External fixture setup selects the established learned/pinned path. All
    // authority and transient outcomes below remain real HTTP observations.
    receiver
        .pool
        .get()
        .await?
        .execute(
            "UPDATE approved_peers SET discovered_endpoint=true WHERE service_key IN ($1,$2)",
            &[&hex::decode(public(11))?, &hex::decode(public(14))?],
        )
        .await?;
    let fault = BusyRead::begin(&source, &c, &format!("{a}/query?order=asc")).await?;
    observed.push(
        observation(
            "pinned source identity429",
            sync(&c, b, 2, 11, &publication, &[]).await?,
        )
        .await?,
    );
    fault.release().await?;
    assert_eq!(
        sync(&c, b, 2, 14, &publication, &[]).await?.status(),
        400,
        "pinned wrong service identity stays permanent"
    );
    let fault = BusyRead::begin(&source, &c, &format!("{a}/query?order=asc")).await?;
    observed.push(
        observation(
            "wrong identity plus busy approved fallback",
            sync(&c, b, 2, 14, &publication, &[public(11)]).await?,
        )
        .await?,
    );
    fault.release().await?;
    // Successful approved fallback remains usable during a genuine primary429.
    let fault = BusyRead::begin(&source, &c, &format!("{a}/query?order=asc")).await?;
    let reply = sync(&c, b, 2, 11, &fallback, &[public(13)]).await?;
    ensure!(
        reply.status().is_success(),
        "approved fallback: {}",
        reply.text().await?
    );
    let result: Value = reply.json().await?;
    assert_eq!(result["hosted"], true);
    assert_eq!(result["used_peers"], json!([public(13)]));
    fault.release().await?;
    exact_event(&c, b, &fallback).await?;
    // None of the failed attempts published a success, even after the fault ends.
    for event in [&publication, &library_event, &account_event] {
        assert_eq!(
            get(&c, b, &format!("/events/{}", event.id)).await?.status(),
            404
        );
    }
    // Refused connections retain their transport type through the pinned proof.
    drop(nodes[0].take());
    observed.push(
        observation(
            "pinned connection refused",
            sync(&c, b, 2, 11, &publication, &[]).await?,
        )
        .await?,
    );
    nodes[0] = Some(spawn(&configs[0], &dsns[0])?);
    ready(&c, a).await?;
    // A separate old-binary run reaches the later blob-fault assertions after
    // the initial status regressions have already been preserved independently.
    if std::env::var("MANGA_PEER_TRANSIENT_CASE").as_deref() != Ok("blob") {
        for (label, status, body) in &observed {
            assert_eq!(*status, 503, "{label}: {body}");
            assert_eq!(
                *body,
                json!({"error":"peer_temporarily_unavailable","retryable":true}),
                "{label}: fixed safe body with no host-state or upstream details"
            );
        }
    }
    // Fresh authorization is produced by each post_json call. The caller alone
    // explicitly retries the same immutable event; no failed POST is replayed.
    for _ in 0..2 {
        let reply = sync(&c, b, 2, 11, &publication, &[]).await?;
        ensure!(
            reply.status().is_success(),
            "explicit retry: {}",
            reply.text().await?
        );
        assert_eq!(reply.json::<Value>().await?["event"], publication.id);
        exact_event(&c, b, &publication).await?;
        let original = get(&c, b, &format!("/originals/{}", auth::hash(&bytes))).await?;
        assert_eq!(original.status(), 200);
        assert_eq!(original.bytes().await?.as_ref(), bytes);
    }
    for (route, body, event) in [
        ("/admin/sync-library", &library_body, &library_event),
        ("/admin/sync-account", &account_body, &account_event),
    ] {
        let reply = post_json(&c, 2, &format!("{b}{route}"), body).await?;
        ensure!(
            reply.status().is_success(),
            "adapter recovery: {}",
            reply.text().await?
        );
        exact_event(&c, b, event).await?;
    }
    // The durable workflow retains its existing retry path, with no skip result.
    let fault = BusyRead::begin(&source, &c, &format!("{a}/query?order=asc")).await?;
    let reply = post_json(
        &c,
        2,
        &format!("{b}/admin/sync-jobs"),
        &json!({"peer":public(11),"filter":"chapter=\"durable\"","fallbacks":[]}),
    )
    .await?;
    ensure!(
        reply.status().is_success(),
        "enqueue: {}",
        reply.text().await?
    );
    let job = reply.json::<Value>().await?["id"]
        .as_str()
        .context("job ID")?
        .to_owned();
    tokio::time::timeout(Duration::from_millis(800), async {
        loop {
            let status = job_status(&c, b, &job).await?;
            assert_eq!(status["skipped_items"], 0);
            assert_ne!(status["state"], "completed_with_skips");
            if status["last_error"].is_string() {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    fault.release().await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let status = job_status(&c, b, &job).await?;
            assert_eq!(status["skipped_items"], 0);
            assert_ne!(status["state"], "completed_with_skips");
            ensure!(status["state"] != "failed", "retrying job failed: {status}");
            if status["state"] == "completed" {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    exact_event(&c, b, &durable).await?;
    // Permanent caller and approval outcomes keep their existing client errors.
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{b}/admin/sync"),
            &json!({"peer":"invalid","event":publication.id})
        )
        .await?
        .status(),
        400
    );
    assert_eq!(
        sync(&c, b, 2, 14, &publication, &[]).await?.status(),
        400,
        "wrong service identity stays permanent"
    );
    policy(&c, b, 2, 13, m, false).await?;
    let fault = BusyRead::begin(&source, &c, &format!("{a}/query?order=asc")).await?;
    assert_eq!(
        sync(&c, b, 2, 11, &fallback, &[public(13)]).await?.status(),
        400,
        "revoked fallback never used"
    );
    fault.release().await?;

    // Exercise the blob-fetch aggregation, status-before-size/proof ordering,
    // partial staging and explicit retry against a real source behind a local
    // wire fault. This source advertises the proxy origin so forwarded proofs
    // remain endpoint-bound; the fault itself never creates trusted metadata.
    let mut output = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([24, 48, 80, 255]),
    ))
    .write_to(&mut output, image::ImageFormat::Png)?;
    let blob_bytes = output.into_inner();
    let blob_event = publish(&c, a, &blob_bytes, "blob-fault").await?;
    ensure!(
        sync(&c, m, 3, 11, &blob_event, &[])
            .await?
            .status()
            .is_success(),
        "prepare independently approved blob mirror"
    );
    exact_event(&c, m, &blob_event).await?;
    policy(&c, b, 2, 13, m, true).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let proxy_origin = format!("http://{}", listener.local_addr()?);
    let wire = WireFault {
        upstream: a.clone(),
        client: c.clone(),
        fault: Arc::new(Mutex::new(None)),
    };
    let app = axum::Router::new().fallback(proxy).with_state(wire.clone());
    let proxy_task = tokio::spawn(async move { axum::serve(listener, app).await });
    drop(nodes[0].take());
    let mut config: Value = serde_json::from_slice(&std::fs::read(&configs[0])?)?;
    config["base_url"] = json!(proxy_origin);
    std::fs::write(&configs[0], serde_json::to_vec(&config)?)?;
    nodes[0] = Some(spawn(&configs[0], &dsns[0])?);
    ready(&c, &proxy_origin).await?;
    policy(&c, b, 2, 11, &proxy_origin, true).await?;
    let blob_path = format!("/federation/blobs/{}", auth::hash(&blob_bytes));
    let mut blob_observations = Vec::new();
    for status in [408, 503, 404] {
        *wire.fault.lock().unwrap() = Some((blob_path.clone(), WireReply::Status(status)));
        blob_observations.push(
            observation(
                &format!("original blob HTTP{status}"),
                sync(&c, b, 2, 11, &blob_event, &[]).await?,
            )
            .await?,
        );
        assert_eq!(
            get(&c, b, &format!("/events/{}", blob_event.id))
                .await?
                .status(),
            404,
            "failed original must not publish staged manifest"
        );
        assert_eq!(
            get(&c, b, &format!("/originals/{}", auth::hash(&blob_bytes)))
                .await?
                .status(),
            404,
            "partial content cannot be served"
        );
    }
    for (index, (label, status, body)) in blob_observations.iter().enumerate() {
        if index < 2 {
            assert_eq!(*status, 503, "{label}: {body}");
            assert_eq!(
                *body,
                json!({"error":"peer_temporarily_unavailable","retryable":true})
            );
        } else {
            assert_eq!(
                *status, 400,
                "complete permanent404 stays a validation failure"
            );
            assert_eq!(body["error"], "invalid_request");
        }
    }
    *wire.fault.lock().unwrap() = Some((
        blob_path.clone(),
        WireReply::Interrupted(200, blob_bytes.clone()),
    ));
    let interrupted = observation(
        "interrupted original body",
        sync(&c, b, 2, 11, &blob_event, &[]).await?,
    )
    .await?;
    assert_eq!(
        get(&c, b, &format!("/events/{}", blob_event.id))
            .await?
            .status(),
        404,
        "interrupted body must not publish staged manifest"
    );
    assert_eq!(
        get(&c, b, &format!("/originals/{}", auth::hash(&blob_bytes)))
            .await?
            .status(),
        404,
        "no partial original serving"
    );
    let mut invalid_bodies = Vec::new();
    for (label, reply) in [
        (
            "complete short original",
            WireReply::CompleteShort(blob_bytes.clone()),
        ),
        ("declared oversized original", WireReply::Oversize),
        (
            "interrupted HTTP404 body",
            WireReply::Interrupted(404, blob_bytes.clone()),
        ),
        (
            "complete corrupt original",
            WireReply::Corrupt(blob_bytes.clone()),
        ),
    ] {
        *wire.fault.lock().unwrap() = Some((blob_path.clone(), reply));
        invalid_bodies.push(observation(label, sync(&c, b, 2, 11, &blob_event, &[]).await?).await?);
    }
    assert_eq!(
        interrupted.1, 503,
        "incomplete transport body is retryable: {}",
        interrupted.2
    );
    assert_eq!(
        interrupted.2,
        json!({"error":"peer_temporarily_unavailable","retryable":true})
    );
    for (label, status, body) in invalid_bodies {
        assert_eq!(status, 400, "{label}: {body}");
        assert_eq!(body["error"], "invalid_request");
    }
    *wire.fault.lock().unwrap() = None;
    for _ in 0..2 {
        ensure!(
            sync(&c, b, 2, 11, &blob_event, &[])
                .await?
                .status()
                .is_success(),
            "explicit retry after partial staging"
        );
        exact_event(&c, b, &blob_event).await?;
        assert_eq!(
            get(&c, b, &format!("/originals/{}", auth::hash(&blob_bytes)))
                .await?
                .bytes()
                .await?
                .as_ref(),
            blob_bytes
        );
    }
    // A transient manifest failure still permits the separately approved mirror.
    *wire.fault.lock().unwrap() = Some((
        format!(
            "/federation/blobs/{}",
            auth::hash(&chapter_manifest(&blob_bytes, "blob-fault"))
        ),
        WireReply::Status(503),
    ));
    let result = sync(&c, b, 2, 11, &blob_event, &[public(13)]).await?;
    ensure!(
        result.status().is_success(),
        "blob fallback: {}",
        result.text().await?
    );
    let result: Value = result.json().await?;
    assert_eq!(result["hosted"], true);
    assert!(
        result["used_peers"]
            .as_array()
            .unwrap()
            .contains(&json!(public(13)))
    );
    *wire.fault.lock().unwrap() = None;
    let expected_used = bytes.len()
        + library_root.len()
        + blob_bytes.len()
        + ["recovery", "fallback", "durable"]
            .iter()
            .map(|chapter| chapter_manifest(&bytes, chapter).len())
            .sum::<usize>()
        + chapter_manifest(&blob_bytes, "blob-fault").len();
    assert_eq!(
        post_json(
            &c,
            2,
            &format!("{b}/admin/publishers/{}", public(249)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200,
        "authorize public capacity probe publisher"
    );
    assert_http_storage_used(&c, b, expected_used).await?;
    proxy_task.abort();
    Ok(())
}

/// Public exact-fill/overflow/delete observations catch both missing and doubled
/// physical charges after failed staging, fallback and explicit duplicate retry.
async fn assert_http_storage_used(c: &reqwest::Client, origin: &str, used: usize) -> Result<()> {
    let mut remaining = 2_097_152usize
        .checked_sub(used)
        .context("fixture capacity")?;
    let probe = [251u8];
    let probe_hash = upload_to(c, 249, origin, &probe).await?;
    assert_eq!(
        c.delete(format!("{origin}/blobs/{probe_hash}"))
            .header("authorization", blossom(249, origin, "delete", &probe_hash))
            .send()
            .await?
            .status(),
        200
    );
    let mut fillers = Vec::new();
    while remaining > 0 {
        let size = remaining.min(262_144);
        let nonce: [u8; 32] = rand::random();
        let bytes: Vec<_> = (0..size).map(|i| nonce[i % nonce.len()]).collect();
        fillers.push(upload_to(c, 249, origin, &bytes).await?);
        remaining -= size;
    }
    assert_eq!(
        c.put(format!("{origin}/upload/{probe_hash}"))
            .header("authorization", blossom(249, origin, "upload", &probe_hash))
            .body(probe.to_vec())
            .send()
            .await?
            .status(),
        503,
        "exact physical fill rejects one more byte"
    );
    for hash in fillers {
        assert_eq!(
            c.delete(format!("{origin}/blobs/{hash}"))
                .header("authorization", blossom(249, origin, "delete", &hash))
                .send()
                .await?
                .status(),
            200,
            "probe cleanup"
        );
    }
    Ok(())
}
