mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
    endpoint_policy::EndpointPolicy,
    library_client::{self, DeliveryLimits, Outcome, Recipient, Signer, SigningRequest},
    library_client_bundle::SaveBundle,
    library_client_recovery,
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    process::{Child, Command},
    sync::Arc,
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
struct TestSigner(u8);
#[async_trait::async_trait]
impl Signer for TestSigner {
    async fn sign(&self, r: SigningRequest) -> Result<auth::Event> {
        Ok(sign_at(self.0, r.kind, r.tags, r.content, r.created_at))
    }
}
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
fn bundle(time: u64) -> Result<SaveBundle> {
    let limits = Limits::default();
    let chunk = Chunk::new(
        Family::Chapters,
        vec![Record::Chapter {
            id: [7; 32],
            completed: true,
            page: Some(12),
            updated_at: 1,
        }],
        &limits,
    )?
    .encode(&limits)?;
    let summary = Chunk::decode(&chunk, &limits)?.1;
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
    .encode(&limits)?;
    let event = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        time,
    );
    SaveBundle::validate(event, root, BTreeMap::from([(summary.hash(), chunk)]))
}
async fn repo(dsn: String) -> Result<Repository> {
    Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
}
fn large_bundle(count: usize, buckets: usize, time: u64) -> Result<SaveBundle> {
    let limits = Limits::default();
    let mut partitioned: Vec<Vec<Record>> = (0..buckets).map(|_| vec![]).collect();
    for n in 0..count {
        let mut key = [0u8; 32];
        key[24..].copy_from_slice(&(n as u64 + 1).to_be_bytes());
        let bucket = manga_node::library_manifest::bucket_for(&key, buckets)?;
        partitioned[bucket].push(Record::Chapter {
            id: key,
            completed: n % 5 != 0,
            page: if n % 5 == 0 {
                Some((n % 30 + 1) as u32)
            } else {
                None
            },
            updated_at: time.saturating_sub((count - n) as u64),
        });
    }
    let mut chunks = BTreeMap::new();
    let mut descriptors = vec![];
    for records in partitioned {
        if records.is_empty() {
            descriptors.push(Descriptor::empty());
            continue;
        }
        let bytes = Chunk::new(Family::Chapters, records, &limits)?.encode(&limits)?;
        let summary = Chunk::decode(&bytes, &limits)?.1;
        descriptors.push(Descriptor::from(&summary));
        chunks.insert(summary.hash(), bytes);
    }
    let root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: if family == Family::Chapters {
                    descriptors.clone()
                } else {
                    vec![Descriptor::empty()]
                },
            })
            .collect(),
    }
    .encode(&limits)?;
    let event = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        time,
    );
    SaveBundle::validate(event, root, chunks)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL and multiple actual HTTP nodes"]
