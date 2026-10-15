//! Fresh actual-original runtime regression for cached withdrawal propagation.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::{Result, ensure};
use fixture::*;
use manga_node::{auth, mangadex_compat};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use weebdex_platform::{
    protocol::bridge,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind},
};
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires isolated PG runner and verified strfry"]
async fn actual_original_group_forum_and_media_honor_cached_parent_withdrawal() -> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture) -> Result<()> {
    let image = png();
    let series=publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":"Cached boundary"},"contentRating":"safe","cover":page(&image)})),BTreeMap::from([(auth::hash(&image),image.clone())])).await?;
    let chapter=publish(&f.runtime,1001,cbor(json!({"type":"chapter","version":1,"series":series.id,"language":"en","chapter":"1","pages":[page(&image)]})),BTreeMap::from([(auth::hash(&image),image.clone())])).await?;
    drain_backend(&f.repo).await?;
    for _ in 0..8 {
        let result = bridge::import_page(&f.state)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        if result.completed_sweep {
            break;
        }
    }
    let db = f.state.db.client().await?;
    let mid: String = db
        .query_one(
            "SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1",
            &[&series.id],
        )
        .await?
        .get(0);
    let cid: String = db
        .query_one(
            "SELECT id FROM platform.catalog_chapters WHERE protocol_event_id=$1",
            &[&chapter.id],
        )
        .await?
        .get(0);
    let cover: String = db
        .query_one(
            "SELECT id FROM platform.catalog_covers WHERE protocol_event_id=$1",
            &[&series.id],
        )
        .await?
        .get(0);
    db.batch_execute("INSERT INTO platform.users(id,username,email,password_hash) VALUES('VisibilityUser','visibility-fixture','visibility@fixture.invalid','synthetic-no-password'); INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('VisibleG01','VisibilityUser','Fixture','Projected source group'),('WrongGrp01','VisibilityUser','Fixture','Unrelated projected group'),('NativeGp01','VisibilityUser','Fixture','Ordinary native group'); INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject) VALUES('VisibilityGroupThread','group','VisibleG01','VisibilityUser','Fixture','Actual discussion')").await?;
    for (group, target) in [
        ("VisibleG01", series.pubkey.clone()),
        ("WrongGrp01", public(3)),
    ] {
        let alias = mangadex_compat::uuid("group", &target);
        f.repo
            .register_compatibility_alias("group", &alias, &target, true)
            .await?;
        db.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('group',$1,$2,$3)",&[&target,&alias,&group]).await?;
    }
    db.execute(
        "INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES($1,'VisibleG01')",
        &[&cid],
    )
    .await?;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let gid = resolver
        .preferred(ResourceKind::Group, "VisibleG01")
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let wrong = resolver
        .preferred(ResourceKind::Group, "WrongGrp01")
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let manga = resolver
        .preferred(ResourceKind::Manga, &mid)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let chapter_uuid = resolver
        .preferred(ResourceKind::Chapter, &cid)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    get(&f.c, &format!("{}/api/v1/group/{gid}", f.origin), None, 200).await?;
    get(
        &f.c,
        &format!("{}/api/v1/group/{wrong}", f.origin),
        None,
        404,
    )
    .await?;
    let original_path = format!("{}/data/{chapter_uuid}/{}", f.origin, auth::hash(&image));
    let cover_path = format!("{}/covers/{manga}/{cover}.png", f.origin);
    for url in [&original_path, &cover_path] {
        let response = f.c.get(url).send().await?;
        ensure!(response.status() == 200, "original media baseline");
        assert_eq!(response.bytes().await?.as_ref(), image.as_slice());
    }
    for visible in [false, true] {
        db.execute(
            "UPDATE platform.catalog_manga SET protocol_source_visible=$2 WHERE id=$1",
            &[&mid, &visible],
        )
        .await?;
        ensure!(
            f.state
                .protocol
                .entry(id(&series.id))
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .is_some(),
            "original live source unchanged"
        );
        let row=db.query_one("SELECT platform.protocol_group_is_visible('VisibleG01'),platform.protocol_group_is_visible('WrongGrp01'),platform.protocol_group_is_visible('NativeGp01'),platform.community_thread_visible('VisibilityGroupThread')",&[]).await?;
        assert_eq!(
            (
                row.get::<_, bool>(0),
                row.get::<_, bool>(1),
                row.get::<_, bool>(2),
                row.get::<_, bool>(3)
            ),
            (visible, false, true, visible)
        );
        for path in [
            format!("/api/v1/group/{gid}"),
            "/api/group/VisibleG01".into(),
            "/api/thread/VisibilityGroupThread".into(),
            format!("/api/v1/manga/{manga}"),
            format!("/api/v1/chapter/{chapter_uuid}"),
        ] {
            let response = f.c.get(format!("{}{path}", f.origin)).send().await?;
            assert_eq!(
                response.status().as_u16(),
                if visible { 200 } else { 404 },
                "{path}"
            );
        }
        for url in [&original_path, &cover_path] {
            let response = f.c.get(url).send().await?;
            assert_eq!(
                response.status().as_u16(),
                if visible { 200 } else { 404 },
                "{url}"
            );
            if visible {
                assert_eq!(response.bytes().await?.as_ref(), image.as_slice());
            }
        }
        let context: Value =
            f.c.post(format!("{}/api/v1/nostr/native-forum/contexts", f.origin))
                .json(&json!({"resources":[{"kind":"group","id":gid}]}))
                .send()
                .await?
                .json()
                .await?;
        assert_eq!(
            context[if visible { "data" } else { "unavailable" }]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        if visible {
            assert_eq!(context["data"][0]["thread"]["id"], "VisibilityGroupThread");
        }
    }
    Ok(())
}
