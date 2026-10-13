mod support;
use anyhow::{Context, Result};
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
async fn repository(dsn: String) -> Result<Repository> {
    Repository::connect(DbConfig {
        dsn,
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
}
async fn python(root: &std::path::Path, args: Vec<String>) -> Result<Value> {
    let output = tokio::process::Command::new("python3")
        .current_dir(root)
        .env(
            "PYTHONPATH",
            format!(
                "{}:{}",
                root.join("vendor/pydeps").display(),
                root.display()
            ),
        )
        .args(["-m", "src.postgres_migration"])
        .args(args)
        .output()
        .await?;
    anyhow::ensure!(
        output.status.success(),
        "import helper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}
async fn backup_python(
    root: &std::path::Path,
    dsn: &str,
    args: Vec<String>,
) -> Result<std::process::Output> {
    Ok(tokio::process::Command::new("python3")
        .current_dir(root)
        .env(
            "PYTHONPATH",
            format!(
                "{}:{}",
                root.join("vendor/pydeps").display(),
                root.display()
            ),
        )
        .env("MANGA_DATABASE_URL", dsn)
        .args([
            "-m",
            "src.postgres_backup",
            "--pg-bin",
            root.join("vendor/pgsql/bin").to_str().unwrap(),
        ])
        .args(args)
        .output()
        .await?)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL, strfry and prepared Python dependencies"]
async fn reviewed_export_import_is_retryable_and_portable() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture database required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture only");
    let a = repository(dsn.clone()).await?;
    a.migrate().await?;
    a.pool
        .get()
        .await?
        .batch_execute("DROP DATABASE IF EXISTS manga_migration_b WITH (FORCE)")
        .await?;
    a.pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_migration_b")
        .await?;
    let bdsn = dsn.replace("dbname=manga_test", "dbname=manga_migration_b");
    let b = repository(bdsn.clone()).await?;
    b.migrate().await?;
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let origins = ["http://127.0.0.1:19520", "http://127.0.0.1:19521"];
    let mut nodes = Vec::new();
    for (i, dsn) in [dsn, bdsn].into_iter().enumerate() {
        let base = dir.path().join(format!("node-{i}"));
        std::fs::create_dir(&base)?;
        let key = base.join("service.key");
        write_key(&key, 11 + i as u8)?;
        let config = base.join("config.json");
        std::fs::write(
            &config,
            serde_json::to_vec(
                &json!({"bind":format!("127.0.0.1:{}",19520+i),"base_url":origins[i],"service_key_file":key,"admin_pubkey":public(1+i as u8),"database_plaintext_local":true,"group_keys_dir":base.join("group-keys"),"storage":{"path":base.join("objects"),"volume":format!("import{i}"),"physical_bytes":8388608,"staging_bytes":2097152,"max_blob_bytes":1048576},"backend":{"binary":project.join("vendor/strfry/strfry"),"data_dir":base.join("backend"),"map_bytes":67108864}}),
            )?,
        )?;
        let log = std::fs::File::create(base.join("node.log"))?;
        nodes.push(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", dsn)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ));
    }
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    for origin in origins {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if c.get(origin).send().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
    }
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{}/admin/accounts/{}", origins[0], public(1)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    let originals = dir.path().join("source-originals");
    std::fs::create_dir(&originals)?;
    let first = png();
    std::fs::write(originals.join("first.png"), &first)?;
    let mut second = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([91u8, 7, 34])))
        .write_to(&mut second, image::ImageFormat::Png)?;
    let second = second.into_inner();
    std::fs::write(originals.join("second.png"), &second)?;
    let manga = "00000000-0000-4000-8000-000000000001";
    let chapter = "00000000-0000-4000-8000-000000000002";
    let list = "00000000-0000-4000-8000-000000000003";
    let bundle = json!({"series":[{"id":manga,"type":"manga","attributes":{"title":{"en":"Synthetic migrated series"},"status":"ongoing"},"relationships":[]}],"chapters":[{"chapter":{"id":chapter,"type":"chapter","attributes":{"chapter":"001.5","volume":"0","title":null,"pages":2,"translatedLanguage":"en-US","externalUrl":null},"relationships":[{"id":manga,"type":"manga"}]},"ordered_originals":["second.png","first.png"]}],"library":{"read_markers":[chapter],"statuses":{manga:"reading"},"lists":[{"id":list,"type":"custom_list","attributes":{"name":"Private legacy list","visibility":"private"},"relationships":[{"type":"manga","id":manga}]}]}});
    let input = dir.path().join("bundle.json");
    std::fs::write(&input, serde_json::to_vec(&bundle)?)?;
    let plan = dir.path().join("plan.json");
    let journal = dir.path().join("journal.json");
    let key = dir.path().join("publisher.key");
    write_key(&key, 1)?;
    let planned = python(
        project,
        vec![
            "plan".into(),
            input.display().to_string(),
            plan.display().to_string(),
            "--originals".into(),
            originals.display().to_string(),
            "--publisher".into(),
            public(1),
            "--timestamp".into(),
            (auth::now() - 10).to_string(),
        ],
    )
    .await?;
    assert_eq!(planned["publications"], 2);
    assert_eq!(planned["errors"], json!([]));
    let data: Value = serde_json::from_slice(&std::fs::read(&plan)?)?;
    let series_id = data["mapping"]["series"][manga].as_str().unwrap();
    let chapter_id = data["mapping"]["chapters"][chapter].as_str().unwrap();
    let args = vec![
        "apply".into(),
        plan.display().to_string(),
        "--originals".into(),
        originals.display().to_string(),
        "--origin".into(),
        origins[0].into(),
        "--service-key".into(),
        public(11),
        "--key-file".into(),
        key.display().to_string(),
        "--journal".into(),
        journal.display().to_string(),
        "--accept-review".into(),
    ];
    let applied = python(project, args.clone()).await?;
    assert_eq!(applied["completed"], 2);
    assert_eq!(applied["library_published"], false);
    let mapping_file = dir.path().join("legacy-aliases.json");
    std::fs::write(&mapping_file, serde_json::to_vec(&data["mapping"])?)?;
    let alias_args = vec![
        "aliases".into(),
        mapping_file.display().to_string(),
        "--origin".into(),
        origins[0].into(),
        "--service-key".into(),
        public(11),
        "--key-file".into(),
        key.display().to_string(),
        "--accept-review".into(),
    ];
    let aliases = python(project, alias_args.clone()).await?;
    assert_eq!(aliases["aliases_registered"], 2);
    assert_eq!(python(project, alias_args).await?["aliases_registered"], 2);
    let legacy: Value = c
        .get(format!("{}/compat/mangadex/manga/{manga}", origins[0]))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(legacy["data"]["nostr"]["eventId"], series_id);

    assert_eq!(
        c.get(format!("{}/libraries/{}", origins[0], public(1)))
            .send()
            .await?
            .status(),
        404,
        "history is not published without explicit consent"
    );
    let mut with_history = args;
    with_history.push("--publish-public-library".into());
    let applied = python(project, with_history.clone()).await?;
    assert_eq!(applied["library_published"], true);
    let retried = python(project, with_history).await?;
    assert_eq!(retried["completed"], 2);
    assert_eq!(retried["library_published"], true);
    assert_eq!(data["library"]["private_lists_excluded"], 1);
    let chapter_event: auth::Event = c
        .get(format!("{}/events/{chapter_id}", origins[0]))
        .send()
        .await?
        .json()
        .await?;
    chapter_event.verify().map_err(anyhow::Error::msg)?;
    let pointer: Value = serde_json::from_str(&chapter_event.content)?;
    let bytes = c
        .get(format!(
            "{}/originals/{}",
            origins[0],
            pointer["manifest"].as_str().unwrap()
        ))
        .send()
        .await?
        .bytes()
        .await?;
    let manifest = manga_node::manifest::chapter(&bytes).map_err(anyhow::Error::msg)?;
    assert_eq!(manifest.series, series_id);
    assert_eq!(manifest.chapter.as_deref(), Some("001.5"));
    assert_eq!(manifest.pages[0].hash, auth::hash(&second));
    assert_eq!(manifest.pages[1].hash, auth::hash(&first));
    for (origin, admin, peer, endpoint) in [
        (origins[0], 1, 12, origins[1]),
        (origins[1], 2, 11, origins[0]),
    ] {
        assert_eq!(
            post_json(
                &c,
                admin,
                &format!("{origin}/admin/peers"),
                &json!({"key":public(peer),"endpoint":endpoint,"allowed":true})
            )
            .await?
            .status(),
            200
        );
    }
    assert!(
        post_json(
            &c,
            2,
            &format!("{}/admin/sync", origins[1]),
            &json!({"peer":public(11),"event":chapter_id})
        )
        .await?
        .status()
        .is_success()
    );
    wait_projected(&c, origins[1], chapter_id).await?;
    assert!(
        post_json(
            &c,
            2,
            &format!("{}/admin/sync-library", origins[1]),
            &json!({"peer":public(11),"publisher":public(1)})
        )
        .await?
        .status()
        .is_success()
    );
    let library_id = data["library"]["event"]["id"].as_str().unwrap();
    wait_projected(&c, origins[1], library_id).await?;
    for event_id in [series_id, chapter_id, library_id] {
        let visible: auth::Event = c
            .get(format!("{}/events/{event_id}", origins[0]))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        assert_eq!(
            visible.id, event_id,
            "retry retained the reviewed publication identity"
        );
    }
    // Back up an offline source with revocations, a tombstone, a public library
    // and an outstanding worker claim. No production credential/data is used.
    let group: Value = post_json(
        &c,
        1,
        &format!("{}/admin/groups", origins[0]),
        &json!({"administrator":public(1)}),
    )
    .await?
    .json()
    .await?;
    let group_id = group["group"]
        .as_str()
        .context("group creation failed")?
        .to_owned();
    for role in [json!("publisher"), Value::Null] {
        assert_eq!(
            post_json(
                &c,
                1,
                &format!("{}/groups/{group_id}/members", origins[0]),
                &json!({"member":public(9),"role":role})
            )
            .await?
            .status(),
            200
        );
    }
    let deletion = sign_at(
        1,
        5,
        vec![vec!["e".into(), series_id.into()]],
        "Synthetic backup tombstone".into(),
        auth::now(),
    );
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{}/deletion-requests", origins[0]),
            &serde_json::to_value(&deletion)?
        )
        .await?
        .status(),
        202
    );
    assert!(
        post_json(
            &c,
            1,
            &format!("{}/admin/deletions/{}/approve", origins[0], deletion.id),
            &json!({})
        )
        .await?
        .status()
        .is_success()
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if c.get(format!("{}/events/{series_id}", origins[0]))
                .send()
                .await?
                .status()
                == 404
            {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    let old_status_url = format!("{}/admin/status", origins[0]);
    let old_auth = http_get(1, &old_status_url);
    let original_hash = auth::hash(&first);
    let old_blob_auth = blossom(1, origins[0], "get", &original_hash);
    let backup_path = dir.path().join("offline-backup");
    let restored_path = dir.path().join("restored-node");
    let source_config = dir.path().join("node-0/config.json");
    let source_dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    let backup_args = vec![
        "backup".into(),
        source_config.display().to_string(),
        backup_path.display().to_string(),
    ];
    let refused = backup_python(project, &source_dsn, backup_args.clone()).await?;
    assert!(
        !refused.status.success(),
        "live-source backup must be refused"
    );
    assert!(!backup_path.exists());
    drop(nodes.remove(0));
    a.enqueue_sync(manga_node::sync_jobs::SyncSpec {
        peer: public(12),
        inventory: "chapters".into(),
        filter: "pages>=99999".into(),
        fallbacks: vec![],
    })
    .await?;
    let old_claim = a.claim_sync().await?.context("missing fixture claim")?;

    let schedule = a
        .save_sync_schedule(manga_node::sync_schedules::ScheduleSpec {
            spec: manga_node::sync_jobs::SyncSpec {
                peer: public(12),
                inventory: "profiles".into(),
                filter: String::new(),
                fallbacks: vec![],
            },
            interval_seconds: 60,
            enabled: true,
        })
        .await?;
    // Synthetic active/creating/draining reader inhibitors and a derived public
    // hard link must not become stale serving authority on the restore target.
    a.pool.get().await?.batch_execute("INSERT INTO public_export_holds(hash,generation,owner,volume,mime,state,withdrawal,root_identity) SELECT hash,generation,owner,volume,'image/png',CASE n WHEN 1 THEN 'active' WHEN 2 THEN 'creating' ELSE 'draining' END,CASE WHEN n=3 THEN decode(repeat('44',16),'hex') ELSE NULL END,decode(repeat('55',32),'hex') FROM (SELECT hash,generation,owner,volume,row_number() OVER (ORDER BY hash) n FROM blob_intents WHERE state='available' ORDER BY hash LIMIT 3) b").await?;
    let export_source = dir.path().join("node-0/objects");
    let export_public = export_source.join("public");
    let export_shard = export_public
        .join(&original_hash[..2])
        .join(&original_hash[2..4]);
    std::fs::create_dir_all(&export_shard)?;
    let export_generation = a
        .blob_intent(hex::decode(&original_hash)?.try_into().unwrap())
        .await?
        .context("retained original missing")?
        .key
        .generation;
    let export_original = export_source
        .join("objects")
        .join(&original_hash[..2])
        .join(&original_hash[2..4])
        .join(format!(
            "{}.{}",
            original_hash,
            hex::encode(export_generation)
        ));
    std::fs::hard_link(
        &export_original,
        export_shard.join(format!("{original_hash}.png")),
    )?;
    std::fs::create_dir(export_public.join(".serving"))?;
    std::fs::write(
        export_public.join(".serving/state.json"),
        b"synthetic old process identity",
    )?;
    a.pool.get().await?.batch_execute("INSERT INTO public_export_roles(owner,volume,root_identity,backfill_done) SELECT owner,volume,decode(repeat('55',32),'hex'),TRUE FROM blob_intents LIMIT 1;SELECT enqueue_public_export(hash) FROM blob_intents LIMIT 1;INSERT INTO public_export_health_work(hash,generation,owner,volume,root_identity) SELECT b.hash,b.generation,b.owner,b.volume,r.root_identity FROM blob_intents b JOIN public_export_roles r ON r.owner=b.owner AND r.volume=b.volume LIMIT 1").await?;
    // An enabled serving configuration must be paused on restore even though
    // the source is offline and derived process state is intentionally excluded.
    let mut source_settings: Value = serde_json::from_slice(&std::fs::read(&source_config)?)?;
    // Preserve admission state and signup budgets, but never reopen enrollment
    // merely because the offline source configuration enabled it.
    let enrollment_policy = manga_node::enrollment::Policy {
        enabled: true,
        max_new_accounts_per_hour: 4,
        max_self_enrolled_accounts: 12,
    };
    let enrolled_key: [u8; 32] = hex::decode(public(61))?.try_into().unwrap();
    let revoked_key: [u8; 32] = hex::decode(public(62))?.try_into().unwrap();
    assert!(
        a.enroll_reader(enrolled_key, &enrollment_policy)
            .await?
            .created
    );
    assert!(
        a.enroll_reader(revoked_key, &enrollment_policy)
            .await?
            .created
    );
    a.set_account_admission(revoked_key, false).await?;
    source_settings["enrollment"] = serde_json::to_value(&enrollment_policy)?;
    let image_bind = "127.0.0.1:19789";
    source_settings["discovery_private_overrides"] = json!(["https://private.example"]);
    source_settings["public_serving"] = json!({
        "enabled":true,
        "helper":project.join("src/serving_supervisor.py"),
        "nginx":project.join("vendor/nginx-1.28.0/objs/nginx"),
        "bind":image_bind,
        "shutdown_seconds":1
    });
    std::fs::write(&source_config, serde_json::to_vec_pretty(&source_settings)?)?;
    let saved = backup_python(project, &source_dsn, backup_args).await?;
    anyhow::ensure!(
        saved.status.success(),
        "backup failed: {}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let saved: Value = serde_json::from_slice(&saved.stdout)?;
    assert_eq!(saved["identity"]["service"], public(11));
    assert_eq!(saved["derived_public_namespace_excluded"], true);
    assert!(!backup_path.join("storage/public").exists());
    a.pool
        .get()
        .await?
        .batch_execute("DROP DATABASE IF EXISTS manga_restore WITH (FORCE)")
        .await?;
    a.pool
        .get()
        .await?
        .batch_execute("CREATE DATABASE manga_restore")
        .await?;
    let restored_dsn = source_dsn.replace("dbname=manga_test", "dbname=manga_restore");
    let refused = backup_python(
        project,
        &source_dsn,
        vec![
            "restore".into(),
            backup_path.display().to_string(),
            dir.path().join("refused-nonempty").display().to_string(),
            "--source-offline".into(),
        ],
    )
    .await?;
    assert!(!refused.status.success());
    assert!(
        !dir.path().join("refused-nonempty").exists(),
        "nonempty database refusal must precede file writes"
    );
    let protected_key = backup_path.join("service.key");
    let original_key = std::fs::read(&protected_key)?;
    std::fs::write(&protected_key, b"deliberately corrupted fixture backup")?;
    let rejected = backup_python(
        project,
        &restored_dsn,
        vec![
            "restore".into(),
            backup_path.display().to_string(),
            dir.path().join("rejected-corruption").display().to_string(),
            "--source-offline".into(),
        ],
    )
    .await?;
    assert!(
        !rejected.status.success(),
        "changed backup must fail verification"
    );
    let empty = repository(restored_dsn.clone()).await?;
    assert!(
        empty.verify_schema().await.is_err(),
        "tampered backup must not initialize target"
    );
    assert!(
        !restored_path.exists(),
        "tampered backup must not create target files"
    );
    std::fs::write(&protected_key, original_key)?;
    let restored = backup_python(
        project,
        &restored_dsn,
        vec![
            "restore".into(),
            backup_path.display().to_string(),
            restored_path.display().to_string(),
            "--source-offline".into(),
        ],
    )
    .await?;
    anyhow::ensure!(
        restored.status.success(),
        "restore failed: {}",
        String::from_utf8_lossy(&restored.stderr)
    );
    let restored: Value = serde_json::from_slice(&restored.stdout)?;
    assert_eq!(restored["activated"], false);
    let restored_repo = repository(restored_dsn.clone()).await?;
    restored_repo.verify_schema().await?;
    assert!(!restored_path.join("storage/public").exists());
    assert!(export_public.join(".serving/state.json").exists());

    let policies = restored_repo.sync_schedules().await?;
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0]["id"], hex::encode(schedule));
    assert_eq!(policies[0]["enabled"], false);
    assert_eq!(policies[0]["dispatch_error"], "paused_after_restore");
    assert!(
        restored_repo.finish_sync_quantum(&old_claim).await.is_err(),
        "old claim must not finish against restored database"
    );
    let fresh = restored_repo
        .claim_sync()
        .await?
        .context("restored queued job missing")?;
    assert!(fresh.lease.generation > old_claim.lease.generation);
    restored_repo.save_missing(&fresh, vec![]).await?;
    restored_repo.finish_sync_quantum(&fresh).await?;
    assert_eq!(
        restored_repo.sync_status(fresh.batch).await?["state"],
        "completed"
    );
    let config_path = restored["config"]
        .as_str()
        .context("restored config missing")?;
    let restored_settings: Value = serde_json::from_slice(&std::fs::read(config_path)?)?;
    assert_eq!(restored_settings["enrollment"]["enabled"], false);
    assert_eq!(
        restored_settings["enrollment"]["max_new_accounts_per_hour"],
        4
    );
    assert_eq!(
        restored_settings["enrollment"]["max_self_enrolled_accounts"],
        12
    );
    restored_repo.account_permit(enrolled_key).await?;
    assert!(restored_repo.account_permit(revoked_key).await.is_err());
    let new_key: [u8; 32] = hex::decode(public(63))?.try_into().unwrap();
    for policy in [
        manga_node::enrollment::Policy {
            max_new_accounts_per_hour: 2,
            ..enrollment_policy.clone()
        },
        manga_node::enrollment::Policy {
            max_self_enrolled_accounts: 2,
            ..enrollment_policy.clone()
        },
    ] {
        let error = restored_repo
            .enroll_reader(new_key, &policy)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<manga_node::enrollment::Denied>(),
                Some(manga_node::enrollment::Denied::Limit)
            ),
            "restored admission budget must retain earlier creations"
        );
    }
    let denied = restored_repo
        .enroll_reader(revoked_key, &enrollment_policy)
        .await
        .unwrap_err();
    assert!(matches!(
        denied.downcast_ref::<manga_node::enrollment::Denied>(),
        Some(manga_node::enrollment::Denied::Blocked)
    ));
    assert_eq!(restored_settings["discovery_private_overrides"], json!([]));
    assert_eq!(restored_settings["public_serving"]["enabled"], false);
    assert_eq!(restored_settings["public_serving"]["bind"], image_bind);
    assert_eq!(
        restored_settings["public_serving"]["helper"],
        source_settings["public_serving"]["helper"]
    );
    let original_settings: Value = serde_json::from_slice(&std::fs::read(&source_config)?)?;
    assert_eq!(original_settings["public_serving"]["enabled"], true);
    assert_eq!(original_settings["enrollment"]["enabled"], true);
    assert_eq!(
        original_settings["discovery_private_overrides"],
        json!(["https://private.example"])
    );
    let log = std::fs::File::create(dir.path().join("restored.log"))?;
    let restored_node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config_path])
            .env("MANGA_DATABASE_URL", &restored_dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if c.get(origins[0]).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    assert_eq!(
        c.get(&old_status_url)
            .header("authorization", old_auth)
            .send()
            .await?
            .status(),
        403,
        "unused pre-restore NIP98 token must be fenced"
    );
    assert_eq!(
        c.get(format!("{}/blobs/{original_hash}", origins[0]))
            .header("authorization", old_blob_auth)
            .send()
            .await?
            .status(),
        403,
        "pre-restore blob token must be fenced"
    );
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(
        c.get(&old_status_url)
            .header("authorization", http_get(1, &old_status_url))
            .send()
            .await?
            .status(),
        200
    );
    let enrollment: Value = c
        .get(format!("{}/enrollment", origins[0]))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(enrollment["enabled"], false);
    assert_eq!(
        post_json(&c, 63, &format!("{}/enrollment", origins[0]), &json!({}))
            .await?
            .status(),
        403,
        "restored startup must not reopen self-registration"
    );
    let caps: Value = c.get(origins[0]).send().await?.json().await?;
    assert_eq!(caps["service_pubkey"], public(11));
    assert_eq!(
        c.get(format!("{}/events/{chapter_id}", origins[0]))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{}/events/{series_id}", origins[0]))
            .send()
            .await?
            .status(),
        404,
        "moderation tombstone must survive restore"
    );
    assert_eq!(
        c.get(format!("{}/libraries/{}", origins[0], public(1)))
            .send()
            .await?
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{}/originals/{original_hash}", origins[0]))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        first
    );
    let group_hash = auth::hash(&first);
    let upload_url = format!("{}/groups/{group_id}/upload/{group_hash}", origins[0]);
    assert_eq!(
        c.post(&upload_url)
            .header("authorization", http(9, &upload_url, &first))
            .body(first.clone())
            .send()
            .await?
            .status(),
        403,
        "revoked group member must stay revoked"
    );
    assert!(
        restored_path
            .join("groups")
            .join(format!("{group_id}.key"))
            .is_file()
    );
    let generation = restored_repo
        .blob_intent(hex::decode(&original_hash)?.try_into().unwrap())
        .await?
        .context("retained original missing")?
        .key
        .generation;
    let relative = format!(
        "storage/objects/{}/{}/{}.{}",
        &original_hash[..2],
        &original_hash[2..4],
        original_hash,
        hex::encode(generation)
    );
    use std::os::unix::fs::MetadataExt;
    assert_ne!(
        std::fs::metadata(restored_path.join(&relative))?.ino(),
        std::fs::metadata(backup_path.join(&relative))?.ino(),
        "restore must not share source backup inodes"
    );
    // Restore is deliberately paused. Audit and repair remain explicit operator
    // actions; disk corruption is an external fault, never a SQL outcome oracle.
    assert!(
        tokio::net::TcpStream::connect(image_bind).await.is_err(),
        "restore must not activate nginx implicitly"
    );
    let schedules_url = format!("{}/admin/sync-schedules", origins[0]);
    let paused: Value = c
        .get(&schedules_url)
        .header("authorization", http_get(1, &schedules_url))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(paused["schedules"][0]["id"], hex::encode(schedule));
    assert_eq!(paused["schedules"][0]["enabled"], false);
    assert_eq!(
        paused["schedules"][0]["dispatch_error"],
        "paused_after_restore"
    );
    std::fs::write(restored_path.join(&relative), b"corrupt restored original")?;
    let audit_url = format!("{}/admin/storage/audit", origins[0]);
    assert_eq!(
        post_json(&c, 9, &audit_url, &json!({"limit":64}))
            .await?
            .status(),
        403
    );
    let audited: Value = post_json(&c, 1, &audit_url, &json!({"limit":64}))
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(audited["quarantined"], 1);
    assert_eq!(
        c.get(format!("{}/events/{chapter_id}", origins[0]))
            .send()
            .await?
            .status(),
        404
    );
    let repair_url = format!("{}/admin/storage/repair", origins[0]);
    let repair = json!({"peer":public(12),"hash":original_hash});
    assert_eq!(post_json(&c, 9, &repair_url, &repair).await?.status(), 403);
    let repaired: Value = post_json(&c, 1, &repair_url, &repair)
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(repaired["repaired"], true);
    let repeated: Value = post_json(&c, 1, &repair_url, &repair)
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(repeated["repaired"], false);
    assert_eq!(
        c.get(format!("{}/originals/{original_hash}", origins[0]))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .as_ref(),
        first
    );
    let recovered_event: auth::Event = c
        .get(format!("{}/events/{chapter_id}", origins[0]))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        serde_json::to_value(recovered_event)?,
        serde_json::to_value(&chapter_event)?
    );
    assert_eq!(
        c.get(format!("{}/events/{series_id}", origins[0]))
            .send()
            .await?
            .status(),
        404,
        "repair cannot erase a moderation tombstone"
    );

    // The recovered ordinary group key must actually sign new complete content.
    // No personal publisher grant is created for this newly admitted member.
    post_json(
        &c,
        1,
        &format!("{}/groups/{group_id}/members", origins[0]),
        &json!({"member":public(8),"role":"publisher"}),
    )
    .await?
    .error_for_status()?;
    let new_image = {
        let mut bytes = std::io::Cursor::new(vec![]);
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            2,
            2,
            image::Rgb([12u8, 34, 56]),
        ))
        .write_to(&mut bytes, image::ImageFormat::Png)?;
        bytes.into_inner()
    };
    let new_series = cbor(json!({"version":1,"titles":{"en":"After restore"}}));
    for bytes in [&new_image, &new_series] {
        let url = format!(
            "{}/groups/{group_id}/upload/{}",
            origins[0],
            auth::hash(bytes)
        );
        c.post(&url)
            .header("authorization", http(8, &url, bytes))
            .body(bytes.clone())
            .send()
            .await?
            .error_for_status()?;
    }
    let publish_url = format!("{}/groups/{group_id}/publish", origins[0]);
    let new_series_event: Value = post_json(
        &c,
        8,
        &publish_url,
        &json!({"kind":1002,"manifest":auth::hash(&new_series),"version":1}),
    )
    .await?
    .error_for_status()?
    .json()
    .await?;
    let series_event: auth::Event = serde_json::from_value(new_series_event["event"].clone())?;
    series_event.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(series_event.pubkey, group_id);
    wait_projected(&c, origins[0], &series_event.id).await?;
    let new_chapter = cbor(
        json!({"version":1,"series":series_event.id,"language":"en","pages":[page(&new_image)]}),
    );
    let url = format!(
        "{}/groups/{group_id}/upload/{}",
        origins[0],
        auth::hash(&new_chapter)
    );
    c.post(&url)
        .header("authorization", http(8, &url, &new_chapter))
        .body(new_chapter.clone())
        .send()
        .await?
        .error_for_status()?;
    let new_chapter_event: Value = post_json(
        &c,
        8,
        &publish_url,
        &json!({"kind":1001,"manifest":auth::hash(&new_chapter),"version":1}),
    )
    .await?
    .error_for_status()?
    .json()
    .await?;
    let group_chapter: auth::Event = serde_json::from_value(new_chapter_event["event"].clone())?;
    group_chapter.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(group_chapter.pubkey, group_id);
    wait_projected(&c, origins[0], &group_chapter.id).await?;
    assert_eq!(
        c.get(format!(
            "{}/originals/{}",
            origins[0],
            auth::hash(&new_image)
        ))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .as_ref(),
        new_image
    );
    assert_eq!(
        c.post(&upload_url)
            .header("authorization", http(9, &upload_url, &first))
            .body(first.clone())
            .send()
            .await?
            .status(),
        403
    );

    // New remote data appears after the backup. Starting reviewed workers does
    // not itself enable the restored schedule; the operator must do that too.
    post_json(
        &c,
        2,
        &format!("{}/admin/accounts/{}", origins[1], public(2)),
        &json!({"allowed":true}),
    )
    .await?
    .error_for_status()?;
    let profile = sign_at(
        2,
        0,
        vec![],
        "{\"name\":\"After backup\"}".into(),
        auth::now(),
    );
    post_json(
        &c,
        2,
        &format!("{}/account-metadata", origins[1]),
        &serde_json::to_value(&profile)?,
    )
    .await?
    .error_for_status()?;
    wait_projected(&c, origins[1], &profile.id).await?;
    assert_eq!(
        c.get(format!("{}/events/{}", origins[0], profile.id))
            .send()
            .await?
            .status(),
        404
    );
    drop(restored_node);
    let mut activated_settings = restored_settings.clone();
    activated_settings["public_serving"]["enabled"] = json!(true);
    activated_settings["sync_worker_enabled"] = json!(true);
    std::fs::write(config_path, serde_json::to_vec(&activated_settings)?)?;
    let log = std::fs::File::create(dir.path().join("reactivated.log"))?;
    let activated = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config_path])
            .env("MANGA_DATABASE_URL", &restored_dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let image_url = format!("http://{image_bind}/{original_hash}.png");
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Ok(response) = c.get(&image_url).send().await
                && response.status().is_success()
            {
                assert_eq!(response.bytes().await?.as_ref(), first);
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;
    let still_paused: Value = c
        .get(&schedules_url)
        .header("authorization", http_get(1, &schedules_url))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(still_paused["schedules"][0]["enabled"], false);
    assert_eq!(
        still_paused["schedules"][0]["dispatches"],
        paused["schedules"][0]["dispatches"]
    );
    assert_eq!(
        c.get(format!("{}/events/{}", origins[0], profile.id))
            .send()
            .await?
            .status(),
        404
    );
    let enable_url = format!("{schedules_url}/{}/enabled", hex::encode(schedule));
    assert_eq!(
        post_json(&c, 8, &enable_url, &json!({"enabled":true}))
            .await?
            .status(),
        403
    );
    post_json(&c, 1, &enable_url, &json!({"enabled":true}))
        .await?
        .error_for_status()?;
    wait_projected(&c, origins[0], &profile.id).await?;
    let imported: auth::Event = c
        .get(format!("{}/events/{}", origins[0], profile.id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        serde_json::to_value(imported)?,
        serde_json::to_value(&profile)?
    );
    post_json(&c, 1, &enable_url, &json!({"enabled":false}))
        .await?
        .error_for_status()?;
    let status: Value = c
        .get(&old_status_url)
        .header("authorization", http_get(1, &old_status_url))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(status["public_serving"]["state"], "active");
    let enrollment: Value = c
        .get(format!("{}/enrollment", origins[0]))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        enrollment["enabled"], false,
        "serving/sync activation must not reopen enrollment"
    );
    drop(activated);
    std::fs::write(
        project.join("results/postgres-backup-restore.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["live source refuses backup before writing output","offline filesystem owners and exported PostgreSQL snapshot","empty-database restore without automatic activation","nonempty target refused before file writes","tampered backup refused before database restore","service identity and originals preserved","public library preserved","moderation tombstone and revoked group membership preserved","private group key copied without returning it","pre-restore worker claim and unused auth/blob tokens rejected","fresh claim and fresh request auth work","restored file inodes independent of backup","derived public aliases and process state excluded","enabled source public serving paused in restored configuration without altering source","enabled source enrollment remains closed after restored startup","signup creation limits and policy bounds preserved","admitted account preserved and revoked self-enrollment remains blocked","authorized audit and approved-peer repair recover a corrupted restored original without erasing tombstones","newly authorized member publishes fresh original-backed content using the restored group identity","explicit configuration activation starts nginx serving exact repaired originals while recurring policy stays paused","operator re-enables restored recurring policy and real background sync imports a new signed remote profile without reopening enrollment"],"scope":"synthetic co-located loopback instance; not online backup or power-loss certification"}),
        )?,
    )?;

    assert_eq!(
        c.get(format!("{}/originals/{}", origins[1], auth::hash(&second)))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        second
    );
    assert_eq!(
        c.get(format!("{}/libraries/{}", origins[1], public(1)))
            .send()
            .await?
            .status(),
        200
    );
    std::fs::write(
        project.join("results/postgres-migration-http.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["offline reviewed plan uses deterministic exact publication mappings","series and ordered original chapter imported through PostgreSQL HTTP paths","nullable/string labels preserved","history withheld without explicit public consent","private legacy list excluded","same plan retries without duplicate identities","reviewed legacy UUID aliases register through pinned signed HTTP and retry idempotently","signed library points at actual imported chapter ID","approved second node retains chapter and library after source shutdown"],"scope":"synthetic supplied resources and locally generated PNGs; no MangaDex account/API/catalogue accessed"}),
        )?,
    )?;
    Ok(())
}
