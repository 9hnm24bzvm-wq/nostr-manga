//! RECONSTRUCTED UNVERIFIED real metadata/profiles fixture after reset.
#[path = "support/nostr_library.rs"]
mod fixture;
use anyhow::{Result, ensure};
use fixture::*;
use manga_node::{auth, library_manifest::Record};
use serde_json::{Value, json};
use weebdex_platform::{
    protocol::list_aliases,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind},
};
async fn metadata(f: &Fixture, key: u8, created: u64, metadata: Value) -> Result<auth::Event> {
    let event = sign_at(key, 0, vec![], metadata.to_string(), created);
    let body = serde_json::to_vec(&event)?;
    let url = format!("{}/compat/mangadex/nostr/account-metadata", f.native_origin);
    let r =
        f.c.post(&url)
            .header("authorization", http(key, &url, &body))
            .body(body)
            .send()
            .await?;
    ensure!(
        r.status().is_success(),
        "genuine signed profile: {}",
        r.text().await?
    );
    Ok(event)
}
async fn alias_count(f: &Fixture) -> Result<(i64, i64)> {
    let r=f.repo.pool.get().await?.query_one("SELECT (SELECT count(*) FROM platform.public_resource_ids),(SELECT count(*) FROM compatibility_ids)",&[]).await?;
    Ok((r.get(0), r.get(1)))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned PostgreSQL runner and actual strfry"]
async fn real_public_profiles_use_current_verified_metadata_without_authority_or_private_native_fields()
-> Result<()> {
    let f = Fixture::start().await?;
    let result = exercise(&f).await;
    f.shutdown().await?;
    result
}
async fn exercise(f: &Fixture) -> Result<()> {
    let now = auth::now() - 30;
    for key in [2, 3] {
        let graph = bundle(
            key,
            now,
            vec![Record::List {
                id: [key; 16],
                name: format!("Owner {key}"),
                updated_at: 1,
            }],
        )?;
        save(&f.c, &f.native_origin, key, &graph).await?;
    }
    for _ in 0..16 {
        if list_aliases::import_page(&f.state)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
            .completed_sweep
        {
            break;
        }
    }
    let profile=metadata(f,3,now,json!({"display_name":"Signed public display","name":"lower priority","about":"Intentionally public Nostr metadata"})).await?;
    drain_backend(&f.repo).await?;
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let a = resolver
        .preferred(ResourceKind::User, &format!("protocol:{}", public(2)))
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let b = resolver
        .preferred(ResourceKind::User, &format!("protocol:{}", public(3)))
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .unwrap();
    let a_url = format!("{}/api/v1/user/{a}", f.origin);
    let b_url = format!("{}/api/v1/user/{b}", f.origin);
    let original =
        f.c.get(format!(
            "{}/compat/mangadex/user/{}",
            f.native_origin,
            public(3)
        ))
        .send()
        .await?;
    ensure!(original.status() == 200, "original profile fixture");
    let original: Value = original.json().await?;
    assert_eq!(
        original["data"]["attributes"]["username"],
        "Signed public display"
    );
    let before = alias_count(f).await?;
    let no = get(&f.c, &a_url, None, 200).await?;
    assert_eq!(
        no["data"]["attributes"],
        json!({"username":public(2),"roles":[]})
    );
    assert_eq!(no["data"]["nostr"]["nameIsPublicKeyFallback"], true);
    assert!(no["data"]["nostr"]["profile"].is_null());
    assert_eq!(no["data"]["nostr"]["metadata"], json!({}));
    let named = get(&f.c, &b_url, None, 200).await?;
    assert_eq!(named["data"]["id"], b.as_str());
    assert_eq!(
        named["data"]["attributes"],
        json!({"username":"Signed public display","roles":[]})
    );
    assert_eq!(named["data"]["nostr"]["nameIsPublicKeyFallback"], false);
    let signed: auth::Event = serde_json::from_value(named["data"]["nostr"]["profile"].clone())?;
    signed.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(signed.id, profile.id);
    assert_eq!(signed.pubkey, public(3));
    get(&f.c, &format!("{b_url}?includes[]=user"), None, 400).await?;
    get(
        &f.c,
        &format!("{}/api/v1/user/not-uuid", f.origin),
        None,
        400,
    )
    .await?;
    get(
        &f.c,
        &format!(
            "{}/api/v1/user/11111111-1111-4111-8111-111111111111",
            f.origin
        ),
        None,
        404,
    )
    .await?;
    let list_alias = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .query_one(
            "SELECT public_id::text FROM platform.public_resource_ids WHERE kind='list' LIMIT 1",
            &[],
        )
        .await?
        .get::<_, String>(0);
    get(
        &f.c,
        &format!("{}/api/v1/user/{list_alias}", f.origin),
        None,
        404,
    )
    .await?;
    assert_eq!(
        alias_count(f).await?,
        before,
        "public GET must not register aliases"
    );
    let unknown: String = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .query_one(
            "SELECT platform.public_register_protocol_publisher($1)::text",
            &[&public(4)],
        )
        .await?
        .get(0);
    get(
        &f.c,
        &format!("{}/api/v1/user/{unknown}", f.origin),
        None,
        404,
    )
    .await?;
    f.repo.set_account_admission(id(&public(3)), false).await?;
    assert_eq!(
        get(&f.c, &b_url, None, 200).await?["data"]["attributes"]["username"],
        "Signed public display"
    );
    f.repo.set_account_admission(id(&public(3)), true).await?;
    let replaced = metadata(
        f,
        3,
        now + 1,
        json!({"display_name":null,"name":["not a name"]}),
    )
    .await?;
    let fallback = get(&f.c, &b_url, None, 200).await?;
    assert_eq!(fallback["data"]["attributes"]["username"], public(3));
    assert_eq!(fallback["data"]["nostr"]["nameIsPublicKeyFallback"], true);
    assert!(!fallback.to_string().contains("Signed public display"));
    let deletion = auth::sign(
        &format!("{:064x}", 3),
        5,
        vec![vec!["e".into(), replaced.id]],
        "fixture profile deletion".into(),
    )
    .unwrap();
    let permit = f.repo.account_permit(id(&public(3))).await?;
    assert_eq!(f.repo.delete_ordinary(&permit, &deletion).await?, 1);
    let deleted = get(&f.c, &b_url, None, 200).await?;
    assert_eq!(deleted["data"]["nostr"]["metadata"], json!({}));
    assert!(deleted["data"]["nostr"]["profile"].is_null());
    assert_eq!(deleted["data"]["attributes"]["username"], public(3));
    let native = "44444444-4444-4444-8444-444444444444";
    let db = f
        .state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    db.execute("INSERT INTO platform.users(id,username,email,password_hash,roles,profile,permissions) VALUES($1,'PublicNative','private-email@fixture.invalid','synthetic-unused','[\"user\",\"moderator\"]','{\"privateSentinel\":\"not-for-this-projection\"}','[\"private.permission\"]')",&[&native]).await?;
    drop(db);
    let url = format!("{}/api/v1/user/{native}", f.origin);
    let value = get(&f.c, &url, None, 200).await?;
    assert_eq!(
        value["data"]["attributes"],
        json!({"username":"PublicNative","roles":["user","moderator"],"version":1})
    );
    assert_eq!(
        value["data"]["nostr"],
        json!({"identity":"native_public_user"})
    );
    for private in [
        "private-email",
        "synthetic-unused",
        "privateSentinel",
        "private.permission",
    ] {
        ensure!(!value.to_string().contains(private));
    }
    f.state
        .db
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .execute(
            "UPDATE platform.users SET disabled=TRUE WHERE id=$1",
            &[&native],
        )
        .await?;
    get(&f.c, &url, None, 404).await?;
    Ok(())
}
