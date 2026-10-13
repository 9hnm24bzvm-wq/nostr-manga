mod support;
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
    upload::UploadStore,
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
async fn ready(c: &reqwest::Client, origin: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(origin).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?;
    Ok(())
}
const PHYSICAL_LIMIT: u64 = 10_000_000;
async fn refused_byte(c: &reqwest::Client, origin: &str) -> Result<()> {
    let bytes = b"Q";
    let hash = auth::hash(bytes);
    let response = c
        .put(format!("{origin}/upload/{hash}"))
        .header("authorization", blossom(1, origin, "upload", &hash))
        .body(bytes.to_vec())
        .send()
        .await?;
    assert_eq!(
        response.status(),
        503,
        "full physical quota must reject another byte"
    );
    assert_eq!(
        c.get(format!("{origin}/blobs/{hash}"))
            .header("authorization", blossom(1, origin, "get", &hash))
            .send()
            .await?
            .status(),
        404
    );
    Ok(())
}
async fn signed_head(c: &reqwest::Client, origin: &str, expected: &auth::Event) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let response = c
                .get(format!("{origin}/libraries/{}", expected.pubkey))
                .send()
                .await?;
            if response.status().is_success() {
                let value: serde_json::Value = response.json().await?;
                let event: auth::Event = serde_json::from_value(value["event"].clone())?;
                event.verify().map_err(anyhow::Error::msg)?;
                assert_eq!(
                    serde_json::to_value(event)?,
                    serde_json::to_value(expected)?
                );
                if value["projected"] == true {
                    return Ok::<_, anyhow::Error>(());
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?
}
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
fn root(chunk: Option<&[u8]>) -> Vec<u8> {
    let descriptor = chunk
        .map(|b| Chunk::decode(b, &Limits::default()).unwrap().1)
        .as_ref()
        .map(Descriptor::from)
        .unwrap_or_else(Descriptor::empty);
    Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![if family == Family::Chapters {
                    descriptor.clone()
                } else {
                    Descriptor::empty()
                }],
            })
            .collect(),
    }
    .encode(&Limits::default())
    .unwrap()
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
fn event(key: u8, root: &[u8], time: u64) -> auth::Event {
    sign_at(
        key,
        11999,
        vec![],
        serde_json::json!({"manifest":auth::hash(root),"version":1}).to_string(),
        time,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and actual strfry"]
async fn real_node_recovers_unlinked_library_without_early_or_double_refund() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database required")?;
    let pg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(pg.get_dbname() == Some("manga_test"), "fixture only");
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
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let config = dir.path().join("node.json");
    let origin = "http://127.0.0.1:19518";
    let mut settings = json!({"bind":"127.0.0.1:19518","base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"library_reclamation":true,"staging_reclamation":false,"storage":{"path":dir.path().join("store"),"volume":"gccrash","physical_bytes":10000000,"staging_bytes":10000000,"max_blob_bytes":1000000,"upload_slots":4},"backend":{"binary":project.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}});
    std::fs::write(&config, serde_json::to_vec(&settings)?)?;
    let spawn = || -> Result<Node> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.path().join("node.log"))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let node = spawn()?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    ready(&c, origin).await?;
    let admission = post_json(
        &c,
        1,
        &format!("{origin}/admin/accounts/{}", public(2)),
        &json!({"allowed":true}),
    )
    .await?;
    assert_eq!(admission.status(), 200);
    let c1 = chunk(1);
    let c2 = chunk(2);
    let r1 = root(Some(&c1));
    let r2 = root(Some(&c2));
    let e1 = event(2, &r1, auth::now() - 20);
    let e2 = event(2, &r2, auth::now() - 19);
    for (ev, expected, ch, rt) in [(&e1, None, &c1, &r1), (&e2, Some(e1.id.as_str()), &c2, &r2)] {
        let response=post_json(&c,2,&format!("{origin}/libraries"),&json!({"event":ev,"expected_head":expected,"objects":[{"type":"chunk","data":STANDARD.encode(ch)},{"type":"root","data":STANDARD.encode(rt)}]})).await?;
        assert!(response.status().is_success(), "{}", response.text().await?);
        signed_head(&c, origin, ev).await?;
        let scan = Command::new(project.join("vendor/strfry/strfry"))
            .arg("--config")
            .arg(dir.path().join("backend/strfry.conf"))
            .args(["scan", &json!({"ids":[ev.id]}).to_string()])
            .output()?;
        assert!(scan.status.success());
        let projected: auth::Event = serde_json::from_slice(&scan.stdout)?;
        projected.verify().map_err(anyhow::Error::msg)?;
        assert_eq!(serde_json::to_value(projected)?, serde_json::to_value(ev)?);
    }
    // A hosted-event acknowledgement cannot certify a retired predecessor.
    // Two sequential real imports establish a later backend batch boundary;
    // actual strfry absence independently confirms the deletion command.
    drain_backend(&repo).await?;
    let scan = Command::new(project.join("vendor/strfry/strfry"))
        .arg("--config")
        .arg(dir.path().join("backend/strfry.conf"))
        .args(["scan", &json!({"ids":[e1.id]}).to_string()])
        .output()?;
    assert!(scan.status.success());
    assert!(
        scan.stdout.is_empty(),
        "superseded root remains in actual backend"
    );
    // Fill the exact remaining capacity through real HTTP admission. These
    // staged probes remain live across the crash and both restarts.
    let input_bytes = (c1.len() + c2.len() + r1.len() + r2.len()) as u64;
    let mut free = PHYSICAL_LIMIT - input_bytes;
    let mut part = 0u64;
    while free > 0 {
        let size = free.min(1_000_000) as usize;
        let mut bytes = vec![0; size];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = ((index as u64 + part * 17) % 251) as u8;
        }
        upload_to(&c, 1, origin, &bytes).await?;
        free -= size as u64;
        part += 1;
    }
    refused_byte(&c, origin).await?;
    let old = [
        repo.blob_intent(id(&auth::hash(&c1))).await?.unwrap(),
        repo.blob_intent(id(&auth::hash(&r1))).await?.unwrap(),
    ];
    let paths: Vec<_> = old
        .iter()
        .map(|b| {
            let h = hex::encode(b.key.hash);
            dir.path()
                .join("store/objects")
                .join(&h[..2])
                .join(&h[2..4])
                .join(format!("{}.{}", h, hex::encode(b.key.generation)))
        })
        .collect();
    assert!(paths.iter().all(|p| p.exists()));
    // Dedicated connection owns the synthetic fault gate, never returns locked to pool.
    let fault = deadpool_postgres::Object::take(repo.pool.get().await?);
    fault
        .execute("SELECT pg_advisory_lock(927442)", &[])
        .await?;
    fault.batch_execute("CREATE FUNCTION fixture_pause_gc() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=OLD.hash AND generation=OLD.generation) THEN PERFORM pg_advisory_xact_lock(927442); END IF; RETURN OLD; END $$; CREATE TRIGGER fixture_pause_gc BEFORE DELETE ON blob_intents FOR EACH ROW EXECUTE FUNCTION fixture_pause_gc();").await?;
    fault.batch_execute("UPDATE library_object_holds SET grace_until=clock_timestamp()-interval '1 second' WHERE state='grace';UPDATE blob_gc_queue SET due_at=clock_timestamp()-interval '1 second';UPDATE account_blob_grants SET lease_until=clock_timestamp()-interval '1 second';UPDATE blob_intents SET staging_until=clock_timestamp()-interval '1 second'").await?;
    let fault_pid: i32 = fault
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let blocked = tokio::time::timeout(Duration::from_secs(15),async {
        loop {
            // External fault synchronization only: actual lock relationships,
            // never the spelling of an implementation query or a claims count.
            let rows=fault.query("WITH RECURSIVE blocked(pid) AS (SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)) UNION SELECT a.pid FROM pg_stat_activity a JOIN blocked b ON b.pid=ANY(pg_blocking_pids(a.pid)) WHERE a.datname=current_database()) SELECT pid FROM blocked", &[&fault_pid]).await?;
            if !rows.is_empty() && paths.iter().all(|p| !p.exists()) {
                return Ok::<_,anyhow::Error>(rows.into_iter().map(|row|row.get::<_,i32>(0)).collect::<Vec<_>>());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.context("GC did not reach the externally blocked after-unlink window")??;
    drop(node);
    // Only synthetic blocked fixture backends are terminated to observe rollback.
    for pid in &blocked {
        fault
            .execute("SELECT pg_terminate_backend($1)", &[pid])
            .await?;
    }
    fault
        .execute("SELECT pg_advisory_unlock(927442)", &[])
        .await?;
    fault
        .batch_execute(
            "DROP TRIGGER fixture_pause_gc ON blob_intents;DROP FUNCTION fixture_pause_gc();",
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !fault
            .query(
                "SELECT pid FROM pg_stat_activity WHERE pid=ANY($1)",
                &[&blocked],
            )
            .await?
            .is_empty()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("terminated GC backends did not close")??;
    drop(fault);
    // No worker is running now. A caller-used store operation proves rollback
    // retained the full charge after both unlinks, before any restart recovery.
    let stopped_store = UploadStore::open(
        repo.clone(),
        serde_json::from_value(settings["storage"].clone())?,
        id(&public(1)),
        id(&public(11)),
    )
    .await?;
    assert_storage_used(&repo, &stopped_store, PHYSICAL_LIMIT, PHYSICAL_LIMIT).await?;
    drop(stopped_store);
    settings["library_reclamation"] = json!(false);
    std::fs::write(&config, serde_json::to_vec(&settings)?)?;
    let node = spawn()?;
    ready(&c, origin).await?;
    // Re-admission of exactly the retired bytes observes durable recovery of
    // both old objects, with new candidate discovery explicitly disabled.
    let recovered_bytes = (c1.len() + r1.len()) as u64;
    let recovery_probe = vec![253u8; recovered_bytes as usize];
    let recovery_hash = auth::hash(&recovery_probe);
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let response = c
                .put(format!("{origin}/upload/{recovery_hash}"))
                .header(
                    "authorization",
                    blossom(1, origin, "upload", &recovery_hash),
                )
                .body(recovery_probe.clone())
                .send()
                .await?;
            if response.status() == 200 {
                return Ok::<_, anyhow::Error>(());
            }
            assert_eq!(
                response.status(),
                503,
                "unexpected recovery admission failure"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("retired capacity was not readmitted after restart")??;
    refused_byte(&c, origin).await?;
    assert_eq!(
        c.get(format!("{origin}/blobs/{recovery_hash}"))
            .header("authorization", blossom(1, origin, "get", &recovery_hash))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        recovery_probe
    );
    signed_head(&c, origin, &e2).await?;
    for bytes in [&c2, &r2] {
        let res = c
            .get(format!("{origin}/library/blobs/{}", auth::hash(bytes)))
            .send()
            .await?;
        assert_eq!(res.status(), 200);
        assert_eq!(res.bytes().await?.as_ref(), bytes.as_slice());
    }
    for bytes in [&c1, &r1] {
        assert_eq!(
            c.get(format!("{origin}/library/blobs/{}", auth::hash(bytes)))
                .send()
                .await?
                .status(),
            404
        );
    }
    drop(node);
    let node = spawn()?;
    ready(&c, origin).await?;
    signed_head(&c, origin, &e2).await?;
    refused_byte(&c, origin).await?;
    assert_eq!(
        c.get(format!("{origin}/blobs/{recovery_hash}"))
            .header("authorization", blossom(1, origin, "get", &recovery_hash))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        recovery_probe
    );
    drop(node);
    // Run one explicit caller-used maintenance quantum after the second node
    // restart. This makes the double-refund check independent of task scheduling.
    let reopened = UploadStore::open(
        repo.clone(),
        serde_json::from_value(settings["storage"].clone())?,
        id(&public(1)),
        id(&public(11)),
    )
    .await?;
    assert_eq!(reopened.reclaim_objects_page(false, false, false).await?, 0);
    assert_storage_used(&repo, &reopened, PHYSICAL_LIMIT, PHYSICAL_LIMIT).await?;
    std::fs::write(
        project.join("results/library-gc-process-recovery.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"physical_capacity_filled":PHYSICAL_LIMIT,"retired_bytes_readmitted":recovered_bytes,"checks":["actual node killed after two generation unlinks while finalization was externally blocked","full-capacity upload admission remains closed after crash rollback before recovery","restart recovers committed claims with new reclamation disabled","current root and chunk remain readable","second restart does not double refund"],"limitations":["synthetic loopback fixture","process kill, not power loss","grace and staging leases advanced by fixture SQL","not a throughput benchmark"]}),
        )?,
    )?;
    Ok(())
}