async fn first_acceptance_retry_supersession_and_cold_recovery() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let a = repo(dsn.clone()).await?;
    a.pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_client_b")
        .await?;
    let dsns = [
        dsn.clone(),
        dsn.replace("dbname=manga_test", "dbname=manga_client_b"),
    ];
    for dsn in &dsns {
        repo(dsn.clone()).await?.migrate().await?;
    }
    let dir = tempfile::tempdir()?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let mut origins = vec![];
    let mut paths = vec![];
    for n in 0..2 {
        let sock = std::net::TcpListener::bind("127.0.0.1:0")?;
        let bind = sock.local_addr()?;
        drop(sock);
        let origin = format!("http://{bind}");
        let key = dir.path().join(format!("key{n}"));
        write_key(&key, 11 + n as u8)?;
        let path = dir.path().join(format!("config{n}.json"));
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"bind":bind.to_string(),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"storage":{"path":dir.path().join(format!("objects{n}")),"volume":format!("client{n}"),"physical_bytes":134217728,"staging_bytes":134217728,"max_blob_bytes":1000000},"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join(format!("backend{n}")),"map_bytes":67108864}}),
            )?,
        )?;
        origins.push(origin);
        paths.push(path);
    }
    let spawn = |n: usize| -> Result<Node> {
        let log = std::fs::File::create(dir.path().join(format!("node{n}.log")))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", paths[n].to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsns[n])
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let mut nodes = [Some(spawn(0)?), Some(spawn(1)?)];
    let client = reqwest::Client::new();
    for (node_index, origin) in origins.iter().enumerate() {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if client.get(origin).send().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .with_context(|| {
            format!(
                "node {node_index} failed readiness: {}",
                std::fs::read_to_string(dir.path().join(format!("node{node_index}.log")))
                    .unwrap_or_default()
            )
        })?;
        ensure!(
            post_json(
                &client,
                1,
                &format!("{origin}/admin/accounts/{}", public(2)),
                &json!({"allowed":true})
            )
            .await?
            .status()
                == 200,
            "account admission"
        );
    }
    // A peer that accepts TCP but never answers identity must not delay success elsewhere.
    let stall = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let stall_origin = format!("http://{}", stall.local_addr()?);
    let stalled = tokio::spawn(async move {
        let mut sockets = vec![];
        loop {
            let (socket, _) = stall.accept().await.unwrap();
            sockets.push(socket);
        }
    });
    let all_origins = [origins.clone(), vec![stall_origin.clone()]].concat();
    let policy = EndpointPolicy::new(&all_origins)?;
    let recipients: Vec<_> = origins
        .iter()
        .enumerate()
        .map(|(n, endpoint)| Recipient {
            relay_url: format!("wss://relay{n}.example"),
            endpoint: endpoint.clone(),
            service_key: id(&public(11 + n as u8)),
        })
        .collect();
    let root_routes: Vec<_> = origins
        .iter()
        .map(|origin| origin.replacen("http://", "ws://", 1))
        .collect();
    let auto_list = sign_at(
        2,
        10002,
        root_routes
            .iter()
            .map(|r| vec!["r".into(), r.clone(), "write".into()])
            .chain([vec![
                "r".into(),
                format!("{}/unsupported", root_routes[0]),
                "write".into(),
            ]])
            .collect(),
        String::new(),
        auth::now(),
    );
    let automatic = manga_node::library_client_discovery::discover(
        &auto_list,
        id(&public(2)),
        vec![],
        policy.clone(),
    )
    .await?;
    assert_eq!(automatic.recipients.len(), 2);
    assert_eq!(automatic.first_observations.len(), 2);
    assert_eq!(automatic.unresolved.len(), 1);
    assert_eq!(automatic.unresolved[0].reason, "explicit_binding_required");
    let remembered = manga_node::library_client_discovery::discover(
        &auto_list,
        id(&public(2)),
        automatic.recipients.clone(),
        policy.clone(),
    )
    .await?;
    assert_eq!(remembered.recipients.len(), 2);
    assert!(remembered.first_observations.is_empty());
    let mut changed = automatic.recipients;
    changed[0].service_key = id(&public(15));
    let changed = manga_node::library_client_discovery::discover(
        &auto_list,
        id(&public(2)),
        changed,
        policy.clone(),
    )
    .await?;
    assert_eq!(
        changed.recipients.len(),
        1,
        "remembered key mismatch cannot silently reset identity"
    );
    assert_eq!(changed.unresolved.len(), 2);
    let signer: Arc<dyn Signer> = Arc::new(TestSigner(2));
    let relay_list = sign_at(
        2,
        10002,
        recipients
            .iter()
            .map(|r| vec!["r".into(), r.relay_url.clone(), "write".into()])
            .chain([vec![
                "r".into(),
                "wss://read-only.example".into(),
                "read".into(),
            ]])
            .collect(),
        String::new(),
        auth::now() - 1,
    );
    for recipient in &recipients {
        assert!(matches!(
            manga_node::library_client_relays::publish(
                &relay_list,
                recipient,
                signer.as_ref(),
                &policy
            )
            .await?,
            Outcome::Accepted { .. }
        ));
        wait_projected(&client, &recipient.endpoint, &relay_list.id).await?;
    }
    let discovered = manga_node::library_client_relays::fetch(
        id(&public(2)),
        recipients.clone(),
        signer.clone(),
        policy.clone(),
    )
    .await?;
    assert_eq!(discovered.event.id, relay_list.id);
    let plan =
        library_client::plan_write_relays(&discovered.event, id(&public(2)), recipients.clone())?;
    assert_eq!(plan.recipients.len(), 2);
    assert!(plan.unresolved_relays.is_empty());
    let original = Arc::new(bundle(auth::now() - 2)?);
    let event_id = original.event().id.clone();
    let slow = Recipient {
        relay_url: "wss://slow.example".into(),
        endpoint: stall_origin,
        service_key: id(&public(15)),
    };
    let delivery = library_client::deliver(
        original.clone(),
        [vec![slow], recipients.clone()].concat(),
        signer.clone(),
        policy.clone(),
        DeliveryLimits {
            concurrency: 3,
            recipient_timeout: Duration::from_secs(3),
        },
    )?;
    let first = tokio::time::timeout(Duration::from_secs(2), delivery.first_acceptance).await??;
    assert!(matches!(first.outcome, Outcome::Accepted { .. }));
    assert!(
        !delivery.completion.is_finished(),
        "slow recipient still pending after first acceptance"
    );
    let results = delivery.completion.await?;
    assert!(matches!(results[0].outcome, Outcome::Retryable { .. }));
    assert!(
        results[1..]
            .iter()
            .all(|r| matches!(r.outcome, Outcome::Accepted { .. }))
    );
    stalled.abort();
    for origin in &origins {
        wait_projected(&client, origin, &event_id).await?;
    }
    let duplicate = library_client::deliver(
        original.clone(),
        vec![recipients[1].clone()],
        signer.clone(),
        policy.clone(),
        DeliveryLimits::default(),
    )?
    .completion
    .await?;
    assert!(matches!(duplicate[0].outcome, Outcome::Accepted { .. }));
    // Persist only the first receipt, then close/reopen as after interrupted work.
    use manga_node::library_client_outbox::{Outbox, OutboxLimits, run_job};
    let queue_dir = dir.path().join("client-outbox");
    let mut queue = Outbox::open(&queue_dir, OutboxLimits::default())?;
    assert!(Outbox::open(&queue_dir, OutboxLimits::default()).is_err());
    let job = queue.enqueue(&original, recipients.clone(), vec![])?;
    let attempts = queue.begin(&job)?;
    assert_eq!(attempts.len(), 2);
    assert!(queue.record(&job, 0, attempts[0].1, results[1].outcome.clone())?);
    drop(queue);
    let queue = std::sync::Arc::new(std::sync::Mutex::new(Outbox::open(
        &queue_dir,
        OutboxLimits::default(),
    )?));
    let durable = run_job(
        queue.clone(),
        job.clone(),
        signer.clone(),
        policy.clone(),
        DeliveryLimits::default(),
    )
    .await?;
    let _ = durable.first_acceptance.await?;
    let saved = durable.completion.await??;
    assert_eq!(
        saved.recipients[0].attempts, 1,
        "already accepted destination is not resent"
    );
    assert_eq!(
        saved.recipients[1].attempts, 2,
        "unfinished destination resumes idempotently"
    );
    assert!(
        saved
            .recipients
            .iter()
            .all(|state| matches!(state.outcome, Some(Outcome::Accepted { .. })))
    );
    let raw = std::fs::read_to_string(queue_dir.join(&job).join("state.json"))?;
    assert!(
        !raw.contains("authorization")
            && !raw.contains("secret")
            && !raw.contains(&format!("{:064x}", 2))
    );
    assert_eq!(queue.lock().unwrap().bundle(&job)?.event().id, event_id);
    // Stop the first accepted copy; a fresh recovery uses the surviving node.
    drop(nodes[0].take());
    // Cold-device workflow: one surviving known seed provides the owner's
    // signed NIP65 list; discover its reference root APIs without saved
    // per-recipient bindings, then recover the exact signed library graph.
    let cold_announcement = sign_at(
        2,
        10002,
        auto_list.tags.clone(),
        String::new(),
        auth::now().max(relay_list.created_at + 1),
    );
    assert!(matches!(
        manga_node::library_client_relays::publish(
            &cold_announcement,
            &recipients[1],
            signer.as_ref(),
            &policy
        )
        .await?,
        Outcome::Accepted { .. }
    ));
    wait_projected(&client, &origins[1], &cold_announcement.id).await?;
    let cold_list = manga_node::library_client_relays::fetch(
        id(&public(2)),
        vec![recipients[1].clone()],
        signer.clone(),
        policy.clone(),
    )
    .await?;
    assert_eq!(cold_list.event.id, cold_announcement.id);
    let cold_routes = manga_node::library_client_discovery::discover(
        &cold_list.event,
        id(&public(2)),
        vec![],
        policy.clone(),
    )
    .await?;
    assert_eq!(cold_routes.recipients.len(), 1);
    assert_eq!(cold_routes.first_observations.len(), 1);
    assert_eq!(
        cold_routes.unresolved.len(),
        2,
        "offline root and unsupported path remain visible"
    );
    let cold_library = library_client_recovery::recover(
        id(&public(2)),
        cold_routes.recipients,
        signer.clone(),
        policy.clone(),
    )
    .await?;
    assert_eq!(cold_library.bundle.event().id, event_id);

    let recovered = library_client_recovery::recover(
        id(&public(2)),
        recipients.clone(),
        signer.clone(),
        policy.clone(),
    )
    .await?;
    assert_eq!(recovered.bundle.event().id, event_id);
    assert_eq!(recovered.bundle.object_count(), 2);
    assert_eq!(recovered.unavailable_recipients, vec![0]);
    let newer = Arc::new(bundle(auth::now())?);
    let newer_results = library_client::deliver(
        newer.clone(),
        vec![recipients[1].clone()],
        signer.clone(),
        policy.clone(),
        DeliveryLimits::default(),
    )?
    .completion
    .await?;
    assert!(matches!(newer_results[0].outcome, Outcome::Accepted { .. }));
    let old = library_client::deliver(
        original.clone(),
        vec![recipients[1].clone()],
        signer.clone(),
        policy.clone(),
        DeliveryLimits::default(),
    )?
    .completion
    .await?;
    assert!(matches!(old[0].outcome, Outcome::Superseded { .. }));
    let mut wrong = recipients[1].clone();
    wrong.service_key = id(&public(19));
    let wrong = library_client::deliver(
        original.clone(),
        vec![wrong],
        signer.clone(),
        policy.clone(),
        DeliveryLimits::default(),
    )?
    .completion
    .await?;
    assert_eq!(
        wrong[0].outcome,
        Outcome::Rejected {
            reason: "service_key_mismatch".into()
        }
    );
    let changed_signer: Arc<dyn Signer> = Arc::new(TestSigner(3));
    let changed = library_client::deliver(
        original.clone(),
        vec![recipients[1].clone()],
        changed_signer,
        policy.clone(),
        DeliveryLimits::default(),
    )?
    .completion
    .await?;
    assert!(matches!(changed[0].outcome, Outcome::Retryable { .. }));
    // Functional full-graph cases, not throughput benchmarks.
    let mut large_reports = vec![];
    let mut last_time = auth::now();
    for (count, buckets) in [(30_000usize, 64usize), (500_000usize, 256usize)] {
        last_time = last_time.max(auth::now()).saturating_add(1);
        let graph = Arc::new(large_bundle(count, buckets, last_time)?);
        assert!(graph.object_count() > 64);
        let raw_bytes: usize = graph.objects().map(|(_, _, bytes)| bytes.len()).sum();
        let large_dir = dir.path().join(format!("outbox-{count}"));
        let mut disk = Outbox::open(&large_dir, OutboxLimits::default())?;
        let job = disk.enqueue(&graph, vec![recipients[1].clone()], vec![])?;
        let first = graph.batch(0)?.unwrap();
        assert!(first.next_object < graph.object_count());
        // Interrupt after a successful object batch and before head publication.
        let url = format!("{}/library/batches", origins[1]);
        let previous: serde_json::Value = client
            .get(format!("{}/libraries/{}", origins[1], public(2)))
            .send()
            .await?
            .json()
            .await?;
        let partial = client
            .post(&url)
            .header("authorization", http(2, &url, &first.body))
            .body(first.body)
            .send()
            .await?;
        ensure!(partial.status() == 200, "partial batch staging");
        let after: serde_json::Value = client
            .get(format!("{}/libraries/{}", origins[1], public(2)))
            .send()
            .await?
            .json()
            .await?;
        assert_eq!(
            after["event"]["id"], previous["event"]["id"],
            "partial object delivery never advances the public head"
        );
        let attempts = disk.begin(&job)?;
        assert_eq!(attempts[0].1, 1);
        drop(disk);
        let disk = Arc::new(std::sync::Mutex::new(Outbox::open(
            &large_dir,
            OutboxLimits::default(),
        )?));
        let run = run_job(
            disk.clone(),
            job.clone(),
            signer.clone(),
            policy.clone(),
            DeliveryLimits::default(),
        )
        .await?;
        assert!(matches!(
            run.first_acceptance.await?.outcome,
            Outcome::Accepted { .. }
        ));
        let completed = run.completion.await??;
        assert_eq!(completed.recipients[0].attempts, 2);
        wait_projected(&client, &origins[1], &job).await?;
        let restored = library_client_recovery::recover(
            id(&public(2)),
            vec![recipients[1].clone()],
            signer.clone(),
            policy.clone(),
        )
        .await?;
        assert_eq!(restored.bundle.event().id, job);
        let records = restored
            .bundle
            .objects()
            .filter(|(kind, _, _)| *kind == "chunk")
            .map(|(_, _, bytes)| {
                Chunk::decode(bytes, &Limits::default())
                    .unwrap()
                    .1
                    .records()
            })
            .sum::<u64>();
        assert_eq!(records, count as u64);
        // The same recovered graph must support bounded reader views after the
        // first node is offline, including the tail of an extreme library.
        for (offset, expected) in [(0usize, 100usize), (count - 1, 1), (count, 0)] {
            let url = format!(
                "{}/libraries/{}/entries?family=chapters&head={job}&offset={offset}&limit=100",
                origins[1],
                public(2)
            );
            let response = client.get(&url).send().await?;
            ensure!(
                response.status().is_success(),
                "survivor entries: {}",
                response.text().await?
            );
            let page: serde_json::Value = response.json().await?;
            assert_eq!(page["event"]["id"], job);
            assert_eq!(page["total"], count);
            assert_eq!(page["entries"].as_array().unwrap().len(), expected);
            assert!(page["chunks"].as_array().unwrap().len() <= 100);
        }
        large_reports.push(json!({"chapters":count,"objects":graph.object_count(),"canonical_bytes":raw_bytes,"interrupted_after_objects":first.next_object,"complete_recovery":true,"survivor_reader_first_last_and_empty_pages":true}));
    }
    std::fs::write(
        root.join("results/http-library-client-large.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"cases":large_reports,"scope":"Functional interrupted multi-batch delivery and full cold recovery, not performance/load qualification"}),
        )?,
    )?;

    ensure!(
        post_json(
            &client,
            1,
            &format!("{}/admin/accounts/{}", origins[1], public(2)),
            &json!({"allowed":false})
        )
        .await?
        .status()
            == 200,
        "revoke account"
    );
    let revoked = library_client::deliver(
        original,
        vec![recipients[1].clone()],
        signer,
        policy,
        DeliveryLimits::default(),
    )?
    .completion
    .await?;
    assert_eq!(
        revoked[0].outcome,
        Outcome::Rejected {
            reason: "http_403".into()
        }
    );
    std::fs::write(
        root.join("results/http-library-client.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["signed NIP65 lists publish, rediscover and select only write routes","automatic root-route binding verifies keys and rejects remembered-key changes","cold device bootstraps NIP65, bindings and exact library graph from one surviving seed","first verified acceptance precedes stalled recipient completion","same signed save accepted independently by two nodes","duplicate retry remains accepted","persistent outbox resumes only unfinished recipients and stores no signing secrets","cold recovery from surviving replica verifies full chunk graph","older head returns superseded","wrong service key rejected","signer substitution rejected","revoked account denied"],"limits":["automatic reference root-route discovery; other paths require explicit bindings; host persists first observations","bounded outbox retry budget; owner-controlled local filesystem","synthetic loopback functional scenarios; no performance qualification"]}),
        )?,
    )?;
    Ok(())
}
