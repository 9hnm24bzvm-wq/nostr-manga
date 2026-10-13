mod support;
use anyhow::Context;
use anyhow::{Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_client_bundle::SaveBundle,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{
    process::{Child, Command},
    time::Duration,
};
use support::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn read_holder_status(c: &reqwest::Client, origin: &str) -> Result<Value> {
    let url = format!("{origin}/admin/status");
    let response = c
        .get(&url)
        .header("authorization", http_get(1, &url))
        .send()
        .await?;
    ensure!(
        response.status() == 200,
        "operator status must remain available"
    );
    response
        .json::<Value>()
        .await?
        .get("read_holder_diagnostics")
        .cloned()
        .context("opt-in holder diagnostics missing")
}

fn rejection_records(dir: &std::path::Path) -> Result<Vec<Value>> {
    std::fs::read_to_string(dir.join("node.log"))?
        .lines()
        .filter_map(|line| line.strip_prefix("READ_ADMISSION_DIAGNOSTIC "))
        .map(|line| {
            ensure!(
                line.len() <= 2048,
                "holder snapshot exceeded fixed log budget"
            );
            Ok(serde_json::from_str(line)?)
        })
        .collect()
}

async fn holder_rejection(dir: &std::path::Path, previous: usize) -> Result<Value> {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let records = rejection_records(dir)?;
            if let Some(record) = records.get(previous) {
                break Ok::<_, anyhow::Error>(record.clone());
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .context("selected holder snapshot did not reach the process log")?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real PostgreSQL and strfry; database failure classification"]
async fn disconnected_http_retains_work_admission() -> Result<()> {
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
            &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"read_policy":"admitted","cors_origins":["https://reader.example"],"read_admission":{"overall":2,"wait_ms":50,"catalogue":{"active":1,"waiting":0},"library_head":{"active":1,"waiting":0},"live":{"active":1,"waiting":0}},"database_plaintext_local":true,"sync_worker_enabled":false,"group_keys_dir":dir.path().join("groups"),"storage":{"path":dir.path().join("objects"),"volume":"groupprofile","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
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
        .env("MANGA_READ_ADMISSION_DIAGNOSTICS", "1")
        .env_remove("MANGA_BENCH_PROFILE")
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?,
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
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
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{origin}/admin/accounts/{}", public(2)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    let mut locker = repo.pool.get().await?;
    let tx = locker.transaction().await?;
    tx.batch_execute("LOCK TABLE account_permissions IN ACCESS EXCLUSIVE MODE")
        .await?;
    let url = format!("{origin}/query?order=asc");
    let mut socket = tokio::net::TcpStream::connect(bind).await?;
    socket
        .write_all(
            format!(
                "GET /query?order=asc HTTP/1.1\r\nHost: {bind}\r\nAuthorization: {}\r\n\r\n",
                http_get(2, &url)
            )
            .as_bytes(),
        )
        .await?;
    // This SQL only establishes that our external fault has engaged.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let blocked: i64 = repo.pool.get().await?.query_one(
                "SELECT count(*) FROM pg_locks WHERE relation='account_permissions'::regclass AND NOT granted", &[]
            ).await?.get(0);
            if blocked > 0 { break Ok::<_, anyhow::Error>(()); }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await??;
    socket.shutdown().await?;
    drop(socket);
    tokio::time::sleep(Duration::from_millis(40)).await;
    let busy = c.get(&url).send().await?;
    assert_eq!(
        busy.status(),
        429,
        "disconnected HTTP must retain its class credit until actual SQL termination"
    );
    assert_eq!(busy.headers().get("retry-after").unwrap(), "1");
    let disconnected = holder_rejection(dir.path(), 0).await?;
    assert_eq!(
        disconnected["holder_phases"][0][4], 1,
        "disconnected HTTP must leave its actual DB operation visible until termination"
    );
    assert_eq!(
        c.get(format!("{origin}/libraries/{}", public(2)))
            .send()
            .await?
            .status(),
        403,
        "another read class remains available while cancelled work drains"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let response = c.get(&url).send().await?;
            if response.status() == 403 {
                break Ok::<_, anyhow::Error>(());
            }
            assert_eq!(response.status(), 429);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    tx.rollback().await?;
    assert_eq!(
        c.get(&url)
            .header("authorization", http_get(2, &url))
            .send()
            .await?
            .status(),
        200
    );
    library_deadline_responds_without_abandoning_admitted_work(&c, &origin, dir.path()).await?;
    Ok(())
}

fn bundle(
    key: u8,
    time: u64,
    series: [u8; 32],
    chapter: [u8; 32],
    name: Option<&str>,
    completed: bool,
) -> Result<SaveBundle> {
    let limits = Limits::default();
    let mut chunks = BTreeMap::new();
    let mut families = vec![];
    for family in Family::ALL {
        let records = match family {
            Family::Chapters => vec![Record::Chapter {
                id: chapter,
                completed,
                page: completed.then_some(12),
                updated_at: 1,
            }],
            Family::Series => vec![Record::Series {
                id: series,
                status: if completed { "reading" } else { "completed" }.into(),
                updated_at: 1,
            }],
            Family::Lists => name
                .map(|name| {
                    vec![Record::List {
                        id: [5; 16],
                        name: name.into(),
                        updated_at: 1,
                    }]
                })
                .unwrap_or_default(),
            Family::Members => name
                .map(|_| {
                    vec![Record::Member {
                        list: [5; 16],
                        series,
                        position: 7,
                        updated_at: 1,
                    }]
                })
                .unwrap_or_default(),
        };
        let descriptor = if records.is_empty() {
            Descriptor::empty()
        } else {
            let bytes = Chunk::new(family, records, &limits)?.encode(&limits)?;
            let summary = Chunk::decode(&bytes, &limits)?.1;
            chunks.insert(summary.hash(), bytes);
            Descriptor::from(&summary)
        };
        families.push(FamilyRoot {
            family,
            chunks: vec![descriptor],
        });
    }
    let root = Root { families }.encode(&limits)?;
    let event = sign_at(
        key,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        time,
    );
    SaveBundle::validate(event, root, chunks)
}
async fn save(c: &reqwest::Client, origin: &str, key: u8, bundle: &SaveBundle) -> Result<()> {
    let mut cursor = 0;
    while let Some(batch) = bundle.batch(cursor)? {
        let url = format!("{origin}/compat/mangadex/nostr/library/batches");
        let r = c
            .post(&url)
            .header("authorization", http(key, &url, &batch.body))
            .body(batch.body)
            .send()
            .await?;
        ensure!(r.status().is_success(), "batch failed: {}", r.text().await?);
        cursor = batch.next_object;
    }
    let url = format!("{origin}/compat/mangadex/nostr/libraries");
    let body = bundle.commit_body()?;
    let r = c
        .post(&url)
        .header("authorization", http(key, &url, &body))
        .body(body)
        .send()
        .await?;
    ensure!(
        r.status().is_success(),
        "commit failed: {}",
        r.text().await?
    );
    Ok(())
}

async fn library_deadline_responds_without_abandoning_admitted_work(
    c: &reqwest::Client,
    origin: &str,
    dir: &std::path::Path,
) -> Result<()> {
    let saved = bundle(2, auth::now(), [8; 32], [9; 32], Some("Saved"), true)?;
    save(c, origin, 2, &saved).await?;
    let (_, root_hash, _) = saved
        .objects()
        .find(|(kind, _, _)| *kind == "root")
        .context("root")?;
    let encoded = hex::encode(root_hash);
    let object_dir = dir
        .join("objects/objects")
        .join(&encoded[..2])
        .join(&encoded[2..4]);
    let object = std::fs::read_dir(object_dir)?
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(&format!("{encoded}."))
        })
        .context("persisted root for external lease")?
        .path();
    for (path, seconds, status, class) in [
        (
            format!("/libraries/{}/entries?family=chapters", public(2)),
            10,
            400,
            1,
        ),
        ("/compat/mangadex/user/list".to_owned(), 15, 503, 0),
    ] {
        let before = read_holder_status(c, origin).await?;
        let records_before = rejection_records(dir)?.len();
        let mut lease = tokio::process::Command::new("python3")
            .args([
                "-u",
                "-c",
                r#"
import fcntl, signal, sys
signal.signal(signal.SIGIO, lambda *_: print('BREAK', flush=True))
with open(sys.argv[1], 'r+b') as f:
    fcntl.fcntl(f, fcntl.F_SETLEASE, fcntl.F_WRLCK)
    print('READY', flush=True)
    sys.stdin.readline()
    fcntl.fcntl(f, fcntl.F_SETLEASE, fcntl.F_UNLCK)
"#,
            ])
            .arg(&object)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let mut output = BufReader::new(lease.stdout.take().context("lease output")?).lines();
        assert_eq!(output.next_line().await?.as_deref(), Some("READY"));
        let url = format!("{origin}{path}");
        let client = c.clone();
        let target = url.clone();
        let token = http_get(2, &target);
        let request_id = auth::token(&token).map_err(anyhow::Error::msg)?.id;
        let started = std::time::Instant::now();
        let request = tokio::spawn(async move {
            client
                .get(&target)
                .header("authorization", token)
                .header("origin", "https://reader.example")
                .send()
                .await
        });
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), output.next_line())
                .await??
                .as_deref(),
            Some("BREAK")
        );
        let response = tokio::time::timeout(Duration::from_secs(seconds + 2), request).await???;
        assert_eq!(response.status(), status);
        assert!(started.elapsed() < Duration::from_secs(seconds + 2));
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "https://reader.example"
        );
        let proof = response
            .headers()
            .get(manga_node::service_proof::HEADER)
            .context("timeout proof")?
            .to_str()?
            .to_owned();
        let bytes = response.bytes().await?;
        manga_node::service_proof::verify(
            &proof,
            hex::decode(public(11))?.try_into().unwrap(),
            &manga_node::service_proof::Scope {
                url: &url,
                method: "GET",
                request: &request_id,
                status,
            },
            &bytes,
        )?;
        let body: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            body,
            if seconds == 10 {
                json!({"error":"invalid_request","message":"library view deadline"})
            } else {
                json!({"error":"library_view_deadline"})
            }
        );
        assert_eq!(
            c.get(&url).send().await?.status(),
            429,
            "early response must leave operation admission owned while filesystem read is blocked"
        );
        let diagnostic = holder_rejection(dir, records_before).await?;
        assert_eq!(diagnostic["holder_phases"][class][6], 1);
        assert_eq!(
            diagnostic["holder_draining"][class], 1,
            "early HTTP response must mark the still-owned filesystem work as draining"
        );
        if class == 1 {
            // Status consumes catalogue capacity itself, so only the separate
            // library class can be observed while this fixture's holder is live.
            let draining = read_holder_status(c, origin).await?;
            assert_eq!(draining["active_phase_counts"][class][6], 1);
            assert_eq!(draining["active_draining"][class], 1);
            assert_eq!(
                draining["finished"][class], before["finished"][class],
                "sending a deadline response must not finish the blocked operation"
            );
        }
        lease
            .stdin
            .take()
            .context("lease input")?
            .write_all(b"release\n")
            .await?;
        assert!(lease.wait().await?.success());
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let probe = c.get(&url).send().await?;
                if probe.status() == 403 {
                    break Ok::<_, anyhow::Error>(());
                }
                assert_eq!(probe.status(), 429);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await??;
        let drained = read_holder_status(c, origin).await?;
        assert_eq!(drained["active_phase_counts"][class][6], 0);
        assert_eq!(drained["active_draining"][class], 0);
        ensure!(
            drained["finished"][class]
                .as_u64()
                .context("finished holder count")?
                > before["finished"][class]
                    .as_u64()
                    .context("initial finished holder count")?,
            "actual filesystem completion must finish its holder"
        );
        ensure!(
            drained["lifetime_max_us"][class]
                .as_u64()
                .context("holder maximum lifetime")?
                >= seconds * 1_000_000,
            "holder lifetime must extend through the deadline"
        );
        ensure!(
            drained["phase_us"][class][6]
                .as_u64()
                .context("storage phase elapsed time")?
                >= (seconds - 1) * 1_000_000,
            "blocked storage time must remain attributed while the response drains"
        );
        ensure!(
            drained["lifetime_buckets"][class][5]
                .as_u64()
                .context("long lifetime bucket")?
                >= 1,
            "completed deadline holder must reach the final bounded lifetime bucket"
        );
        let alias_url = format!(
            "{origin}/compat/mangadex/list/{}",
            manga_node::mangadex_compat::uuid(
                "list",
                &format!("{}:{}", public(2), hex::encode([5; 16]))
            )
        );
        if seconds == 15 {
            assert_eq!(
                c.get(&alias_url)
                    .header("authorization", http_get(2, &alias_url))
                    .send()
                    .await?
                    .status(),
                404,
                "timed-out lists handler must not continue to register its list alias after draining"
            );
        }
        let recovered = c
            .get(&url)
            .header("authorization", http_get(2, &url))
            .send()
            .await?;
        assert_eq!(recovered.status(), 200);
        let value: serde_json::Value = recovered.json().await?;
        if seconds == 10 {
            assert_eq!(value["entries"][0]["id"], hex::encode([9; 32]));
        } else {
            assert_eq!(value["data"][0]["attributes"]["name"], "Saved");
            assert_eq!(
                c.get(&alias_url)
                    .header("authorization", http_get(2, &alias_url))
                    .send()
                    .await?
                    .status(),
                200,
                "a fresh successful request performs normal alias registration"
            );
        }
    }
    Ok(())
}
