//! RECONSTRUCTED UNVERIFIED fixture; rerun after base integration.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::{Result, ensure};
use fixture::*;
use manga_node::{auth, library_manifest::Record};
use serde_json::json;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use weebdex_platform::public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind};
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned PostgreSQL runner and actual strfry"]
async fn real_signed_library_read_markers_are_self_scoped_complete_visible_and_snapshot_bound()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture) -> Result<()> {
    let s=publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":"Reader series"},"contentRating":"safe"})),BTreeMap::new()).await?;
    let s2=publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":"Other reader series"},"contentRating":"safe"})),BTreeMap::new()).await?;
    let image = png();
    let mut chapters = vec![];
    for n in 1..=3 {
        chapters.push(publish(&f.runtime,1001,cbor(json!({"type":"chapter","version":1,"series":s.id,"language":"en","chapter":n.to_string(),"pages":[page(&image)]})),BTreeMap::from([(auth::hash(&image),image.clone())])).await?);
    }
    drain_backend(&f.repo).await?;
    for _ in 0..2 {
        for _ in 0..16 {
            if weebdex_platform::protocol::bridge::import_page(&f.state)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .completed_sweep
            {
                break;
            }
        }
    }
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let rows=f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?.query("SELECT id,protocol_series_id FROM platform.catalog_manga WHERE protocol_series_id=ANY($1::text[])",&[&vec![s.id.clone(),s2.id.clone()]]).await?;
    let mut manga = BTreeMap::new();
    for r in rows {
        let native: String = r.get(0);
        let series: String = r.get(1);
        let alias = resolver
            .preferred(ResourceKind::Manga, &native)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
            .unwrap();
        manga.insert(series, (native, alias));
    }
    let chapter_native: String = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .query_one(
            "SELECT id FROM platform.catalog_chapters WHERE protocol_event_id=$1",
            &[&chapters[0].id],
        )
        .await?
        .get(0);
    let chapter_alias = resolver
        .preferred(ResourceKind::Chapter, &chapter_native)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let now = auth::now() - 30;
    let a = bundle(
        2,
        now,
        vec![
            chapter(&chapters[0], true, None),
            chapter(&chapters[1], false, Some(1)),
            Record::Series {
                id: id(&s.id),
                status: "completed".into(),
                updated_at: 1,
            },
        ],
    )?;
    let b = bundle(3, now, vec![chapter(&chapters[2], true, None)])?;
    save(&f.c, &f.native_origin, 2, &a).await?;
    save(&f.c, &f.native_origin, 3, &b).await?;
    let url = format!("{}/api/v1/manga/{}/read", f.origin, manga[&s.id].1);
    get(&f.c, &url, None, 401).await?;
    response(
        f.c.get(&url)
            .header("authorization", proof(2, &url))
            .body("undeclared"),
        400,
    )
    .await?;
    let first = get(&f.c, &url, Some(2), 200).await?;
    assert_eq!(first["data"], json!([chapter_alias.as_str()]));
    assert_eq!(first["nostr"]["pubkey"], public(2));
    assert_eq!(first["nostr"]["head"], a.event().id);
    let other = get(&f.c, &url, Some(3), 200).await?;
    assert_ne!(other["data"], first["data"]);
    assert_eq!(other["data"].as_array().unwrap().len(), 1);
    assert_eq!(other["nostr"]["pubkey"], public(3));
    let absent = get(&f.c, &url, Some(4), 200).await?;
    assert_eq!(absent["data"], json!([]));
    assert!(absent["nostr"]["head"].is_null());
    let grouped = format!(
        "{}/api/v1/manga/read?ids[]={}&ids[]={}&grouped=true",
        f.origin, manga[&s.id].1, manga[&s2.id].1
    );
    let grouped = get(&f.c, &grouped, Some(2), 200).await?;
    assert_eq!(grouped["data"][manga[&s.id].1.as_str()], first["data"]);
    assert_eq!(grouped["data"][manga[&s2.id].1.as_str()], json!([]));
    for suffix in ["?offset=1", "?limit=10", "?principal=other", "?head=bad"] {
        get(&f.c, &format!("{url}{suffix}"), Some(2), 400).await?;
    }
    let replay = proof(2, &url);
    response(f.c.get(&url).header("authorization", replay.clone()), 200).await?;
    response(f.c.get(&url).header("authorization", replay), 401).await?;
    for event in [
        request_event(2, &url, auth::now() - 61, 11),
        request_event(2, &url, auth::now() + 61, 11),
        request_event(2, &url, auth::now(), 12),
        request_event(2, &format!("{url}?changed=1"), auth::now(), 11),
    ] {
        response(f.c.get(&url).header("authorization", token(&event)), 401).await?;
    }
    // Lock contention must be bounded before authority recheck and free its slot.
    let mut block = f.repo.pool.get().await?;
    let tx = block.transaction().await?;
    tx.query_one(
        "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR UPDATE",
        &[&id(&public(2)).as_slice()],
    )
    .await?;
    let started = Instant::now();
    tokio::time::timeout(Duration::from_secs(3), get(&f.c, &url, Some(2), 503)).await??;
    ensure!(started.elapsed() < Duration::from_secs(3));
    tx.rollback().await?;
    drop(block);
    get(&f.c, &url, Some(2), 200).await?;
    // Local visibility and corrupted projected membership fail closed.
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.catalog_manga SET state='draft' WHERE id=$1",
            &[&manga[&s.id].0],
        )
        .await?;
    get(&f.c, &url, Some(2), 404).await?;
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.catalog_manga SET state='published' WHERE id=$1",
            &[&manga[&s.id].0],
        )
        .await?;
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.catalog_chapters SET manga_id=$2 WHERE id=$1",
            &[&chapter_native, &manga[&s2.id].0],
        )
        .await?;
    let both = format!(
        "{}/api/v1/manga/read?ids[]={}&ids[]={}",
        f.origin, manga[&s.id].1, manga[&s2.id].1
    );
    get(&f.c, &both, Some(2), 503).await?;
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.catalog_chapters SET manga_id=$2 WHERE id=$1",
            &[&chapter_native, &manga[&s.id].0],
        )
        .await?;
    // Real replacement while UUID mapping waits must never mix head snapshots.
    let mut block = f.repo.pool.get().await?;
    let tx = block.transaction().await?;
    tx.batch_execute("LOCK TABLE platform.catalog_chapters IN ACCESS EXCLUSIVE MODE")
        .await?;
    let blocker: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).await?.get(0);
    let req = f.c.get(&url).header("authorization", proof(2, &url));
    let pending = tokio::spawn(async move { response(req, 409).await });
    wait_for_blocked_session(&f.repo, blocker).await?;
    let replacement = bundle(2, now + 1, vec![chapter(&chapters[0], false, None)])?;
    save(&f.c, &f.native_origin, 2, &replacement).await?;
    tx.commit().await?;
    drop(block);
    pending.await??;
    get(&f.c, &format!("{url}?head={}", a.event().id), Some(2), 409).await?;
    assert_eq!(get(&f.c, &url, Some(2), 200).await?["data"], json!([]));
    f.repo.set_account_admission(id(&public(2)), false).await?;
    get(&f.c, &url, Some(2), 403).await?;
    let deletion = auth::sign(
        &format!("{:064x}", 3),
        5,
        vec![vec!["e".into(), b.event().id.clone()]],
        "fixture graph deletion".into(),
    )
    .unwrap();
    let permit = f.repo.account_permit(id(&public(3))).await?;
    assert_eq!(f.repo.delete_ordinary(&permit, &deletion).await?, 1);
    let deleted = get(&f.c, &url, Some(3), 200).await?;
    assert_eq!(deleted["data"], json!([]));
    assert!(deleted["nostr"]["head"].is_null());
    Ok(())
}
