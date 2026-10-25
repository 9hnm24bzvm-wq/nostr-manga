//! Fresh actual-original signed graph / UUID status transport regression.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::Result;
use fixture::*;
use manga_node::{auth, library_manifest::Record};
use serde_json::json;
use std::collections::BTreeMap;
use weebdex_platform::{
    protocol::bridge,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind},
};
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires isolated PG runner and actual strfry"]
async fn genuine_status_reads_preserve_signed_values_owner_visibility_and_no_follow_inference()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture) -> Result<()> {
    let names = [
        "reading",
        "on_hold",
        "plan_to_read",
        "dropped",
        "re_reading",
        "completed",
    ];
    let mut series = vec![];
    for name in names {
        series.push(publish(&f.runtime,1002,cbor(json!({"type":"series","version":1,"titles":{"en":format!("Status {name}")},"contentRating":"safe"})),BTreeMap::new()).await?);
    }
    drain_backend(&f.repo).await?;
    for _ in 0..8 {
        if bridge::import_page(&f.state)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
            .completed_sweep
        {
            break;
        }
    }
    let records = series
        .iter()
        .zip(names)
        .map(|(e, status)| Record::Series {
            id: id(&e.id),
            status: status.into(),
            updated_at: 1,
        })
        .collect::<Vec<_>>();
    let now = auth::now() - 30;
    let graph = bundle(2, now, records)?;
    save(&f.c, &f.native_origin, 2, &graph).await?;
    drain_backend(&f.repo).await?;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let db = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut ids = vec![];
    let mut natives = vec![];
    for e in &series {
        let native: String = db
            .query_one(
                "SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1",
                &[&e.id],
            )
            .await?
            .get(0);
        ids.push(
            resolver
                .preferred(ResourceKind::Manga, &native)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .unwrap(),
        );
        natives.push(native);
    }
    let base = format!("{}/api/v1/manga/status", f.origin);
    get(&f.c, &base, None, 401).await?;
    let all = get(&f.c, &base, Some(2), 200).await?;
    assert_eq!(all["statuses"].as_object().unwrap().len(), 6);
    assert_eq!(all["nostr"]["followFlagInferred"], false);
    let head = all["nostr"]["head"].as_str().unwrap();
    for (uuid, name) in ids.iter().zip(names) {
        assert_eq!(all["statuses"][uuid.as_str()], name);
        let filter = get(
            &f.c,
            &format!("{base}?status={name}&head={head}"),
            Some(2),
            200,
        )
        .await?;
        assert_eq!(filter["statuses"].as_object().unwrap().len(), 1);
        assert_eq!(filter["statuses"][uuid.as_str()], name);
        let one = format!("{}/api/v1/manga/{uuid}/status", f.origin);
        assert_eq!(get(&f.c, &one, Some(2), 200).await?["status"], name);
        let other = get(&f.c, &one, Some(3), 200).await?;
        assert!(other["status"].is_null());
        assert!(other["nostr"]["head"].is_null());
    }
    let before: i64 = db
        .query_one("SELECT count(*) FROM platform.public_resource_ids", &[])
        .await?
        .get(0);
    get(&f.c, &base, Some(2), 200).await?;
    assert_eq!(
        before,
        db.query_one("SELECT count(*) FROM platform.public_resource_ids", &[])
            .await?
            .get::<_, i64>(0)
    );
    for query in [
        "status=other",
        "status=reading&status=completed",
        "head=bad",
        "limit=1",
        "user=someone",
    ] {
        get(&f.c, &format!("{base}?{query}"), Some(2), 400).await?;
    }
    response(
        f.c.get(&base)
            .header("authorization", proof(2, &base))
            .body("undeclared"),
        400,
    )
    .await?;
    get(
        &f.c,
        &format!("{base}?head={}", "f".repeat(64)),
        Some(2),
        409,
    )
    .await?;
    db.execute(
        "UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id=$1",
        &[&natives[0]],
    )
    .await?;
    let hidden = get(&f.c, &base, Some(2), 200).await?;
    assert!(hidden["statuses"].get(ids[0].as_str()).is_none());
    get(
        &f.c,
        &format!("{}/api/v1/manga/{}/status", f.origin, ids[0]),
        Some(2),
        404,
    )
    .await?;
    db.execute(
        "UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id=$1",
        &[&natives[0]],
    )
    .await?;
    f.repo.set_account_admission(id(&public(2)), false).await?;
    get(&f.c, &base, Some(2), 403).await?;
    f.repo.set_account_admission(id(&public(2)), true).await?;
    get(&f.c, &base, Some(2), 200).await?;
    Ok(())
}
