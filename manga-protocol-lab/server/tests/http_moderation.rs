mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::json;
use std::{
    path::Path,
    process::{Child, Command},
    time::Duration,
};
use support::*;
const ORIGIN: &str = "http://127.0.0.1:19509";
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn ready(c: &reqwest::Client) -> Result<()> {
    for _ in 0..100 {
        if c.get(ORIGIN).send().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::bail!("node failed to start")
}
fn backend_events(binary: &Path, backend: &Path, id: &str) -> Result<Vec<auth::Event>> {
    let scan = Command::new(binary)
        .arg("--config")
        .arg(backend.join("strfry.conf"))
        .args(["scan", &json!({"ids":[id]}).to_string()])
        .output()?;
    anyhow::ensure!(scan.status.success(), "actual strfry scan failed");
    let events = scan
        .stdout
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice)
        .collect::<std::result::Result<Vec<auth::Event>, _>>()?;
    for event in &events {
        event.verify().map_err(anyhow::Error::msg)?;
        anyhow::ensure!(event.id == id, "backend returned unrelated event");
    }
    Ok(events)
}
fn assert_backend_event(binary: &Path, backend: &Path, event: &auth::Event) -> Result<()> {
    let events = backend_events(binary, backend, &event.id)?;
    assert_eq!(
        events.len(),
        1,
        "expected event was never imported into actual backend"
    );
    assert_eq!(
        serde_json::to_value(&events[0])?,
        serde_json::to_value(event)?
    );
    Ok(())
}
async fn removed(binary: &Path, backend: &Path, id: &str) -> Result<()> {
    for _ in 0..250 {
        if backend_events(binary, backend, id)?.is_empty() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::bail!("actual backend deletion did not complete")
}
async fn publication_post(c: &reqwest::Client, event: &auth::Event) -> Result<reqwest::Response> {
    post_json(
        c,
        1,
        &format!("{ORIGIN}/publications"),
        &serde_json::to_value(event)?,
    )
    .await
}
async fn request(c: &reqwest::Client, key: u8, target: &str) -> Result<auth::Event> {
    let e = auth::sign(
        &format!("{key:064x}"),
        5,
        vec![vec!["e".into(), target.into()]],
        "Synthetic author deletion request".into(),
    )
    .unwrap();
    assert_eq!(
        post_json(
            c,
            1,
            &format!("{ORIGIN}/deletion-requests"),
            &serde_json::to_value(&e)?
        )
        .await?
        .status(),
        202
    );
    Ok(e)
}
async fn approve(c: &reqwest::Client, e: &auth::Event) -> Result<reqwest::Response> {
    post_json(
        c,
        1,
        &format!("{ORIGIN}/admin/deletions/{}/approve", e.id),
        &json!({}),
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and real strfry; run scripts/test-postgres.sh"]
async fn local_moderation_and_orphaned_projection() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
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
    let binary = std::env::var_os("STRFRY_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("vendor/strfry/strfry"));
    let backend = std::env::var_os("MANGA_TEST_BACKEND_DIR")
        .map(std::path::PathBuf::from)
        .context("fixture backend required")?;
    let pause = dir.path().join("pause-import");
    let entered = dir.path().join("entered");
    let finished = dir.path().join("finished-import");
    let wrapper = dir.path().join("strfry-delay.sh");
    let orphan_log = dir.path().join("orphan.log");
    // Scan stdout must remain connected; only delayed import redirects inherited pipes.
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\ncase \" $* \" in *' import '*) if [ -f '{}' ]; then echo entered > '{}'; sleep 2; '{}' \"$@\" >> '{}' 2>&1; status=$?; printf '%s\\n' \"$status\" > '{}'; exit \"$status\"; fi;; esac\nexec '{}' \"$@\"\n",
            pause.display(),
            entered.display(),
            binary.display(),
            orphan_log.display(),
            finished.display(),
            binary.display()
        ),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
    }
    let config = dir.path().join("node.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":"127.0.0.1:19509","base_url":ORIGIN,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"backend":{"binary":wrapper,"data_dir":backend,"timeout_seconds":10,"map_bytes":67108864},"storage":{"path":dir.path().join("originals"),"volume":"moderation","physical_bytes":2097152,"staging_bytes":65536,"max_blob_bytes":262144}}),
        )?,
    )?;
    let spawn = || -> Result<Node> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.path().join("node.log"))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .env("RUST_LOG", "warn")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let node = spawn()?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    ready(&c).await?;
    let image = png();
    let image_hash = upload_to(&c, 1, ORIGIN, &image).await?;
    let series = cbor(json!({"version":1,"titles":{"en":"Moderation fixture"}}));
    upload_to(&c, 1, ORIGIN, &series).await?;
    let se = publication(1, 1002, &series);
    assert!(publication_post(&c, &se).await?.status().is_success());
    wait_projected(&c, ORIGIN, &se.id).await?;
    assert_backend_event(&binary, &backend, &se)?;
    let make = |chapter: &str| {
        cbor(
            json!({"version":1,"series":se.id,"language":"en","chapter":chapter,"pages":[page(&image)]}),
        )
    };
    let first = make("1");
    upload_to(&c, 1, ORIGIN, &first).await?;
    let first = publication(1, 1001, &first);
    assert!(publication_post(&c, &first).await?.status().is_success());
    wait_projected(&c, ORIGIN, &first.id).await?;
    assert_backend_event(&binary, &backend, &first)?;
    let second = make("2");
    upload_to(&c, 1, ORIGIN, &second).await?;
    let second = publication(1, 1001, &second);
    assert!(publication_post(&c, &second).await?.status().is_success());
    wait_projected(&c, ORIGIN, &second.id).await?;
    assert_backend_event(&binary, &backend, &second)?;
    let sr = request(&c, 1, &se.id).await?;
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", se.id))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(approve(&c, &sr).await?.status(), 202);
    removed(&binary, &backend, &se.id).await?;
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", second.id))
            .send()
            .await?
            .status(),
        200,
        "series deletion must not cascade"
    );
    let wrong = request(&c, 2, &second.id).await?;
    assert_eq!(approve(&c, &wrong).await?.status(), 400);
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", second.id))
            .send()
            .await?
            .status(),
        200
    );
    let deletion = request(&c, 1, &first.id).await?;
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", first.id))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(approve(&c, &deletion).await?.status(), 202);
    removed(&binary, &backend, &first.id).await?;
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", first.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{image_hash}"))
            .send()
            .await?
            .status(),
        200,
        "shared original must survive"
    );
    assert_eq!(
        publication_post(&c, &first).await?.status(),
        400,
        "removed event cannot resurrect via upload retry"
    );
    let repeated = approve(&c, &deletion).await?;
    assert_eq!(repeated.status(), 202);
    let repeated: serde_json::Value = repeated.json().await?;
    assert_eq!(repeated["new_withdrawals"], 0);
    assert_eq!(repeated["projection_completion_pending"], false);
    let scan = Command::new(&binary)
        .arg("--config")
        .arg(backend.join("strfry.conf"))
        .args(["scan", &json!({"ids":[deletion.id]}).to_string()])
        .output()?;
    assert!(scan.status.success());
    assert!(scan.stdout.is_empty(), "raw kind5 must never be imported");
    // Backend absence can precede durable acknowledgement by one transaction.
    // Observe restore readiness through its own public operation, not a state row.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let response = post_json(
                &c,
                1,
                &format!("{ORIGIN}/admin/publications/{}/restore", first.id),
                &json!({}),
            )
            .await?;
            if response.status() == 200 {
                return Ok::<_, anyhow::Error>(());
            }
            assert_eq!(response.status(), 400, "unexpected restoration response");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", first.id))
            .send()
            .await?
            .status(),
        404
    );
    assert!(publication_post(&c, &first).await?.status().is_success());
    wait_projected(&c, ORIGIN, &first.id).await?;
    assert_backend_event(&binary, &backend, &first)?;
    assert_eq!(approve(&c, &deletion).await?.status(), 202);
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", first.id))
            .send()
            .await?
            .status(),
        200,
        "replaying old approval must not undo explicit restore"
    );
    // Kill the parent while its import subprocess is still running. The new
    // projector must wait for the inherited command lock before deleting.
    let interrupted = make("crash");
    upload_to(&c, 1, ORIGIN, &interrupted).await?;
    let interrupted = publication(1, 1001, &interrupted);
    std::fs::write(&pause, b"pause")?;
    assert_eq!(publication_post(&c, &interrupted).await?.status(), 202);
    for _ in 0..100 {
        if entered.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(entered.exists());
    drop(node);
    let er = auth::sign(
        &format!("{:064x}", 1),
        5,
        vec![vec!["e".into(), interrupted.id.clone()]],
        "remove after parent crash".into(),
    )
    .unwrap();
    repo.record_deletion_request(&er).await?;
    repo.approve_deletion(hex::decode(&er.id)?.try_into().unwrap(), None)
        .await?;
    std::fs::remove_file(&pause)?;
    let _node = spawn()?;
    ready(&c).await?;
    // Waiting for actual importer completion prevents an empty-backend false
    // pass before the orphan has had a chance to resurrect the approved target.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(status) = std::fs::read_to_string(&finished)
                && !status.trim().is_empty()
            {
                anyhow::ensure!(status.trim() == "0", "delayed importer failed");
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("orphan import did not complete")??;
    let orphan_output = std::fs::read_to_string(&orphan_log)?;
    assert!(
        orphan_output.contains("1 added"),
        "delayed importer must actually add the target: {orphan_output}"
    );
    removed(&binary, &backend, &interrupted.id).await?;
    assert!(
        backend_events(&binary, &backend, &interrupted.id)?.is_empty(),
        "orphaned import resurrected approved deletion"
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", interrupted.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(publication_post(&c, &interrupted).await?.status(), 400);
    std::fs::write(
        root.join("results/postgres-http-moderation.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["signed request alone does not remove protected content","operator approval deletes through explicit strfry command, never raw kind5","series deletion has no chapter cascade","wrong-author request cannot remove target","shared originals remain publicly retained","duplicate approval and replay do not recreate work","explicit restore requires revalidation and old approval cannot undo it","parent crash with delayed orphan import cannot resurrect newer approved deletion"],"limits":["synthetic loopback fixture","physical orphan reclamation remains maintenance ticket","no static serving projection yet"]}),
        )?,
    )?;
    Ok(())
}
