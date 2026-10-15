//! RECONSTRUCTED-UNVERIFIED after environment replacement on 2026-10-15.
//! Six public statistics routes through a real loopback HTTP listener and an
//! owned disposable PostgreSQL cluster. No production data or credentials.
use axum::http::StatusCode;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use weebdex_platform::{
    AppState, Config, domains,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind},
};

const MANGA: &str = "StatsManga";
const CHAPTER: &str = "StatsChapt";
const GROUP: &str = "StatsGroup";
const UNKNOWN: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
const LEGACY: &str = "12345678-1234-4234-8234-123456789abc";

async fn fixture() -> AppState {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
        .expect("Run only with scripts/run-auth-tests.sh");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    for host in pg.get_hosts() {
        assert!(matches!(host, tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1"));
    }
    assert!(pg.get_ports().iter().all(|p| *p > 1024 && *p != 5432));
    let runtime = std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap();
    assert!(runtime.contains("/weebdex-platform/test-runtime/"));
    assert!(std::path::Path::new(&runtime).join("fixture.json").is_file());
    let state = AppState::connect(
        Config {
            database_plaintext_local: true,
            database_connections: 4,
            ..Config::default()
        },
        dsn,
    ).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    state
}
async fn public_ids(state: &AppState) -> BTreeMap<&'static str, PublicUuid> {
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let mut ids = BTreeMap::new();
    for (name, kind, native) in [
        ("manga", ResourceKind::Manga, MANGA),
        ("chapter", ResourceKind::Chapter, CHAPTER),
        ("group", ResourceKind::Group, GROUP),
    ] {
        ids.insert(name, resolver.preferred(kind, native).await.unwrap().unwrap());
    }
    resolver.register_alias(ResourceKind::Manga, MANGA, &PublicUuid::parse(LEGACY).unwrap()).await.unwrap();
    ids
}
async fn get(client: &reqwest::Client, base: &str, path: &str, expected: StatusCode) -> Value {
    let response = client.get(format!("{base}/api/v1{path}"))
        .header("accept", "text/html")
        .send().await.unwrap();
    assert_eq!(response.status(), expected, "{path}");
    assert!(response.headers()["content-type"].to_str().unwrap().starts_with("application/json"));
    assert!(response.headers()["cache-control"].to_str().unwrap().contains("no-store"));
    let value: Value = response.json().await.unwrap();
    if !expected.is_success() {
        assert_eq!(value["result"], "error", "{path}: {value}");
        assert_eq!(value["errors"][0]["status"], expected.as_u16());
        assert!(PublicUuid::parse(value["errors"][0]["id"].as_str().unwrap()).is_ok());
        let text = value.to_string();
        assert!(!text.contains("platform.") && !text.contains("SELECT ") && !text.contains("password"));
    }
    value
}

#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner"]
async fn statistics_real_http_counts_batches_aliases_visibility_and_errors() {
    let state = fixture().await;
    let db = state.db.client().await.unwrap();
    db.batch_execute(r#"
        INSERT INTO platform.users(id,username,email,password_hash) VALUES
            ('StatsUser1','statistics-one','statistics1@example.invalid','synthetic-no-password'),
            ('StatsUser2','statistics-two','statistics2@example.invalid','synthetic-no-password');
        INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state)
            VALUES('StatsManga','StatsUser1','Fixture','Statistics fixture','ja','ongoing','safe','published');
        INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name)
            VALUES('StatsGroup','StatsUser1','Fixture','Statistics fixture group');
        INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language,is_unavailable,deleted_at,protocol_event_id) VALUES
            ('StatsChapt','StatsManga','StatsUser1','Fixture','en',false,NULL,NULL),
            ('StatsUnav1','StatsManga','StatsUser1','Fixture','en',true,NULL,NULL),
            ('StatsUnav2','StatsManga','StatsUser1','Fixture','fr',true,NULL,NULL),
            ('StatsDelet','StatsManga','StatsUser1','Fixture','en',true,clock_timestamp(),NULL),
            ('StatsSrcHd','StatsManga','StatsUser1','Fixture','en',true,NULL,repeat('a',64));
        INSERT INTO platform.library_manga_follows(user_id,manga_id,reading_status,is_private) VALUES
            ('StatsUser1','StatsManga','reading',true),('StatsUser2','StatsManga','completed',false);
        -- A legacy cached number is not enough evidence for a real rating or
        -- distribution; follows are counted from authoritative persisted rows.
        INSERT INTO platform.catalog_manga_counters(manga_id,follows,rating) VALUES('StatsManga',999,8.75);
        INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject)
            VALUES('text-not-a-forum-number','manga','StatsManga','StatsUser1','Fixture','Local discussion');
        INSERT INTO platform.community_replies(thread_id,reply_number,author_id,body)
            VALUES('text-not-a-forum-number',1,'StatsUser2','Actual local reply');
    "#).await.unwrap();
    let ids = public_ids(&state).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = weebdex_platform::router::router(state.clone());
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).with_graceful_shutdown(async { let _ = stopped.await; }).await.unwrap();
    });
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().unwrap();

    // All six exact upstream paths, UUID keys, optional unsupported data absent.
    for kind in ["manga", "chapter", "group"] {
        let id = ids[kind].as_str();
        let one = get(&client, &base, &format!("/statistics/{kind}/{id}"), StatusCode::OK).await;
        let batch = get(&client, &base, &format!("/statistics/{kind}?{kind}%5B%5D={id}"), StatusCode::OK).await;
        assert_eq!(one, batch);
        let values = one["statistics"].as_object().unwrap();
        assert_eq!(values.len(), 1);
        for key in values.keys() { PublicUuid::parse(key).unwrap(); }
        assert_eq!(values[id]["comments"], Value::Null);
        assert!(values[id].get("rating").is_none());
        if kind == "manga" {
            assert_eq!(values[id]["follows"], 2);
            assert_eq!(values[id]["unavailableChapterCount"], 2);
        } else { assert_eq!(values[id], json!({"comments":null})); }
        let empty = get(&client, &base, &format!("/statistics/{kind}?{kind}[]={UNKNOWN}"), StatusCode::OK).await;
        assert_eq!(empty, json!({"result":"ok","statistics":{}}));
        get(&client, &base, &format!("/statistics/{kind}/{UNKNOWN}"), StatusCode::NOT_FOUND).await;
    }
    let manga_id = ids["manga"].as_str();
    let legacy = get(&client, &base, &format!("/statistics/manga/{LEGACY}"), StatusCode::OK).await;
    assert!(legacy["statistics"].get(manga_id).is_some());
    assert!(legacy["statistics"].get(LEGACY).is_none());
    let mixed = get(&client, &base, &format!("/statistics/manga?manga[]={LEGACY}&manga[]={manga_id}&manga[]={UNKNOWN}&manga[]={}", ids["chapter"]), StatusCode::OK).await;
    assert_eq!(mixed, legacy);
    get(&client, &base, &format!("/statistics/chapter/{manga_id}"), StatusCode::NOT_FOUND).await;
    get(&client, &base, &format!("/statistics/group/{manga_id}"), StatusCode::NOT_FOUND).await;

    for path in [
        "/statistics/manga".to_owned(), "/statistics/chapter".into(), "/statistics/group".into(),
        "/statistics/manga/not-a-uuid".into(), "/statistics/manga/%FF".into(),
        "/statistics/chapter/not-a-uuid".into(), "/statistics/group/not-a-uuid".into(),
        format!("/statistics/manga/{manga_id}?limit=0"),
        format!("/statistics/manga?manga={manga_id}"),
        format!("/statistics/chapter?manga[]={manga_id}"),
        format!("/statistics/manga?manga[]={manga_id}&offset=0"),
        "/statistics/manga?manga[]=bad".into(),
        "/statistics/group?group[]=%FF".into(),
        "/statistics/chapter?chapter[]=%".into(),
        format!("/statistics/manga?{}", std::iter::repeat_n(format!("manga[]={manga_id}"),101).collect::<Vec<_>>().join("&")),
    ] { get(&client, &base, &path, StatusCode::BAD_REQUEST).await; }
    let wrong_method = client.post(format!("{base}/api/v1/statistics/manga/{manga_id}"))
        .send().await.unwrap();
    assert_eq!(wrong_method.status(), StatusCode::METHOD_NOT_ALLOWED);
    let wrong_method: Value = wrong_method.json().await.unwrap();
    assert_eq!(wrong_method["errors"][0]["status"], 405);

    // Full100-resource batch produces100 keys, not100 per-row repository reads.
    db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) SELECT 'StatsBatch'||n,'StatsUser1','Fixture','Batch '||n,'ja','ongoing','safe','published' FROM generate_series(1,100) n", &[]).await.unwrap();
    let natives: Vec<String> = (1..=100).map(|n| format!("StatsBatch{n}")).collect();
    let batch_ids = PostgresPublicIdResolver::new(state.db.clone()).preferred_many(ResourceKind::Manga, &natives).await.unwrap();
    let query = batch_ids.values().map(|id| format!("manga[]={id}")).collect::<Vec<_>>().join("&");
    let full = get(&client, &base, &format!("/statistics/manga?{query}"), StatusCode::OK).await;
    assert_eq!(full["statistics"].as_object().unwrap().len(), 100);
    assert!(full["statistics"].as_object().unwrap().values().all(|v| v["follows"] == 0 && v["unavailableChapterCount"] == 0));

    // Live counts, not response caches or alias creation on reads.
    db.execute("DELETE FROM platform.library_manga_follows WHERE user_id='StatsUser1' AND manga_id='StatsManga'", &[]).await.unwrap();
    let updated = get(&client, &base, &format!("/statistics/manga/{manga_id}"), StatusCode::OK).await;
    assert_eq!(updated["statistics"][manga_id]["follows"], 1);
    let before: i64 = db.query_one("SELECT count(*) FROM platform.public_resource_ids", &[]).await.unwrap().get(0);
    get(&client, &base, &format!("/statistics/manga/{UNKNOWN}"), StatusCode::NOT_FOUND).await;
    let after: i64 = db.query_one("SELECT count(*) FROM platform.public_resource_ids", &[]).await.unwrap().get(0);
    assert_eq!(before, after);

    // Parent withdrawal, drafts, tombstones, and live source authority all hide
    // statistics even though their immutable aliases still resolve.
    for change in ["state='draft'", "deleted_at=clock_timestamp()", "protocol_source_visible=false", "protocol_series_id=repeat('b',64)"] {
        db.execute(&format!("UPDATE platform.catalog_manga SET {change} WHERE id='StatsManga'"), &[]).await.unwrap();
        for kind in ["manga", "chapter"] {
            let id = ids[kind].as_str();
            get(&client, &base, &format!("/statistics/{kind}/{id}"), StatusCode::NOT_FOUND).await;
            let batch = get(&client, &base, &format!("/statistics/{kind}?{kind}[]={id}"), StatusCode::OK).await;
            assert_eq!(batch["statistics"], json!({}));
        }
        db.execute("UPDATE platform.catalog_manga SET state='published',deleted_at=NULL,protocol_source_visible=true,protocol_series_id=NULL WHERE id='StatsManga'", &[]).await.unwrap();
    }
    db.execute("UPDATE platform.catalog_chapters SET protocol_event_id=repeat('c',64) WHERE id='StatsChapt'", &[]).await.unwrap();
    get(&client, &base, &format!("/statistics/chapter/{}", ids["chapter"]), StatusCode::NOT_FOUND).await;
    db.execute("UPDATE platform.catalog_chapters SET protocol_event_id=NULL,deleted_at=clock_timestamp() WHERE id='StatsChapt'", &[]).await.unwrap();
    get(&client, &base, &format!("/statistics/chapter/{}", ids["chapter"]), StatusCode::NOT_FOUND).await;
    db.execute("UPDATE platform.catalog_groups SET deleted_at=clock_timestamp() WHERE id='StatsGroup'", &[]).await.unwrap();
    get(&client, &base, &format!("/statistics/group/{}", ids["group"]), StatusCode::NOT_FOUND).await;
    db.execute("UPDATE platform.catalog_groups SET deleted_at=NULL WHERE id='StatsGroup'", &[]).await.unwrap();
    db.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES('group','statistics-withdrawn-source','11111111-2222-4333-8444-555555555555','StatsGroup')", &[]).await.unwrap();
    get(&client, &base, &format!("/statistics/group/{}", ids["group"]), StatusCode::NOT_FOUND).await;

    // Storage failure must be a safe upstream error, never a successful empty
    // map, leaked SQL, or an HTML SPA. Pool close is scoped to this owned state.
    drop(db);
    state.db.pool().close();
    get(&client, &base, &format!("/statistics/manga/{manga_id}"), StatusCode::SERVICE_UNAVAILABLE).await;
    let _ = shutdown.send(());
    server.await.unwrap();
}
