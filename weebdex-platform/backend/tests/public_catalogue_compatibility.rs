//! Real HTTP + disposable PostgreSQL compatibility seams. No production data.
use serde_json::{Value, json};
use weebdex_platform::{
    AppState, Config, domains,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind, edge_identity::manga_edge_key},
};

async fn get(client: &reqwest::Client, base: &str, path: &str, status: u16) -> Value {
    let response = client.get(format!("{base}/api/v1{path}")).send().await.unwrap();
    let actual = response.status().as_u16();
    let value: Value = response.json().await.unwrap();
    assert_eq!(actual, status, "{path}: {value}");
    value
}

fn assert_public_ids(value: &Value) {
    match value {
        Value::Object(fields) => {
            if let Some(id) = fields.get("id").and_then(Value::as_str) {
                assert!(PublicUuid::parse(id).is_ok(), "non-public id: {id}");
            }
            for child in fields.values() { assert_public_ids(child); }
        }
        Value::Array(values) => for child in values { assert_public_ids(child); },
        _ => {}
    }
}

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server { fn drop(&mut self) { self.0.abort(); } }

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires explicitly isolated PostgreSQL runner"]
async fn read_compatibility_edges_uuid_expansion_paging_and_revocation() {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(pg.get_hosts().iter().all(|h| matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1")));
    let runtime = std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
    assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let state = AppState::connect(Config {
        database_plaintext_local: true,
        database_connections: 4,
        public_origin: base.clone(),
        uploads_dir: runtime.join("media"),
        ui_dir: runtime.join("no-ui"),
        ..Config::default()
    }, dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    let db = state.db.client().await.unwrap();
    db.batch_execute(r#"
        INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES
        ('CompatUser','compat-user','compat@fixture.invalid','not-a-password',true),
        ('CompatOff1','compat-disabled','disabled@fixture.invalid','not-a-password',true);
        UPDATE platform.users SET disabled=true WHERE id='CompatOff1';
        INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES
        ('CompatGrp1','CompatUser','Fixture','Other'),
        ('CompatGrp2','CompatUser','Fixture','Exact'),
        ('CompatGrp0','CompatUser','Fixture','Exact story');
        INSERT INTO platform.library_group_follows(user_id,group_id) VALUES
        ('CompatUser','CompatGrp1'),('CompatOff1','CompatGrp2');
        INSERT INTO platform.catalog_tags(id,name,tag_group) VALUES('CompatTag1','Compatibility','theme');
        INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES
        ('CompatRoot','CompatUser','Fixture','Root','en','ongoing','safe','published'),
        ('CompatSafe','CompatUser','Fixture','Safe target','ja','ongoing','safe','published'),
        ('CompatPorn','CompatUser','Fixture','Adult target','en','completed','pornographic','published'),
        ('CompatHide','CompatUser','Fixture','Hidden target','en','ongoing','safe','published'),
        ('CompatDrft','CompatUser','Fixture','Draft target','en','ongoing','safe','draft');
        UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='CompatHide';
        INSERT INTO platform.catalog_manga_tags(manga_id,tag_id) VALUES('CompatRoot','CompatTag1');
        INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by) VALUES
        ('CompatRoot','CompatSafe','sequel','CompatUser'),
        ('CompatRoot','CompatPorn','side_story','CompatUser'),
        ('CompatRoot','CompatHide','prequel','CompatUser'),
        ('CompatRoot','CompatDrft','adapted_from','CompatUser');
        INSERT INTO platform.catalog_recommendations(manga_id,recommended_id,score) VALUES
        ('CompatRoot','CompatSafe',12),('CompatRoot','CompatPorn',99),
        ('CompatRoot','CompatHide',100),('CompatRoot','CompatDrft',101);
        INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,volume,chapter,language,is_unavailable,metadata) VALUES
        ('CompatCh01','CompatRoot','CompatUser','Fixture','Readable','1','1','en',false,'{"data":[{"name":"fixture.webp","dimensions":[1,1]}]}'),
        ('CompatCh02','CompatRoot','CompatUser','Fixture','Unavailable duplicate','1','1','en',true,'{"data":[]}'),
        ('CompatCh03','CompatRoot','CompatUser','Fixture','Unavailable only','2','2','en',true,'{"data":[]}');
    "#).await.unwrap();
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let root = resolver.preferred(ResourceKind::Manga, "CompatRoot").await.unwrap().unwrap();
    let safe = resolver.preferred(ResourceKind::Manga, "CompatSafe").await.unwrap().unwrap();
    let porn = resolver.preferred(ResourceKind::Manga, "CompatPorn").await.unwrap().unwrap();
    let group = resolver.preferred(ResourceKind::Group, "CompatGrp1").await.unwrap().unwrap();
    let exact_group = resolver.preferred(ResourceKind::Group, "CompatGrp2").await.unwrap().unwrap();
    let creator = resolver.preferred(ResourceKind::User, "CompatUser").await.unwrap().unwrap();
    let readable = resolver.preferred(ResourceKind::Chapter, "CompatCh01").await.unwrap().unwrap();
    let duplicate = resolver.preferred(ResourceKind::Chapter, "CompatCh02").await.unwrap().unwrap();
    let relation_key = manga_edge_key("CompatRoot", "CompatSafe");
    let relation = resolver.preferred(ResourceKind::MangaRelation, &relation_key).await.unwrap().unwrap();
    let recommendation = resolver.preferred(ResourceKind::MangaRecommendation, &relation_key).await.unwrap().unwrap();
    assert_ne!(relation, recommendation);
    let alias = PublicUuid::parse("AAAAAAAA-AAAA-8AAA-8AAA-AAAAAAAAAAAA").unwrap();
    resolver.register_alias(ResourceKind::Manga, "CompatRoot", &alias).await.unwrap();
    let before: i64 = db.query_one("SELECT count(*) FROM platform.public_resource_ids", &[]).await.unwrap().get(0);
    drop(db);
    let app = weebdex_platform::router::router(state.clone());
    let _server = Server(tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); }));
    let client = reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(10)).build().unwrap();

    let ping = client.get(format!("{base}/api/v1/ping")).send().await.unwrap();
    assert_eq!(ping.status(), 200);
    assert!(ping.headers()["content-type"].to_str().unwrap().starts_with("text/plain"));
    assert_eq!(ping.text().await.unwrap(), "pong");
    for path in ["/ping".to_owned(), format!("/manga/{root}/recommendation")] {
        let response = client.post(format!("{base}/api/v1{path}")).send().await.unwrap();
        assert_eq!(response.status(), 405);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["result"], "error");
        assert_eq!(body["errors"][0]["status"], 405);
    }

    let manga = get(&client, &base, &format!("/manga/{}?includes[]=tag&includes[]=creator", alias.as_str().to_uppercase()), 200).await;
    assert_eq!(manga["data"]["id"], root.as_str());
    let relations = manga["data"]["relationships"].as_array().unwrap();
    let tag = relations.iter().find(|r| r["type"]=="tag").unwrap();
    assert_eq!(tag["attributes"]["name"]["en"], "Compatibility");
    assert_eq!(relations.iter().find(|r| r["type"]=="creator").unwrap()["id"], creator.as_str());
    assert_public_ids(&manga);
    let bare = get(&client, &base, &format!("/manga/{root}"), 200).await;
    assert!(bare["data"]["relationships"].as_array().unwrap().iter().find(|r| r["type"]=="tag").unwrap().get("attributes").is_none());

    let edges = get(&client, &base, &format!("/manga/{alias}/relation?includes[]=manga"), 200).await;
    assert_eq!(edges["total"], 2);
    assert_eq!(edges["limit"], 2);
    let forward = edges["data"].as_array().unwrap().iter().find(|r| r["id"]==relation.as_str()).unwrap();
    assert_eq!(forward["type"], "manga_relation");
    assert_eq!(forward["attributes"]["relation"], "sequel");
    assert_eq!(forward["attributes"]["version"], 1);
    assert_eq!(forward["relationships"][0]["id"], safe.as_str());
    assert_eq!(forward["relationships"][0]["attributes"]["title"]["ja"], "Safe target");
    assert_public_ids(&edges);
    assert_eq!(get(&client, &base, &format!("/manga/{safe}/relation"), 200).await["data"], json!([]));

    let recs = get(&client, &base, &format!("/manga/{root}/recommendation?includes[]=manga"), 200).await;
    assert_eq!(recs["total"], 1);
    assert_eq!(recs["limit"], 20);
    assert_eq!(recs["data"][0]["id"], recommendation.as_str());
    assert_eq!(recs["data"][0]["type"], "manga_recommendation");
    assert_eq!(recs["data"][0]["attributes"]["score"], 12);
    assert_eq!(recs["data"][0]["relationships"][0]["id"], safe.as_str());
    assert_public_ids(&recs);
    let adult = get(&client, &base, &format!("/manga/{root}/recommendation?contentRating[]=pornographic"), 200).await;
    assert_eq!(adult["total"], 1);
    assert_eq!(adult["data"][0]["relationships"][0]["id"], porn.as_str());
    assert!(adult["data"][0]["relationships"][0].get("attributes").is_none());
    let both = get(&client, &base, &format!("/manga/{root}/recommendation?contentRating[]=safe&contentRating[]=pornographic&order[score]=asc"), 200).await;
    assert_eq!(both["total"], 2);
    assert_eq!(both["data"][0]["attributes"]["score"], 12);
    assert_eq!(both["data"][1]["attributes"]["score"], 99);

    let followers = get(&client, &base, "/group?order[followedCount]=desc", 200).await;
    assert_eq!(followers["data"][0]["id"], group.as_str());
    let ascending = get(&client, &base, &format!("/group?ids[]={group}&ids[]={exact_group}&order[followedCount]=asc"), 200).await;
    assert_eq!(ascending["data"][0]["id"], exact_group.as_str(), "disabled followers do not count");
    let relevance = get(&client, &base, "/group?name=Exact&order[relevance]=desc", 200).await;
    assert_eq!(relevance["data"][0]["id"], exact_group.as_str());
    let page = get(&client, &base, "/manga?order[title]=asc&limit=1&offset=1", 200).await;
    assert_eq!(page["total"], 2);
    assert_eq!(page["data"].as_array().unwrap().len(), 1);
    let count = get(&client, &base, "/manga?limit=0", 200).await;
    assert_eq!(count["total"], 2);
    assert_eq!(count["data"], json!([]));

    let aggregate = get(&client, &base, &format!("/manga/{root}/aggregate?includeUnavailable=1"), 200).await;
    assert_eq!(aggregate["volumes"]["1"]["chapters"]["1"]["id"], readable.as_str());
    assert_eq!(aggregate["volumes"]["1"]["chapters"]["1"]["isUnavailable"], false);
    assert_eq!(aggregate["volumes"]["1"]["chapters"]["1"]["others"], json!([duplicate]));
    assert_eq!(aggregate["volumes"]["2"]["chapters"]["2"]["isUnavailable"], true);
    let available = get(&client, &base, &format!("/manga/{root}/aggregate"), 200).await;
    assert_eq!(available["volumes"]["1"]["count"], 1);
    assert!(available["volumes"].get("2").is_none());

    for path in [
        format!("/manga/{root}/relation?limit=0"),
        format!("/manga/{root}/relation?includes[]=author"),
        format!("/manga/{root}/recommendation?order[score]=random"),
        format!("/manga/{root}/recommendation?contentRating[]=unknown"),
        format!("/manga/{root}/aggregate?includeUnavailable=true"),
        "/manga/CompatRoot/relation".into(),
        "/manga?ids[]=CompatRoot".into(),
        "/manga?limit=101".into(),
        "/manga?offset=-1".into(),
    ] { get(&client, &base, &path, 400).await; }
    get(&client, &base, &format!("/manga/{creator}/relation"), 404).await;
    let after: i64 = state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.public_resource_ids", &[]).await.unwrap().get(0);
    assert_eq!(before, after, "reads must not manufacture aliases");

    // The official recommendation operation has a hard 20-row response cap,
    // while its total remains the real number of eligible persisted edges.
    state.db.client().await.unwrap().batch_execute(r#"
        INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state)
        SELECT 'CompatMore'||n,'CompatUser','Fixture','Target '||n,'en','ongoing','safe','published' FROM generate_series(1,25)n;
        INSERT INTO platform.catalog_recommendations(manga_id,recommended_id,score)
        SELECT 'CompatRoot','CompatMore'||n,100+n FROM generate_series(1,25)n;
    "#).await.unwrap();
    let capped = get(&client, &base, &format!("/manga/{root}/recommendation"), 200).await;
    assert_eq!(capped["total"], 26);
    assert_eq!(capped["data"].as_array().unwrap().len(), 20);
    assert_eq!(capped["data"][0]["attributes"]["score"], 125);
    assert_public_ids(&capped);
    state.db.client().await.unwrap().batch_execute("DELETE FROM platform.catalog_recommendations WHERE recommended_id LIKE 'CompatMore%'; DELETE FROM platform.catalog_manga WHERE id LIKE 'CompatMore%';").await.unwrap();

    // Canonical source authority is evaluated even when the cached flag is true.
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_series_id=repeat('f',64) WHERE id='CompatSafe'", &[]).await.unwrap();
    assert_eq!(get(&client, &base, &format!("/manga/{root}/recommendation"), 200).await["total"], 0);
    assert_eq!(get(&client, &base, &format!("/manga/{root}/relation"), 200).await["total"], 1);
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_series_id=NULL WHERE id='CompatSafe'", &[]).await.unwrap();

    // Persisted edges remain in place while cached target eligibility is revoked.
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='CompatSafe'", &[]).await.unwrap();
    assert_eq!(get(&client, &base, &format!("/manga/{root}/recommendation"), 200).await["total"], 0);
    assert_eq!(get(&client, &base, &format!("/manga/{root}/relation"), 200).await["total"], 1);
    state.db.client().await.unwrap().execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='CompatRoot'", &[]).await.unwrap();
    get(&client, &base, &format!("/manga/{alias}/relation"), 404).await;
    get(&client, &base, &format!("/manga/{root}/recommendation"), 404).await;
}
