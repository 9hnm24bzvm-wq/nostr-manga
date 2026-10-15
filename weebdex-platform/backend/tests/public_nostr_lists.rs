//! RECONSTRUCTED UNVERIFIED genuine signed-list HTTP/backfill fixture.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::{Context, Result, ensure};
use fixture::*;
use manga_node::{auth, library_manifest::Record, mangadex_compat};
use serde_json::json;
use std::collections::BTreeMap;
use weebdex_platform::{
    protocol::{bridge, list_aliases},
    public_api::{
        PostgresPublicIdResolver, PublicIdResolver, PublicIdTarget, PublicUuid, ResourceKind,
    },
};
fn list(n: u8, name: &str) -> Record {
    Record::List {
        id: [n; 16],
        name: name.into(),
        updated_at: 1,
    }
}
fn member(n: u8, series: &auth::Event, position: u64) -> Record {
    Record::Member {
        list: [n; 16],
        series: id(&series.id),
        position,
        updated_at: 1,
    }
}
async fn project(f: &Fixture) -> Result<()> {
    for _ in 0..32 {
        if list_aliases::import_page(&f.state)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
            .completed_sweep
        {
            return Ok(());
        }
    }
    anyhow::bail!("bounded fixture alias sweep did not complete")
}
async fn alias_count(f: &Fixture) -> Result<(i64, i64)> {
    let r=f.repo.pool.get().await?.query_one("SELECT (SELECT count(*) FROM platform.public_resource_ids),(SELECT count(*) FROM compatibility_ids)",&[]).await?;
    Ok((r.get(0), r.get(1)))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned PostgreSQL runner and actual strfry"]
async fn real_signed_public_lists_preserve_owner_scope_aliases_visibility_feeds_and_head_consistency()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f, true).await;
    f.shutdown().await?;
    result
}
/// Core graph/list reads have independent evidence while the shared chapter
/// feed adapter is restored. This test never claims feed compatibility.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned PostgreSQL runner and actual strfry"]
async fn real_signed_public_list_core_reads_preserve_owner_scope_bounds_and_current_heads()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f, false).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture, exercise_feed: bool) -> Result<()> {
    let s1 = publish(
        &f.runtime,
        1002,
        cbor(
            json!({"type":"series","version":1,"titles":{"en":"List one"},"contentRating":"safe"}),
        ),
        BTreeMap::new(),
    )
    .await?;
    let s2 = publish(
        &f.runtime,
        1002,
        cbor(
            json!({"type":"series","version":1,"titles":{"en":"List two"},"contentRating":"safe"}),
        ),
        BTreeMap::new(),
    )
    .await?;
    let image = png();
    for (series, language) in [(&s1, "en"), (&s2, "ja")] {
        publish(&f.runtime,1001,cbor(json!({"type":"chapter","version":1,"series":series.id,"language":language,"chapter":"1","pages":[page(&image)]})),BTreeMap::from([(auth::hash(&image),image.clone())])).await?;
    }
    drain_backend(&f.repo).await?;
    for _ in 0..2 {
        for _ in 0..4 {
            if bridge::import_page(&f.state)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .completed_sweep
            {
                break;
            }
        }
    }
    let now = auth::now() - 30;
    let a = bundle(
        2,
        now,
        vec![
            list(1, "Empty"),
            list(2, "Second"),
            list(5, "Owner two"),
            member(5, &s1, 9),
            member(5, &s2, 1),
        ],
    )?;
    let b = bundle(3, now, vec![list(5, "Owner three"), member(5, &s2, 1)])?;
    save(&f.c, &f.native_origin, 2, &a).await?;
    save(&f.c, &f.native_origin, 3, &b).await?;
    drain_backend(&f.repo).await?;
    let mine = format!("{}/api/v1/user/list", f.origin);
    get(&f.c, &mine, None, 401).await?;
    response(
        f.c.get(&mine)
            .header("authorization", proof(2, &mine))
            .body("undeclared"),
        400,
    )
    .await?;
    get(&f.c, &mine, Some(2), 503).await?;
    let absent = get(&f.c, &mine, Some(4), 200).await?;
    assert_eq!(absent["data"], json!([]));
    assert!(absent["nostr"]["head"].is_null());
    let native_url = format!(
        "{}/compat/mangadex/user/{}/list",
        f.native_origin,
        public(2)
    );
    let r = f.c.get(&native_url).send().await?;
    ensure!(
        r.status() == 200,
        "original list fixture: {}",
        r.text().await?
    );
    let original_main =
        mangadex_compat::uuid("list", &format!("{}:{}", public(2), hex::encode([5; 16])));
    let preserved = "33333333-3333-4333-8333-333333333333";
    f.repo
        .register_compatibility_alias(
            "list",
            preserved,
            &format!("{}:{}", public(2), hex::encode([5; 16])),
            true,
        )
        .await?;
    project(f).await?;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let owner_alias = resolver
        .preferred(ResourceKind::User, &format!("protocol:{}", public(2)))
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .context("projected owner alias")?;
    let target = resolver
        .resolve(ResourceKind::List, &PublicUuid::parse(preserved)?)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .context("typed list alias")?;
    assert_eq!(
        target.target,
        PublicIdTarget::ProtocolList {
            owner: public(2),
            list_id: hex::encode([5; 16])
        }
    );
    let before = alias_count(f).await?;
    let all = get(&f.c, &mine, Some(2), 200).await?;
    assert_eq!(all["total"], 3);
    assert_eq!(all["data"][2]["id"], preserved);
    assert_eq!(
        all["data"][2]["attributes"],
        json!({"name":"Owner two","visibility":"public"})
    );
    assert_eq!(all["data"][2]["nostr"]["listId"], hex::encode([5; 16]));
    assert_eq!(
        all["data"][2]["relationships"][0]["id"],
        owner_alias.as_str()
    );
    assert_eq!(
        all["data"][2]["relationships"][1]["nostr"]["eventId"],
        s2.id
    );
    assert_eq!(
        all["data"][2]["relationships"][2]["nostr"]["eventId"],
        s1.id
    );
    let second = get(&f.c, &format!("{mine}?limit=1&offset=1"), Some(2), 200).await?;
    assert_eq!(second["data"][0]["attributes"]["name"], "Second");
    assert_eq!(second["offset"], 1);
    assert_eq!(second["total"], 3);
    let zero = get(&f.c, &format!("{mine}?limit=0&offset=2"), Some(2), 200).await?;
    assert_eq!(zero["data"], json!([]));
    assert_eq!(zero["total"], 3);
    let other = get(&f.c, &mine, Some(3), 200).await?;
    assert_eq!(other["total"], 1);
    assert_eq!(other["data"][0]["attributes"]["name"], "Owner three");
    assert_ne!(other["data"][0]["id"], preserved);
    let public_owner = format!("{}/api/v1/user/{owner_alias}/list", f.origin);
    assert_eq!(
        get(&f.c, &public_owner, None, 200).await?["data"],
        all["data"]
    );
    let detail = format!("{}/api/v1/list/{preserved}", f.origin);
    assert_eq!(get(&f.c, &detail, None, 200).await?["data"], all["data"][2]);
    assert_eq!(
        get(
            &f.c,
            &format!("{}/api/v1/list/{original_main}", f.origin),
            None,
            200
        )
        .await?["data"]["id"],
        preserved
    );
    for url in [
        format!("{mine}?privacy=private"),
        format!("{mine}?includes[]=user"),
        format!("{detail}?limit=1"),
        format!("{detail}?head=bad"),
    ] {
        get(&f.c, &url, Some(2), 400).await?;
    }
    get(
        &f.c,
        &format!("{}/api/v1/list/not-uuid", f.origin),
        None,
        400,
    )
    .await?;
    get(
        &f.c,
        &format!(
            "{}/api/v1/list/11111111-1111-4111-8111-111111111111",
            f.origin
        ),
        None,
        404,
    )
    .await?;
    let feed = format!("{detail}/feed");
    if exercise_feed {
        assert_eq!(get(&f.c, &feed, None, 200).await?["total"], 2);
        let en = get(
            &f.c,
            &format!("{feed}?translatedLanguage[]=en&includes[]=manga&limit=1&offset=0"),
            None,
            200,
        )
        .await?;
        assert_eq!(en["total"], 1);
        assert_eq!(en["data"][0]["attributes"]["translatedLanguage"], "en");
        let empty = all["data"][0]["id"].as_str().unwrap();
        assert_eq!(
            get(
                &f.c,
                &format!("{}/api/v1/list/{empty}/feed", f.origin),
                None,
                200
            )
            .await?["total"],
            0
        );
        get(&f.c, &format!("{feed}?privacy=private"), None, 400).await?;
        get(&f.c, &format!("{feed}?limit=%ZZ"), None, 400).await?;
    }
    assert_eq!(
        alias_count(f).await?,
        before,
        "public GETs must not write either registry"
    );
    let native_owner = "44444444-4444-4444-8444-444444444444";
    let db = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    db.execute("INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES($1,'LocalOnly','local@fixture.invalid','synthetic-unused',TRUE)",&[&native_owner]).await?;
    db.execute("INSERT INTO platform.library_lists(id,user_id,name,visibility) VALUES('NativeOnly1',$1,'Private native secret','private')",&[&native_owner]).await?;
    drop(db);
    let native_list = resolver
        .preferred(ResourceKind::List, "NativeOnly1")
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    get(
        &f.c,
        &format!("{}/api/v1/user/{native_owner}/list", f.origin),
        None,
        404,
    )
    .await?;
    get(
        &f.c,
        &format!("{}/api/v1/list/{native_list}", f.origin),
        None,
        404,
    )
    .await?;
    let many = bundle(
        4,
        now,
        (1..=101)
            .map(|n| Record::List {
                id: [n; 16],
                name: format!("List {n}"),
                updated_at: 1,
            })
            .collect(),
    )?;
    save(&f.c, &f.native_origin, 4, &many).await?;
    let mut saw_pending = false;
    for _ in 0..32 {
        let report = list_aliases::import_page(&f.state)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert!(report.aliases <= 100);
        let pending:bool=f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_public_list_alias_projection WHERE publisher=$1 AND NOT completed)",&[&public(4)]).await?.get(0);
        if pending {
            saw_pending = true;
            get(&f.c, &mine, Some(4), 503).await?;
        }
        if report.completed_sweep {
            break;
        }
    }
    ensure!(saw_pending, "truthful pending readiness");
    let last = get(&f.c, &format!("{mine}?limit=1&offset=100"), Some(4), 200).await?;
    assert_eq!(last["total"], 101);
    assert_eq!(last["data"][0]["attributes"]["name"], "List 101");
    let mut saw_active = false;
    for _ in 0..16 {
        list_aliases::import_page(&f.state)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let active:Option<String>=f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?.query_one("SELECT active_publisher FROM platform.protocol_public_list_alias_cursor WHERE singleton",&[]).await?.get(0);
        if active.as_deref() == Some(public(4).as_str()) {
            saw_active = true;
            break;
        }
    }
    ensure!(saw_active, "rescan fixture must reach first page");
    get(&f.c, &format!("{mine}?limit=1&offset=100"), Some(4), 200).await?;
    let mut records = vec![list(6, "Too many members")];
    for n in 0u64..5001 {
        records.push(Record::Member {
            list: [6; 16],
            series: id(&auth::hash(&n.to_be_bytes())),
            position: n,
            updated_at: 1,
        });
    }
    let oversized = bundle(4, now + 1, records)?;
    save(&f.c, &f.native_origin, 4, &oversized).await?;
    let deferred = list_aliases::import_page(&f.state)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    ensure!(deferred.deferred, "changed original owner must be deferred");
    let next = list_aliases::import_page(&f.state)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    ensure!(
        next.completed_owner && !next.deferred,
        "healthy later owner must progress"
    );
    let after:Option<String>=f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?.query_one("SELECT after_publisher FROM platform.protocol_public_list_alias_cursor WHERE singleton",&[]).await?.get(0);
    assert_eq!(after, Some(public(3)));
    project(f).await?;
    project(f).await?;
    let ready:bool=f.state.db.client().await.map_err(|e|anyhow::anyhow!("{e:?}"))?.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_public_list_alias_projection WHERE publisher=$1 AND head=$2 AND completed)",&[&public(4),&oversized.event().id]).await?.get(0);
    ensure!(ready, "later sweep retries changed owner");
    let alias = mangadex_compat::uuid("list", &format!("{}:{}", public(4), hex::encode([6; 16])));
    get(
        &f.c,
        &format!("{}/api/v1/list/{alias}", f.origin),
        None,
        400,
    )
    .await?;
    if exercise_feed {
        get(
            &f.c,
            &format!("{}/api/v1/list/{alias}/feed", f.origin),
            None,
            400,
        )
        .await?;
    }
    get(&f.c, &mine, Some(4), 400).await?;
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.catalog_manga SET state='draft' WHERE protocol_series_id=$1",
            &[&s2.id],
        )
        .await?;
    assert_eq!(
        get(&f.c, &detail, None, 200).await?["data"]["relationships"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    if exercise_feed {
        assert_eq!(get(&f.c, &feed, None, 200).await?["total"], 1);
    }
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.catalog_manga SET state='published' WHERE protocol_series_id=$1",
            &[&s2.id],
        )
        .await?;
    let mut block = f.repo.pool.get().await?;
    let tx = block.transaction().await?;
    tx.batch_execute("LOCK TABLE platform.catalog_manga IN ACCESS EXCLUSIVE MODE")
        .await?;
    let blocker: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).await?.get(0);
    let req = f.c.get(&detail);
    let pending = tokio::spawn(async move { response(req, 409).await });
    wait_for_blocked_session(&f.repo, blocker).await?;
    let raced = bundle(
        2,
        now + 1,
        vec![
            list(1, "Empty"),
            list(2, "Second"),
            list(5, "Changed current list"),
            member(5, &s1, 9),
            member(5, &s2, 1),
        ],
    )?;
    save(&f.c, &f.native_origin, 2, &raced).await?;
    tx.commit().await?;
    drop(block);
    pending.await??;
    get(
        &f.c,
        &format!("{mine}?limit=1&offset=1&head={}", a.event().id),
        Some(2),
        409,
    )
    .await?;
    get(&f.c, &detail, None, 503).await?;
    project(f).await?;
    assert_eq!(
        get(&f.c, &detail, None, 200).await?["data"]["attributes"]["name"],
        "Changed current list"
    );
    let replacement = bundle(2, now + 2, vec![list(1, "Only remaining")])?;
    save(&f.c, &f.native_origin, 2, &replacement).await?;
    get(&f.c, &detail, None, 404).await?;
    if exercise_feed {
        get(&f.c, &feed, None, 404).await?;
    }
    get(&f.c, &format!("{mine}?head={}", a.event().id), Some(2), 409).await?;
    get(&f.c, &mine, Some(2), 503).await?;
    project(f).await?;
    let current = get(&f.c, &mine, Some(2), 200).await?;
    assert_eq!(current["total"], 1);
    let other_alias = other["data"][0]["id"].as_str().unwrap();
    get(
        &f.c,
        &format!("{}/api/v1/list/{other_alias}", f.origin),
        None,
        200,
    )
    .await?;
    let surviving = current["data"][0]["id"].as_str().unwrap();
    f.repo.set_account_admission(id(&public(2)), false).await?;
    get(&f.c, &mine, Some(2), 403).await?;
    get(
        &f.c,
        &format!("{}/api/v1/list/{surviving}", f.origin),
        None,
        200,
    )
    .await?;
    let deletion = auth::sign(
        &format!("{:064x}", 3),
        5,
        vec![vec!["e".into(), b.event().id.clone()]],
        "fixture public-library deletion".into(),
    )
    .unwrap();
    let permit = f.repo.account_permit(id(&public(3))).await?;
    assert_eq!(f.repo.delete_ordinary(&permit, &deletion).await?, 1);
    get(
        &f.c,
        &format!("{}/api/v1/list/{other_alias}", f.origin),
        None,
        404,
    )
    .await?;
    let deleted = get(&f.c, &mine, Some(3), 200).await?;
    assert_eq!(deleted["total"], 0);
    assert!(deleted["nostr"]["head"].is_null());
    Ok(())
}
