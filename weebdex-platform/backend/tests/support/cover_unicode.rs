//! Actual signed HTTP/PostgreSQL normalization, not a mocked server receipt.
//! Run only in the owned isolated public_manga_writes fixture. Frontend and
//! mounted-controller acceptance are separately covered by the Node tests.
use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned isolated PostgreSQL and verified pinned strfry"]
async fn signed_cover_unicode_metadata_matches_native_trim() {
    let mut f = fixture().await;
    let db = f.state.db.client().await.unwrap();
    db.batch_execute(r#"UPDATE platform.users SET permissions='["manga.cover.edit"]' WHERE id='PublicUploadUser1'"#).await.unwrap();
    let resolver = PostgresPublicIdResolver::new(f.state.db.clone());
    let manga = resolver.preferred(ResourceKind::Manga, FIXTURE_MANGA_ID).await.unwrap().unwrap().to_string();
    let account = resolver.preferred(ResourceKind::User, "PublicUploadUser1").await.unwrap().unwrap().to_string();
    let path = format!("/api/v1/cover/{manga}");
    // Explicit expected outputs catch the NEL/BOM mismatch without using trim
    // to calculate the expected result or replacing production validation.
    for (label, input, expected) in [
        ("NEL", "\u{0085}Text\u{0085}", "Text"),
        ("BOM", "\u{feff}Text\u{feff}", "\u{feff}Text\u{feff}"),
        ("ordinary", " \tText\n ", "Text"),
        ("empty", "\u{0085} \t\n", ""),
        ("BOM-only", "\u{feff}", "\u{feff}"),
        ("internal", "A\u{0085}B\u{feff}C", "A\u{0085}B\u{feff}C"),
    ] {
        let (body, mime) = cover_multipart(1, &account, &[("locale", "ja"), ("volume", input), ("description", input), ("main", "false")], false);
        let created = f.raw("POST", &path, body.clone(), &mime, Some(proof(1, 11, &format!("{}{path}", f.origin), "POST", &body)), 201).await;
        let wire_volume = if expected.is_empty() { Value::Null } else { json!(expected) };
        assert_eq!(created["data"]["attributes"]["volume"], wire_volume, "POST {label}");
        assert_eq!(created["data"]["attributes"]["description"], expected, "POST {label}");
        let cover = created["data"]["id"].as_str().unwrap();
        let native = f.native(ResourceKind::Cover, cover).await;
        let provenance = created["data"]["attributes"]["nostr"].clone();
        assert_eq!(provenance["original"]["sha256"], auth::hash(&png()));
        assert_eq!(provenance["originalEventRewritten"], false);
        let target = format!("/api/v1/cover/{cover}");
        // Imported nullable locale and untouched metadata must not be rewritten
        // when only the description is edited. Native empty language renders as
        // null via nullable_nonempty; SQL seeds only this owned legacy row.
        db.execute("UPDATE platform.catalog_covers SET language='',volume='legacy',description='legacy' WHERE id=$1", &[&native]).await.unwrap();
        let updated = f.call(Some(1), "PUT", &target, Some(json!({"version":1,"description":input})), 200).await;
        assert_eq!(updated["data"]["attributes"]["locale"], Value::Null);
        assert_eq!(updated["data"]["attributes"]["volume"], "legacy");
        assert_eq!(updated["data"]["attributes"]["description"], expected, "PUT description {label}");
        assert_eq!(updated["data"]["attributes"]["nostr"], provenance);
        let updated = f.call(Some(1), "PUT", &target, Some(json!({"version":2,"volume":input})), 200).await;
        assert_eq!(updated["data"]["attributes"]["locale"], Value::Null);
        assert_eq!(updated["data"]["attributes"]["volume"], wire_volume, "PUT volume {label}");
        assert_eq!(updated["data"]["attributes"]["description"], expected);
        assert_eq!(updated["data"]["attributes"]["nostr"], provenance);
        let stored = db.query_one("SELECT volume,description,language,version FROM platform.catalog_covers WHERE id=$1", &[&native]).await.unwrap();
        assert_eq!(stored.get::<_, String>(0), expected);
        assert_eq!(stored.get::<_, Option<String>>(1).as_deref(), Some(expected));
        assert_eq!(stored.get::<_, String>(2), "");
        assert_eq!(stored.get::<_, i32>(3), 3);
        f.call(Some(1), "PUT", &target, Some(json!({"version":2,"description":"stale"})), 409).await;
        let preview = format!("/api/v1/nostr/covers/{cover}/original");
        let response = f.http.get(format!("{}{preview}", f.origin)).header("authorization", proof(1,11,&format!("{}{preview}",f.origin),"GET",&[])).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.bytes().await.unwrap().to_vec(), png());
    }
    assert_eq!(db.query_one("SELECT version FROM platform.catalog_manga WHERE id=$1", &[&FIXTURE_MANGA_ID]).await.unwrap().get::<_, i32>(0), 1);
    let mut native = f.native.take().unwrap();
    if let Some(shutdown) = native.shutdown.take() { let _ = shutdown.send(()); }
    tokio::time::timeout(Duration::from_secs(30), native.task).await.unwrap().unwrap().unwrap();
    println!("SIGNED_COVER_UNICODE_VERIFIED=POST and PUT NEL/BOM/ordinary/empty/internal text, persisted metadata, nullable locale omission, unchanged provenance, exact original bytes and stale-version rejection");
}
