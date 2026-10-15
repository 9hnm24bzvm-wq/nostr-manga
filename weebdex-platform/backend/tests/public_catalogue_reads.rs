//! Fresh reconstructed catalogue acceptance over actual loopback TCP + isolated PG.
//! Synthetic native rows are fixtures, not fabricated responses or account authority.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use weebdex_platform::{
    AppState, Config, domains,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind},
};
async fn get(client: &reqwest::Client, base: &str, path: &str, status: u16) -> Value {
    let response = client
        .get(format!("{base}/api/v1{path}"))
        .header("accept", "text/html")
        .send()
        .await
        .unwrap();
    let actual = response.status().as_u16();
    let value: Value = response.json().await.unwrap();
    assert_eq!(actual, status, "{path}: {value}");
    value
}
fn only(value: &Value, id: &PublicUuid) {
    assert_eq!(value["total"], 1, "{value}");
    assert_eq!(value["data"][0]["id"], id.as_str());
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner"]
async fn catalogue_actual_filters_paging_relationships_order_visibility_and_contract() {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(
        pg.get_hosts()
            .iter()
            .all(|h| matches!(h,tokio_postgres::config::Host::Tcp(h)if h=="127.0.0.1"))
    );
    assert!(
        std::env::var("PLATFORM_TEST_RUNTIME_DIR")
            .unwrap()
            .contains("/weebdex-platform/test-runtime/")
    );
    let state = AppState::connect(
        Config {
            database_plaintext_local: true,
            database_connections: 4,
            ..Config::default()
        },
        dsn,
    )
    .await
    .unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    let sql = state.db.client().await.unwrap();
    sql.batch_execute(r#"
 INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES('CatUser001','actual-fixture-user','cat@fixture.invalid','synthetic-no-password',true);
 INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name,metadata) VALUES('CatAuthor1','CatUser001','Fixture','Creator Alpha','{"biography":{"en":"Actual biography"}}'),('CatAuthor2','CatUser001','Fixture','Creator Beta','{}');
 INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name,metadata) VALUES('CatGroup01','CatUser001','Fixture','Group Alpha','{"focused_languages":["en"]}'),('CatGroup02','CatUser001','Fixture','Group Beta','{"focused_languages":["fr"]}');
 INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('CatGroup01','CatUser001','actual-fixture-user',true);
 INSERT INTO platform.catalog_tags(id,name,tag_group) VALUES('CatTag0001','Actual genre','genre'),('CatTag0002','Actual theme','theme'),('CatCustom1','Local category','tag');
 INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,year,language,demographic,publication_status,content_rating,state,metadata,created_at,updated_at) VALUES
 ('CatManga01','CatUser001','Fixture','Alpha Fixture',2024,'ja','shounen','ongoing','safe','published','{"alt_titles":{"en":["Actual alternate"]},"description":{"en":"Actual description"},"links":{"al":"123"}}','2024-01-01','2024-02-01'),
 ('CatManga02','CatUser001','Fixture','Beta Fixture',2025,'ko','seinen','completed','suggestive','published','{}','2025-01-01','2025-02-01'),
 ('CatManga03','CatUser001','Fixture','Gamma Fixture',0,'ja',NULL,'hiatus','pornographic','published','{}','2023-01-01','2023-02-01'),
 ('CatDraft01','CatUser001','Fixture','Hidden draft',2024,'en',NULL,'ongoing','safe','draft','{}','2024-01-01','2024-01-01'),
 ('CatHidden1','CatUser001','Fixture','Hidden source',2024,'en',NULL,'ongoing','safe','published','{}','2024-01-01','2024-01-01');
 UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='CatHidden1';
 INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role) VALUES('CatManga01','CatAuthor1','author'),('CatManga01','CatAuthor1','artist'),('CatManga02','CatAuthor2','author');
 INSERT INTO platform.catalog_manga_tags(manga_id,tag_id) VALUES('CatManga01','CatTag0001'),('CatManga01','CatTag0002'),('CatManga01','CatCustom1'),('CatManga02','CatTag0002');
 INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,is_main,volume,language) VALUES('CatCover01','CatManga01','CatUser001','Fixture','webp',ARRAY[320,480],true,'1','ja');
 INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,volume,chapter,language,metadata,published_at,created_at,updated_at) VALUES
 ('CatChap001','CatManga01','CatUser001','Fixture','First chapter','1','2','en','{"data":[{"name":"opaque-original.webp","dimensions":[32,48]}]}','2024-01-01','2024-01-01','2024-01-01'),
 ('CatChap002','CatManga01','CatUser001','Fixture','Second chapter','1','10','fr','{"data":[{"name":"another-original.webp","dimensions":[64,96]}]}','2024-02-01','2024-02-01','2024-02-01'),
 ('CatChap003','CatManga01','CatUser001','Fixture','External chapter',NULL,NULL,'en','{"data":[],"externalUrl":"https://external.fixture.invalid/read/1"}','2024-03-01','2024-03-01','2024-03-01'),
 ('CatChap004','CatManga02','CatUser001','Fixture','Beta chapter','2','1','en','{"data":[{"name":"beta.webp","dimensions":[16,24]}]}','2025-01-01','2025-01-01','2025-01-01'),
 ('CatChap005','CatManga01','CatUser001','Fixture','Future chapter','2','3','en','{"data":[{"name":"future.webp","dimensions":[16,24]}]}','2999-01-01','2025-03-01','2025-03-01'),
 ('CatChap006','CatDraft01','CatUser001','Fixture','Draft child','1','1','en','{"data":[{"name":"draft.webp","dimensions":[16,24]}]}','2025-01-01','2025-01-01','2025-01-01'),
 ('CatChap007','CatHidden1','CatUser001','Fixture','Cache hidden child','1','1','en','{"data":[{"name":"hidden.webp","dimensions":[16,24]}]}','2025-01-01','2025-01-01','2025-01-01');
 INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('CatChap001','CatGroup01'),('CatChap002','CatGroup02'),('CatChap004','CatGroup02');
 INSERT INTO platform.catalog_manga_counters(manga_id,follows,rating) VALUES('CatManga01',12,7.5),('CatManga02',2,9.0);
 "#).await.unwrap();
    drop(sql);
    let resolver = PostgresPublicIdResolver::new(state.db.clone());
    let mut ids = BTreeMap::new();
    for (key, kind, native) in [
        ("m1", ResourceKind::Manga, "CatManga01"),
        ("m2", ResourceKind::Manga, "CatManga02"),
        ("m3", ResourceKind::Manga, "CatManga03"),
        ("draft", ResourceKind::Manga, "CatDraft01"),
        ("hidden", ResourceKind::Manga, "CatHidden1"),
        ("a1", ResourceKind::Author, "CatAuthor1"),
        ("a2", ResourceKind::Author, "CatAuthor2"),
        ("g1", ResourceKind::Group, "CatGroup01"),
        ("g2", ResourceKind::Group, "CatGroup02"),
        ("t1", ResourceKind::Tag, "CatTag0001"),
        ("t2", ResourceKind::Tag, "CatTag0002"),
        ("cover", ResourceKind::Cover, "CatCover01"),
        ("c1", ResourceKind::Chapter, "CatChap001"),
        ("c2", ResourceKind::Chapter, "CatChap002"),
        ("c3", ResourceKind::Chapter, "CatChap003"),
        ("c4", ResourceKind::Chapter, "CatChap004"),
        ("c5", ResourceKind::Chapter, "CatChap005"),
        ("user", ResourceKind::User, "CatUser001"),
    ] {
        ids.insert(
            key,
            resolver.preferred(kind, native).await.unwrap().unwrap(),
        );
    }
    let alias = PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();
    resolver
        .register_alias(ResourceKind::Manga, "CatManga01", &alias)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = weebdex_platform::router::router(state.clone());
    let _server = Server(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let m = &ids["m1"];
    let m2 = &ids["m2"];
    let a = &ids["a1"];
    let g = &ids["g1"];
    let t = &ids["t1"];
    let t2 = &ids["t2"];
    let user = &ids["user"];
    let manga = get(
        &client,
        &base,
        &format!("/manga/{alias}?includes[]=author&includes[]=artist&includes[]=cover_art"),
        200,
    )
    .await;
    assert_eq!(manga["data"]["id"], m.as_str());
    assert_eq!(manga["data"]["attributes"]["title"]["ja"], "Alpha Fixture");
    assert_eq!(
        manga["data"]["attributes"]["latestUploadedChapter"],
        ids["c5"].as_str()
    );
    let relationships = manga["data"]["relationships"].as_array().unwrap();
    let author = relationships
        .iter()
        .find(|r| r["type"] == "author")
        .unwrap();
    let artist = relationships
        .iter()
        .find(|r| r["type"] == "artist")
        .unwrap();
    assert_eq!(author["id"], artist["id"]);
    assert_eq!(author["attributes"]["biography"]["en"], "Actual biography");
    let cover = relationships
        .iter()
        .find(|r| r["type"] == "cover_art")
        .unwrap();
    assert_eq!(cover["attributes"]["fileName"], "CatCover01.webp");
    assert_eq!(cover["attributes"]["dimensions"], json!([320, 480]));
    for query in [
        format!("ids[]={alias}"),
        format!("authors[]={a}"),
        format!("artists[]={a}"),
        format!("authorOrArtist={a}"),
        format!("group={g}"),
        format!("includedTags[]={t}&includedTags[]={t2}&includedTagsMode=AND"),
        "title=Actual%20alternate".into(),
        "year=2024".into(),
        "originalLanguage[]=ja".into(),
        "availableTranslatedLanguage[]=fr".into(),
        "publicationDemographic[]=shounen".into(),
        "status[]=ongoing".into(),
        "createdAtSince=2024-01-01T00:00:00&updatedAtSince=2024-02-01T00:00:00&year=2024".into(),
    ] {
        only(
            &get(&client, &base, &format!("/manga?{query}"), 200).await,
            m,
        );
    }
    only(
        &get(
            &client,
            &base,
            "/manga?publicationDemographic[]=none&contentRating[]=pornographic",
            200,
        )
        .await,
        &ids["m3"],
    );
    let both = get(
        &client,
        &base,
        "/manga?order[year]=asc&limit=1&offset=1",
        200,
    )
    .await;
    assert_eq!(both["total"], 2);
    assert_eq!(both["data"][0]["id"], m2.as_str());
    let count = get(&client, &base, "/manga?limit=0", 200).await;
    assert_eq!(count["total"], 2);
    assert_eq!(count["data"], json!([]));
    only(
        &get(&client, &base, &format!("/manga?excludedTags[]={t}"), 200).await,
        m2,
    );
    for query in [
        format!("ids[]={}", ids["draft"]),
        format!("ids[]={}", ids["hidden"]),
        format!("authors[]={m}"),
        format!(
            "includedTags[]={t}&includedTags[]=00000000-0000-4000-8000-000000000000&includedTagsMode=AND"
        ),
    ] {
        assert_eq!(
            get(&client, &base, &format!("/manga?{query}"), 200).await["total"],
            0
        );
    }
    let random = get(
        &client,
        &base,
        &format!("/manga/random?includedTags[]={t}"),
        200,
    )
    .await;
    assert_eq!(random["data"]["id"], m.as_str());
    let tags = get(&client, &base, "/manga/tag", 200).await;
    assert_eq!(tags["total"], 3);
    let local = tags["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["attributes"]["name"]["en"] == "Local category")
        .unwrap();
    assert!(local["attributes"].get("group").is_none());
    assert_eq!(local["attributes"]["nostr"]["nativeTagGroup"], "tag");
    let chapter = get(
        &client,
        &base,
        &format!(
            "/chapter/{}?includes[]=manga&includes[]=scanlation_group&includes[]=user",
            ids["c1"]
        ),
        200,
    )
    .await;
    assert_eq!(chapter["data"]["attributes"]["pages"], 1);
    assert_eq!(
        chapter["data"]["attributes"]["pageDimensions"],
        json!([[32, 48]])
    );
    assert_eq!(chapter["data"]["attributes"]["volume"], "1");
    assert!(
        chapter["data"]["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| PublicUuid::parse(r["id"].as_str().unwrap()).is_ok())
    );
    let bare = get(
        &client,
        &base,
        &format!("/chapter?ids[]={}&includes=manga&includes=user", ids["c1"]),
        200,
    )
    .await;
    only(&bare, &ids["c1"]);
    assert!(
        bare["data"][0]["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["type"] == "manga")
            .unwrap()["attributes"]
            .is_object()
    );
    only(
        &get(
            &client,
            &base,
            &format!("/chapter?manga={m}&groups[]={g}&uploader={user}&volume[]=1&chapter=2"),
            200,
        )
        .await,
        &ids["c1"],
    );
    let numeric = get(
        &client,
        &base,
        &format!("/manga/{m}/feed?order[chapter]=asc&includeFuturePublishAt=0"),
        200,
    )
    .await;
    assert_eq!(numeric["total"], 2);
    assert_eq!(numeric["data"][0]["id"], ids["c1"].as_str());
    assert_eq!(numeric["data"][1]["id"], ids["c2"].as_str());
    let external = get(
        &client,
        &base,
        &format!("/manga/{m}/feed?includeEmptyPages=1&externalUrl=external.fixture.invalid"),
        200,
    )
    .await;
    only(&external, &ids["c3"]);
    assert!(external["data"][0]["attributes"]["chapter"].is_null());
    assert!(external["data"][0]["attributes"]["volume"].is_null());
    let exclude = get(
        &client,
        &base,
        &format!("/manga/{m}/feed?includeEmptyPages=1&excludeExternalUrl=external.fixture.invalid"),
        200,
    )
    .await;
    assert_eq!(exclude["total"], 3);
    let filter = get(
        &client,
        &base,
        &format!("/manga/{m}/feed?translatedLanguage[]=fr&limit=500"),
        200,
    )
    .await;
    only(&filter, &ids["c2"]);
    assert_eq!(filter["limit"], 500);
    let aggregate = get(
        &client,
        &base,
        &format!("/manga/{m}/aggregate?translatedLanguage[]=en&groups[]={g}"),
        200,
    )
    .await;
    assert_eq!(
        aggregate["volumes"]["1"]["chapters"]["2"]["id"],
        ids["c1"].as_str()
    );
    assert_eq!(aggregate["volumes"]["1"]["count"], 1);
    let author = get(
        &client,
        &base,
        &format!("/author/{a}?includes[]=manga"),
        200,
    )
    .await;
    assert_eq!(author["data"]["relationships"][0]["id"], m.as_str());
    assert_eq!(
        author["data"]["relationships"][0]["attributes"]["title"]["ja"],
        "Alpha Fixture"
    );
    only(&get(&client, &base, "/author?name=Alpha", 200).await, a);
    let group = get(
        &client,
        &base,
        &format!("/group/{g}?includes[]=leader"),
        200,
    )
    .await;
    assert_eq!(group["data"]["relationships"][0]["type"], "leader");
    assert!(group["data"]["relationships"][0].get("related").is_none());
    assert_eq!(group["data"]["relationships"][0]["id"], user.as_str());
    only(
        &get(&client, &base, "/group?focusedLanguage=en", 200).await,
        g,
    );
    let cover = get(
        &client,
        &base,
        &format!("/cover/{}?includes[]=manga&includes[]=user", ids["cover"]),
        200,
    )
    .await;
    assert_eq!(cover["data"]["attributes"]["fileName"], "CatCover01.webp");
    assert_eq!(cover["data"]["relationships"][0]["id"], m.as_str());
    assert!(cover["data"]["relationships"][0]["attributes"].is_object());
    only(
        &get(
            &client,
            &base,
            &format!("/cover?manga[]={m}&uploaders[]={user}&locales[]=ja"),
            200,
        )
        .await,
        &ids["cover"],
    );
    for path in [
        "/manga?limit=101",
        "/manga?limit=1&limit=2",
        "/manga?native=1",
        "/manga?year=bogus",
        "/manga?createdAtSince=2025-02-29T00:00:00",
        "/manga?order[title]=up",
        "/manga?authors[]=CatAuthor1",
        "/manga?includedTagsMode=bad",
        "/chapter?externalUrl=external.fixture.invalid",
        "/chapter?includeFutureUpdates=true",
        "/manga/random?limit=0",
        "/manga/tag?limit=1",
    ] {
        get(&client, &base, path, 400).await;
    }
    for path in [
        format!("/manga/{}", ids["draft"]),
        format!("/manga/{}", ids["hidden"]),
        "/manga/00000000-0000-4000-8000-000000000000".into(),
    ] {
        get(&client, &base, &path, 404).await;
    }
    get(&client, &base, "/manga/CatManga01", 400).await;
    get(&client, &base, "/group?order[followedCount]=desc", 501).await;
    get(&client, &base, "/unknown", 404).await;
    let response = client
        .patch(format!("{base}/api/v1/manga"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 405);
    // Consistent public visibility at detail, filter, feed and included relationships.
    let sql = state.db.client().await.unwrap();
    sql.execute(
        "UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='CatManga01'",
        &[],
    )
    .await
    .unwrap();
    get(&client, &base, &format!("/manga/{m}"), 404).await;
    get(&client, &base, &format!("/chapter/{}", ids["c1"]), 404).await;
    get(&client, &base, &format!("/cover/{}", ids["cover"]), 404).await;
    get(&client, &base, &format!("/manga/{m}/feed"), 404).await;
    assert_eq!(
        get(
            &client,
            &base,
            &format!("/author/{a}?includes[]=manga"),
            200
        )
        .await["data"]["relationships"],
        json!([])
    );
    sql.execute(
        "UPDATE platform.catalog_manga SET protocol_source_visible=true WHERE id='CatManga01'",
        &[],
    )
    .await
    .unwrap();
    get(&client, &base, &format!("/manga/{m}"), 200).await;
}
