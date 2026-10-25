//! SOURCE-AUTHORED, UNEXECUTED. Actual router + PostgreSQL, never response stubs.
//! Requires a separately admitted fresh disposable database owner.
use axum::{body::{Body, to_bytes}, http::{Request, StatusCode}};
use serde_json::Value;
use tower::ServiceExt;
use weebdex_platform::{AppState, Config, domains, public_api::{PostgresPublicIdResolver, PublicIdResolver, ResourceKind}};
async fn get(app: &axum::Router, path: &str, status: StatusCode) -> Value {
    let response = app.clone().oneshot(Request::builder().uri(path)
        .header("origin", "https://badge.fixture.invalid").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), status, "{path}");
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
fn binding(value: &Value, parent: &str, profile: &str, namespace: &str) {
    let witness = &value["chapterPage"];
    assert_eq!(witness["version"], 1); assert_eq!(witness["mangaId"], parent);
    assert_eq!(witness["idProfile"], profile); assert_eq!(witness["consistency"], "response-snapshot-v1");
    assert_eq!(witness["catalogIdentity"]["catalogNamespace"], namespace);
    assert_eq!(witness["catalogIdentity"]["serviceKey"], Value::Null);
    assert_eq!(witness.as_object().unwrap().len(), 5);
    assert!(witness.get("account").is_none());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires explicitly admitted isolated PostgreSQL owner; not executed source evidence"]
async fn actual_title_page_identity_is_additive_scoped_and_snapshot_bound() {
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use the isolated owner");
        let pg: tokio_postgres::Config = dsn.parse().unwrap();
        assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
        assert_eq!(pg.get_hosts().len(), 1);
        assert!(pg.get_hosts().iter().all(|host| matches!(host, tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1")));
        assert_eq!(pg.get_ports().len(), 1);
        assert!(pg.get_ports().iter().all(|port| *port > 1024 && *port != 5432));
        let runtime = std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR")
            .expect("Owned runtime receipt from scripts/run-auth-tests.sh is required"));
        assert!(runtime.to_string_lossy().contains("/weebdex-platform/test-runtime/auth-"));
        // Consume the actual launcher receipt before any connection, migration
        // or fixture mutation. Endpoint ownership and receipt/DSN association
        // rely on pinned auth_fixture_guard.py and its unchanged environment;
        // the receipt alone proves neither and is not authorization to launch.
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(runtime.join("fixture.json")).expect("Owned disposable fixture receipt"),
        ).unwrap();
        assert_eq!(receipt["database"], "weebdex_platform_test");
        assert_eq!(receipt["fixture_only"], true);
        assert_eq!(receipt["transport"], "private loopback TCP");
        let state = AppState::connect(Config { public_origin: "https://badge.fixture.invalid".into(), database_plaintext_local: true, database_connections: 4, ..Config::default() }, dsn).await.unwrap();
        state.db.migrate(domains::MIGRATIONS).await.unwrap();
        let db = state.db.client().await.unwrap();
        db.batch_execute(r#"
INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES('BadgeUser','Badge fixture','badge@fixture.invalid','synthetic-unusable',TRUE);
INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('BadgeTitle','BadgeUser','Fixture','Badge title','ja','ongoing','safe','published'),('BadgeOther','BadgeUser','Fixture','Other title','ja','ongoing','safe','published');
INSERT INTO platform.catalog_manga_counters(manga_id) VALUES('BadgeTitle'),('BadgeOther');
INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('BadgeGroup','BadgeUser','Fixture','Badge group');
INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,chapter,language,title,metadata) SELECT 'BadgeChapter'||n,'BadgeTitle','BadgeUser','Fixture',n::text,'en','A','{"data":[{"name":"fixture.webp","dimensions":[1,1]}]}'::jsonb FROM generate_series(1,3)n;
INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) SELECT id,'BadgeGroup' FROM platform.catalog_chapters WHERE manga_id='BadgeTitle';
"#).await.unwrap();
        let namespace: String = db.query_one("SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton", &[]).await.unwrap().get(0);
        let resolver = PostgresPublicIdResolver::new(state.db.clone());
        let parent = resolver.preferred(ResourceKind::Manga, "BadgeTitle").await.unwrap().unwrap();
        let group = resolver.preferred(ResourceKind::Group, "BadgeGroup").await.unwrap().unwrap();
        let app = weebdex_platform::router::router(state.clone());
        let native = "/api/manga/BadgeTitle/chapters?limit=2&sort=chapter&order=asc";
        let public = format!("/api/v1/manga/{parent}/feed?limit=2&translatedLanguage[]=en&order[chapter]=asc");
        let grouped = format!("/api/v1/chapter?manga={parent}&groups[]={group}&limit=2&order[chapter]=asc");
        let mut baseline = Vec::new();
        for (path, id, profile) in [(native, "BadgeTitle", "native-id-v1"), (public.as_str(), parent.as_str(), "public-uuid-v1"), (grouped.as_str(), parent.as_str(), "public-uuid-v1")] {
            let value = get(&app, path, StatusCode::OK).await; binding(&value, id, profile, &namespace);
            assert_eq!(value["data"].as_array().unwrap().len(), 2); assert_eq!(value["total"], 3);
            if profile == "public-uuid-v1" { assert!(!value["chapterPage"].to_string().contains("BadgeTitle")); }
            baseline.push(value);
        }
        let empty = get(&app, &format!("/api/v1/manga/{parent}/feed?translatedLanguage[]=fr&limit=1"), StatusCode::OK).await;
        assert_eq!(empty["data"].as_array().unwrap().len(), 0); binding(&empty, parent.as_str(), "public-uuid-v1", &namespace);
        let empty_group = get(&app, &format!("/api/v1/chapter?manga={parent}&translatedLanguage[]=fr&limit=1"), StatusCode::OK).await;
        assert_eq!(empty_group["data"].as_array().unwrap().len(), 0); binding(&empty_group, parent.as_str(), "public-uuid-v1", &namespace);
        assert!(get(&app, "/api/v1/chapter?limit=2", StatusCode::OK).await.get("chapterPage").is_none());
        get(&app, &format!("/api/v1/chapter?manga={parent}&manga={parent}"), StatusCode::BAD_REQUEST).await;
        // Namespace absence must not turn a healthy feed into failure or empty.
        // This mutation is only ever performed in the guarded disposable fixture.
        db.batch_execute("DELETE FROM platform.catalogue_namespace WHERE singleton").await.unwrap();
        for (index, path) in [native, public.as_str(), grouped.as_str()].iter().enumerate() {
            let value = get(&app, path, StatusCode::OK).await; assert!(value.get("chapterPage").is_none());
            assert_eq!(value["data"], baseline[index]["data"]); assert_eq!(value["total"], baseline[index]["total"]);
        }
        db.execute("INSERT INTO platform.catalogue_namespace(namespace) VALUES($1::text::uuid)", &[&namespace]).await.unwrap();
        // Deterministic A→B→A response sequence, separate from the scheduling-
        // dependent stress below. Each real route must observe both changes.
        {
            let mut writer = state.db.client().await.unwrap();
            for (ns, label) in [("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", "B"), (namespace.as_str(), "A")] {
                let tx = writer.transaction().await.unwrap();
                tx.execute("UPDATE platform.catalogue_namespace SET namespace=$1::text::uuid WHERE singleton", &[&ns]).await.unwrap();
                tx.execute("UPDATE platform.catalog_chapters SET title=$1 WHERE manga_id='BadgeTitle'", &[&label]).await.unwrap();
                tx.commit().await.unwrap();
                for (path, id, profile) in [(native, "BadgeTitle", "native-id-v1"), (public.as_str(), parent.as_str(), "public-uuid-v1"), (grouped.as_str(), parent.as_str(), "public-uuid-v1")] {
                    let value = get(&app, path, StatusCode::OK).await; binding(&value, id, profile, ns);
                    for row in value["data"].as_array().unwrap() { assert_eq!(if path == native { &row["title"] } else { &row["attributes"]["title"] }, label); }
                }
            }
        }
        // Concurrent finite churn changes namespace and row labels atomically.
        // Every response must bind both to its own statement/RR snapshot, not a
        // capabilities lookup before or after the request. This stress phase is
        // scheduling-dependent, not deterministic proof of a concurrent overlap.
        let state2 = state.clone(); let original = namespace.clone();
        let changing = async move {
            let mut writer = state2.db.client().await.unwrap();
            for index in 0..8 {
                let tx = writer.transaction().await.unwrap();
                let (ns, label) = if index % 2 == 0 { ("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", "B") } else { (original.as_str(), "A") };
                tx.execute("UPDATE platform.catalogue_namespace SET namespace=$1::text::uuid WHERE singleton", &[&ns]).await.unwrap();
                tx.execute("UPDATE platform.catalog_chapters SET title=$1 WHERE manga_id='BadgeTitle'", &[&label]).await.unwrap();
                tx.commit().await.unwrap(); tokio::task::yield_now().await;
            }
        };
        let reading = async {
            for _ in 0..8 { for path in [native, public.as_str(), grouped.as_str()] {
                let value = get(&app, path, StatusCode::OK).await;
                let expected = if value["chapterPage"]["catalogIdentity"]["catalogNamespace"] == namespace { "A" } else { assert_eq!(value["chapterPage"]["catalogIdentity"]["catalogNamespace"], "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"); "B" };
                for row in value["data"].as_array().unwrap() { assert_eq!(if path == native { &row["title"] } else { &row["attributes"]["title"] }, expected); }
            } }
        };
        tokio::join!(changing, reading);
    }).await.expect("Finite badge-binding control exceeded its admitted deadline");
}
