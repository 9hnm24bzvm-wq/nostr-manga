//! Original uploads share bounded receipt capacity with library staging.
//! External PostgreSQL waits arrange contention; HTTP outcomes are the oracle.
mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_manifest::{Chunk, Family, Limits, Record},
};
use serde_json::json;
use std::{
    process::{Child, Command},
    time::Duration,
};
use support::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const FAULT: i64 = 793841;
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Fixture {
    _node: Node,
    _dir: tempfile::TempDir,
    repo: Repository,
    client: reqwest::Client,
    origin: String,
    bind: std::net::SocketAddr,
}
impl Fixture {
    async fn start() -> Result<Self> {
        let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
        ensure!(
            dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
            "fixture database required"
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
        let dir = tempfile::tempdir()?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let bind = listener.local_addr()?;
        drop(listener);
        let origin = format!("http://{bind}");
        let key = dir.path().join("service.key");
        write_key(&key, 11)?;
        let config = dir.path().join("node.json");
        std::fs::write(
            &config,
            serde_json::to_vec(&json!({
                "bind":bind.to_string(), "base_url":origin, "service_key_file":key,
                "admin_pubkey":public(1), "database_plaintext_local":true,
                "sync_worker_enabled":false,
                "storage":{"path":dir.path().join("objects"), "volume":"uploadadmission",
                    "physical_bytes":1048576, "staging_bytes":1048576,
                    "max_blob_bytes":4096, "upload_slots":1}
            }))?,
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
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(8))
            .build()?;
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if client
                    .get(&origin)
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("node readiness")?;
        repo.set_account_admission(hex::decode(public(3))?.try_into().unwrap(), true)
            .await?;
        repo.set_publisher_admission(hex::decode(public(2))?.try_into().unwrap(), true)
            .await?;
        Ok(Self {
            _node: node,
            _dir: dir,
            repo,
            client,
            origin,
            bind,
        })
    }
    fn upload(&self, blossom_route: bool, bytes: Vec<u8>) -> reqwest::RequestBuilder {
        let hash = auth::hash(&bytes);
        let path = if blossom_route {
            "/upload".to_owned()
        } else {
            format!("/upload/{hash}")
        };
        let authority = blossom(
            2,
            if blossom_route {
                "127.0.0.1"
            } else {
                &self.origin
            },
            "upload",
            &hash,
        );
        self.client
            .put(format!("{}{path}", self.origin))
            .header("authorization", authority)
            .body(bytes)
    }
    fn stage(&self, n: u8) -> tokio::task::JoinHandle<Result<reqwest::Response, reqwest::Error>> {
        let bytes = chunk(n);
        let url = format!("{}/library/objects/chunk", self.origin);
        let request = self
            .client
            .post(&url)
            .header("authorization", http(3, &url, &bytes))
            .body(bytes);
        tokio::spawn(async move { request.send().await })
    }
    async fn wait_for_fault(&self, blocker: &deadpool_postgres::Object) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let waiting: bool = blocker.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND pg_backend_pid()=ANY(pg_blocking_pids(pid)))", &[]).await?.get(0);
                if waiting { return Ok::<_,anyhow::Error>(()); }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.context("staging did not reach external fault")??;
        Ok(())
    }
    async fn verify_original(&self, bytes: &[u8]) -> Result<()> {
        let hash = auth::hash(bytes);
        let response = self
            .client
            .get(format!("{}/blobs/{hash}", self.origin))
            .header("authorization", blossom(2, &self.origin, "get", &hash))
            .send()
            .await?;
        ensure!(
            response.status() == 200,
            "original unavailable: {}",
            response.text().await?
        );
        ensure!(
            response.bytes().await?.as_ref() == bytes,
            "original bytes changed"
        );
        Ok(())
    }
    async fn expect_upload(&self, bytes: &[u8]) -> Result<tokio::net::TcpStream> {
        let hash = auth::hash(bytes);
        let mut stream = tokio::net::TcpStream::connect(self.bind).await?;
        stream.write_all(format!("PUT /upload/{hash} HTTP/1.1\r\nHost: {}\r\nAuthorization: {}\r\nContent-Length: {}\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n",self.bind,blossom(2,&self.origin,"upload",&hash),bytes.len()).as_bytes()).await?;
        Ok(stream)
    }
}
fn chunk(n: u8) -> Vec<u8> {
    Chunk::new(
        Family::Chapters,
        vec![Record::Chapter {
            id: [n; 32],
            completed: true,
            page: None,
            updated_at: 1,
        }],
        &Limits::default(),
    )
    .unwrap()
    .encode(&Limits::default())
    .unwrap()
}
fn original(n: u8) -> Vec<u8> {
    let mut bytes = png();
    bytes.push(n);
    bytes
}
async fn header(stream: &mut tokio::net::TcpStream) -> Result<String> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(stream.read_u8().await?);
        ensure!(bytes.len() < 16384, "response header bound");
    }
    Ok(String::from_utf8(bytes)?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real HTTP node"]
async fn bounded_original_uploads_progress_behind_library_staging() -> Result<()> {
    let fixture = Fixture::start().await?;
    let blocker = fixture.repo.pool.get().await?;
    let hashes = (1..=8u8)
        .map(|n| format!("decode('{}','hex')", auth::hash(&chunk(n))))
        .collect::<Vec<_>>()
        .join(",");
    blocker.batch_execute(&format!("CREATE FUNCTION fixture_upload_admission() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.hash IN ({hashes}) THEN PERFORM set_config('lock_timeout','4500',true); PERFORM pg_advisory_xact_lock({FAULT}); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_upload_admission BEFORE INSERT ON library_chunk_summaries FOR EACH ROW EXECUTE FUNCTION fixture_upload_admission()")).await?;
    for (n, blossom_route) in [(1, false), (2, true), (3, false)] {
        blocker
            .execute("SELECT pg_advisory_lock($1)", &[&FAULT])
            .await?;
        let stage = fixture.stage(n);
        fixture.wait_for_fault(&blocker).await?;
        // Chapter manifests use the same signed PUT adapter as original images.
        let bytes = if n == 3 {
            cbor(
                json!({"version":1,"series":"41".repeat(32),"language":"en","chapter":"fixture","pages":[page(&original(1))]}),
            )
        } else {
            original(n)
        };
        let request = fixture.upload(blossom_route, bytes.clone());
        let mut upload = tokio::spawn(async move { request.send().await });
        let early = tokio::time::timeout(Duration::from_millis(100), &mut upload).await;
        blocker
            .execute("SELECT pg_advisory_unlock($1)", &[&FAULT])
            .await?;
        let staged = stage.await??;
        ensure!(
            staged.status() == 200,
            "library staging: {}",
            staged.text().await?
        );
        let queued = early.is_err();
        let response = match early {
            Ok(done) => done??,
            Err(_) => upload.await??,
        };
        let status = response.status();
        ensure!(
            status.is_success(),
            "original upload must wait for temporary library contention: {status} {}",
            response.text().await?
        );
        ensure!(
            queued,
            "original upload bypassed occupied shared receipt capacity"
        );
        fixture.verify_original(&bytes).await?;
    }

    // Closing a TCP peer need not cancel Hyper's handler before it polls the
    // request body. Both incomplete and fully sent requests must release their
    // bounded waiting reservation by the two-second admission deadline.
    for (n, send_body) in [(4, false), (7, true)] {
        blocker
            .execute("SELECT pg_advisory_lock($1)", &[&FAULT])
            .await?;
        let stage = fixture.stage(n);
        fixture.wait_for_fault(&blocker).await?;
        let bytes = original(n + 20);
        let started = tokio::time::Instant::now();
        let mut cancelled = fixture.expect_upload(&bytes).await?;
        if send_body {
            cancelled.write_all(&bytes).await?;
        }
        ensure!(
            tokio::time::timeout(Duration::from_millis(100), cancelled.read_u8())
                .await
                .is_err(),
            "queued upload began receipt or returned early"
        );
        let overflow = fixture
            .upload(true, original(5))
            .timeout(Duration::from_millis(500))
            .send()
            .await?;
        ensure!(
            overflow.status() == 429,
            "original waiting queue was not bounded"
        );
        cancelled.shutdown().await?;
        drop(cancelled);
        tokio::time::sleep_until(started + Duration::from_millis(2300)).await;
        let mut successor = fixture.expect_upload(&bytes).await?;
        let early = tokio::time::timeout(Duration::from_millis(100), successor.read_u8()).await;
        blocker
            .execute("SELECT pg_advisory_unlock($1)", &[&FAULT])
            .await?;
        ensure!(
            stage.await??.status() == 200,
            "library staging after disconnected waiter"
        );
        ensure!(
            early.is_err(),
            "disconnected original retained its ticket past the admission deadline"
        );
        ensure!(
            header(&mut successor).await?.starts_with("HTTP/1.1 100"),
            "successor did not acquire receipt"
        );
        successor.write_all(&bytes).await?;
        ensure!(
            header(&mut successor).await?.starts_with("HTTP/1.1 200"),
            "successor upload did not finish"
        );
        fixture.verify_original(&bytes).await?;
    }

    // An unavailable shared slot never creates an unbounded wait. Timeout
    // removes the FIFO ticket so the next original can proceed normally.
    blocker
        .execute("SELECT pg_advisory_lock($1)", &[&FAULT])
        .await?;
    let stage = fixture.stage(5);
    fixture.wait_for_fault(&blocker).await?;
    let started = tokio::time::Instant::now();
    let expired = fixture.upload(false, original(5)).send().await?;
    let elapsed = started.elapsed();
    blocker
        .execute("SELECT pg_advisory_unlock($1)", &[&FAULT])
        .await?;
    ensure!(
        stage.await??.status() == 200,
        "library staging after queue timeout"
    );
    ensure!(
        expired.status() == 429
            && elapsed >= Duration::from_millis(1800)
            && elapsed < Duration::from_secs(3),
        "original queue deadline was not bounded: {} in {elapsed:?}",
        expired.status()
    );
    ensure!(
        fixture.upload(false, original(5)).send().await?.status() == 200,
        "timed-out queue ticket was not released"
    );
    fixture.verify_original(&original(5)).await?;

    // Permission checked before queueing cannot authorize a later revoked write.
    blocker
        .execute("SELECT pg_advisory_lock($1)", &[&FAULT])
        .await?;
    let stage = fixture.stage(6);
    fixture.wait_for_fault(&blocker).await?;
    let request = fixture.upload(true, original(6));
    let mut revoked = tokio::spawn(async move { request.send().await });
    ensure!(
        tokio::time::timeout(Duration::from_millis(100), &mut revoked)
            .await
            .is_err(),
        "revocation probe did not queue"
    );
    fixture
        .repo
        .set_publisher_admission(hex::decode(public(2))?.try_into().unwrap(), false)
        .await?;
    blocker
        .execute("SELECT pg_advisory_unlock($1)", &[&FAULT])
        .await?;
    ensure!(
        stage.await??.status() == 200,
        "library staging during original revocation"
    );
    ensure!(
        revoked.await??.status() == 403,
        "queued upload bypassed publisher revocation"
    );
    fixture
        .repo
        .set_publisher_admission(hex::decode(public(2))?.try_into().unwrap(), true)
        .await?;
    blocker.batch_execute("DROP TRIGGER fixture_upload_admission ON library_chunk_summaries; DROP FUNCTION fixture_upload_admission()").await?;

    // Stall a real original after filesystem work, before durable finalization.
    // Expect:100-continue observes receipt admission without conflating it with
    // the independent lower filesystem-work limit.
    let owned = original(7);
    blocker.batch_execute(&format!("CREATE FUNCTION fixture_owned_original() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.hash=decode('{}','hex') THEN PERFORM set_config('lock_timeout','4500',true); PERFORM pg_advisory_xact_lock({FAULT}); END IF; RETURN NEW; END $$; CREATE TRIGGER fixture_owned_original BEFORE UPDATE OF state ON blob_intents FOR EACH ROW EXECUTE FUNCTION fixture_owned_original()",auth::hash(&owned))).await?;
    blocker
        .execute("SELECT pg_advisory_lock($1)", &[&FAULT])
        .await?;
    let mut abandoned = fixture.expect_upload(&owned).await?;
    ensure!(
        header(&mut abandoned).await?.starts_with("HTTP/1.1 100"),
        "original was not admitted"
    );
    abandoned.write_all(&owned).await?;
    fixture.wait_for_fault(&blocker).await?;
    drop(abandoned);
    let next = original(8);
    let mut waiting = fixture.expect_upload(&next).await?;
    let early = tokio::time::timeout(Duration::from_millis(100), waiting.read_u8()).await;
    blocker
        .execute("SELECT pg_advisory_unlock($1)", &[&FAULT])
        .await?;
    ensure!(
        early.is_err(),
        "disconnected original released receipt while owned mutation was active"
    );
    ensure!(
        header(&mut waiting).await?.starts_with("HTTP/1.1 100"),
        "owned completion did not release receipt"
    );
    waiting.write_all(&next).await?;
    ensure!(
        header(&mut waiting).await?.starts_with("HTTP/1.1 200"),
        "next original failed after owned completion"
    );
    fixture.verify_original(&owned).await?;
    fixture.verify_original(&next).await?;
    blocker.batch_execute("DROP TRIGGER fixture_owned_original ON blob_intents; DROP FUNCTION fixture_owned_original()").await?;
    Ok(())
}
