//! Fresh disposable PostgreSQL only. Reconstructed v2, no old-byte parity claim.
use weebdex_platform::{
    AppState, Config, domains,
    public_api::{
        PostgresPublicIdResolver, PublicIdResolver, ResourceKind, edge_identity::manga_edge_key,
    },
};
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL runner"]
async fn composite_edge_aliases_keep_direction_kind_namespace_and_readded_identity() {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use run-auth-tests.sh");
    let cfg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(cfg.get_dbname(), Some("weebdex_platform_test"));
    for host in cfg.get_hosts() {
        assert!(matches!(host,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1"));
    }
    let state = AppState::connect(
        Config {
            database_plaintext_local: true,
            ..Config::default()
        },
        dsn,
    )
    .await
    .unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    let c = state.db.client().await.unwrap();
    c.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating) VALUES('Edge000001','fixture','Fixture','One','en','ongoing','safe'),('Edge000002','fixture','Fixture','Two','en','ongoing','safe'); INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES('Edge000001','Edge000002','sequel','fixture'),('Edge000002','Edge000001','prequel','fixture'); INSERT INTO platform.catalog_recommendations(manga_id,recommended_id) VALUES('Edge000001','Edge000002');").await.unwrap();
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let key = manga_edge_key("Edge000001", "Edge000002");
    let forward = resolver
        .preferred(ResourceKind::MangaRelation, &key)
        .await
        .unwrap()
        .unwrap();
    let backward = resolver
        .preferred(
            ResourceKind::MangaRelation,
            &manga_edge_key("Edge000002", "Edge000001"),
        )
        .await
        .unwrap()
        .unwrap();
    let recommendation = resolver
        .preferred(ResourceKind::MangaRecommendation, &key)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(forward, backward);
    assert_ne!(forward, recommendation);
    let row=c.query_one("SELECT platform.public_manga_edge_key('Edge000001','Edge000002'),platform.public_native_uuid('00000000-0000-4000-8000-000000000001','manga_relation',$1)::TEXT,platform.public_native_uuid('00000000-0000-4000-8000-000000000002','manga_relation',$1)::TEXT,platform.public_manga_edge_key('a','bc')<>platform.public_manga_edge_key('ab','c')",&[&key]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), key);
    assert_ne!(row.get::<_, String>(1), row.get::<_, String>(2));
    assert!(row.get::<_, bool>(3));
    c.batch_execute("DELETE FROM platform.catalog_relations WHERE manga_id='Edge000001' AND related_id='Edge000002'; INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES('Edge000001','Edge000002','side_story','fixture');").await.unwrap();
    assert_eq!(
        resolver
            .preferred(ResourceKind::MangaRelation, &key)
            .await
            .unwrap(),
        Some(forward)
    );
    c.query_one("SELECT platform.public_backfill_resource_ids()", &[])
        .await
        .unwrap();
    assert_eq!(
        resolver
            .preferred(ResourceKind::MangaRecommendation, &key)
            .await
            .unwrap(),
        Some(recommendation)
    );
}
