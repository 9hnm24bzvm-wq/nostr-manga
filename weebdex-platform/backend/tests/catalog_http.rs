//! Public HTTP contracts backed by a disposable PostgreSQL database. Synthetic
//! data is created through real editorial services, never successful response stubs.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use tower::ServiceExt;
use weebdex_platform::{
    AppState, Config,
    domains::{
        self,
        catalog::{CatalogPage, CatalogRepository, ChapterPublication, CoverPublication},
    },
};

async fn request(
    app: &Router,
    method: Method,
    path: &str,
    payload: Option<Value>,
    cookie: Option<&str>,
    expected: StatusCode,
) -> Value {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("origin", "https://catalog.fixture.invalid");
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    let body = match payload {
        Some(p) => {
            request = request.header("content-type", "application/json");
            Body::from(p.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let actual = response.status();
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let result = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| panic!("Non-JSON response for {path}"))
    };
    assert_eq!(actual, expected, "{path}: {result}");
    result
}
async fn op(
    verified: &mut BTreeSet<&'static str>,
    key: &'static str,
    app: &Router,
    method: Method,
    path: &str,
    payload: Option<Value>,
    cookie: Option<&str>,
    expected: StatusCode,
) -> Value {
    let value = request(app, method, path, payload, cookie, expected).await;
    verified.insert(key);
    value
}
async fn fixture() -> AppState {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
        .expect("Use the owned disposable PostgreSQL fixture runner");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(!pg.get_hosts().is_empty());
    for host in pg.get_hosts() {
        match host {
            tokio_postgres::config::Host::Unix(path) => assert!(
                path.to_string_lossy()
                    .contains("weebdex-platform/test-runtime/")
            ),
            tokio_postgres::config::Host::Tcp(host) => assert_eq!(host, "127.0.0.1"),
        }
    }
    assert!(
        std::env::var("PLATFORM_TEST_RUNTIME_DIR")
            .unwrap()
            .contains("weebdex-platform/test-runtime/")
    );
    let cfg = Config {
        public_origin: "https://catalog.fixture.invalid".into(),
        database_plaintext_local: true,
        registration_enabled: false,
        ..Config::default()
    };
    let state = AppState::connect(cfg, dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state
}
async fn make_published(app: &Router, cookie: &str, title: &str, author: &str) -> String {
    let mut manga=request(app,Method::POST,"/api/manga",Some(json!({"title":title,"year":2024,"language":"en","status":"ongoing","content_rating":"safe","authors":[author],"artists":[]})),Some(cookie),StatusCode::CREATED).await;
    let id = manga["id"].as_str().unwrap().to_owned();
    manga = request(
        app,
        Method::POST,
        &format!("/api/manga/draft/{id}/commit"),
        Some(json!({"version":manga["version"]})),
        Some(cookie),
        StatusCode::OK,
    )
    .await;
    request(
        app,
        Method::POST,
        &format!("/api/manga/draft/{id}/publish"),
        Some(json!({"version":manga["version"]})),
        Some(cookie),
        StatusCode::OK,
    )
    .await;
    id
}
#[tokio::test]
#[ignore = "Requires the explicitly isolated PostgreSQL runner"]
async fn persisted_catalog_editorial_and_navigation_contracts() {
    let state = fixture().await;
    let client = state.db.client().await.unwrap();
    let permissions = json!([
        "author.create",
        "author.update",
        "author.delete",
        "author.lock.update",
        "scanlation_group.create",
        "scanlation_group.update",
        "scanlation_group.delete",
        "self.scanlation_group.update",
        "manga.create",
        "manga.update",
        "manga.title.edit",
        "manga.tag.edit",
        "manga.author.edit",
        "manga.artist.edit",
        "manga.link.edit",
        "manga.relation.edit",
        "manga.state.update",
        "manga.lock.update",
        "manga.delete",
        "chapter.create",
        "chapter.update",
        "chapter.remove",
        "self.chapter.update",
        "self.chapter.remove",
        "manga.cover.edit",
        "cover.delete"
    ]);
    let uid = "CatAdmin01";
    client.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES($1,'CatalogAdmin','catalog-admin@fixture.invalid','synthetic-no-password',TRUE,'[\"staff\"]',$2)",&[&uid,&permissions]).await.unwrap();
    client.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES('CatReader1','CatalogReader','catalog-reader@fixture.invalid','synthetic-no-password',TRUE,'[\"user\"]',$1)",&[&permissions]).await.unwrap();
    client.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions) VALUES('CatOwner02','CatalogUploader','catalog-uploader@fixture.invalid','synthetic-no-password',TRUE,'[\"user\"]',$1)",&[&permissions]).await.unwrap();
    let owner_hash = Sha256::digest(b"catalog-fixture-uploader-only").to_vec();
    client.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('CatOwnerS2','CatOwner02',$1,clock_timestamp()+interval '1 hour')", &[&owner_hash]).await.unwrap();
    let read_hash = Sha256::digest(b"catalog-fixture-reader-only").to_vec();
    client.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('CatReaderS','CatReader1',$1,clock_timestamp()+interval '1 hour')",&[&read_hash]).await.unwrap();
    let token = "catalog-fixture-session-only";
    let digest = Sha256::digest(token.as_bytes()).to_vec();
    client.execute("INSERT INTO platform.sessions(id,user_id,token_hash,expires_at) VALUES('CatSession1',$1,$2,clock_timestamp()+interval '1 hour')",&[&uid,&digest]).await.unwrap();
    client.execute("INSERT INTO platform.catalog_tags(id,name,tag_group) VALUES('CatGenre01','Fixture Adventure','genre'),('CatTheme01','Fixture Mystery','theme')",&[]).await.unwrap();
    drop(client);
    let app = weebdex_platform::router::router(state.clone());
    let cookie = format!("weebdex_session={token}");
    let cookie = Some(cookie.as_str());
    let reader = Some("weebdex_session=catalog-fixture-reader-only");
    let mut verified = BTreeSet::new();
    request(
        &app,
        Method::POST,
        "/api/author",
        Some(json!({"name":"Denied anonymous author"})),
        None,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let author = op(
        &mut verified,
        "Author.Create",
        &app,
        Method::POST,
        "/api/author",
        Some(json!({"name":"Fixture Creator"})),
        cookie,
        StatusCode::CREATED,
    )
    .await;
    let aid = author["id"].as_str().unwrap();
    assert_eq!(aid.len(), 10);
    assert!(author["created_at"].as_str().unwrap().ends_with('Z'));
    let author = op(
        &mut verified,
        "Author.Update",
        &app,
        Method::PUT,
        &format!("/api/author/{aid}"),
        Some(json!({"version":1,"description":"UTF-8 著者", "website":"https://fixture.invalid"})),
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(author["version"], 2);
    request(
        &app,
        Method::PUT,
        &format!("/api/author/{aid}"),
        Some(json!({"version":1,"name":"Stale overwrite"})),
        cookie,
        StatusCode::CONFLICT,
    )
    .await;
    op(
        &mut verified,
        "Author.Get",
        &app,
        Method::GET,
        &format!("/api/author/{aid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    let feed = op(
        &mut verified,
        "Author.Search",
        &app,
        Method::GET,
        "/api/author?name=Fixture&limit=100",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(feed["data"][0]["group"], "author");
    let unused = request(
        &app,
        Method::POST,
        "/api/author",
        Some(json!({"name":"Unused Fixture Creator"})),
        cookie,
        StatusCode::CREATED,
    )
    .await;
    op(
        &mut verified,
        "Author.Delete",
        &app,
        Method::DELETE,
        &format!("/api/author/{}", unused["id"].as_str().unwrap()),
        None,
        cookie,
        StatusCode::NO_CONTENT,
    )
    .await;
    let group = op(
        &mut verified,
        "Group.Create",
        &app,
        Method::POST,
        "/api/group",
        Some(json!({"name":"Fixture Scanlation"})),
        cookie,
        StatusCode::CREATED,
    )
    .await;
    let gid = group["id"].as_str().unwrap();
    assert_eq!(group["relationships"]["members"], json!([]));
    let group=op(&mut verified,"Group.Update",&app,Method::PUT,&format!("/api/group/{gid}"),Some(json!({"version":1,"description":"Group fixture","leader":uid,"focused_languages":["en","zh-hk","pt-br","es-la"]})),cookie,StatusCode::OK).await;
    assert_eq!(group["version"], 2);
    op(
        &mut verified,
        "Group.Get",
        &app,
        Method::GET,
        &format!("/api/group/{gid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    op(
        &mut verified,
        "Group.Search",
        &app,
        Method::GET,
        "/api/group?name=Fixture&limit=100",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    let unused = request(
        &app,
        Method::POST,
        "/api/group",
        Some(json!({"name":"Unused fixture group"})),
        cookie,
        StatusCode::CREATED,
    )
    .await;
    op(
        &mut verified,
        "Group.Delete",
        &app,
        Method::DELETE,
        &format!("/api/group/{}", unused["id"].as_str().unwrap()),
        None,
        cookie,
        StatusCode::NO_CONTENT,
    )
    .await;
    let manga=op(&mut verified,"Manga.Create",&app,Method::POST,"/api/manga",Some(json!({"title":"Catalog Vertical Slice","description":"Synthetic UTF-8 漫画","year":2024,"language":"en","status":"ongoing","content_rating":"safe","alt_titles":{"ja":["合成漫画"]},"tags":["CatGenre01"],"authors":[aid],"artists":[aid]})),cookie,StatusCode::CREATED).await;
    let mid = manga["id"].as_str().unwrap();
    assert_eq!(mid.len(), 10);
    assert_eq!(manga["state"], "draft");
    assert!(manga.get("attributes").is_none());
    assert!(
        manga["relationships"]["thread"].is_null(),
        "Unpublished title discussions are not public entities"
    );
    request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &app,
        Method::GET,
        &format!("/api/manga/draft/{mid}"),
        None,
        reader,
        StatusCode::FORBIDDEN,
    )
    .await;
    op(
        &mut verified,
        "Manga.GetDraft",
        &app,
        Method::GET,
        &format!("/api/manga/draft/{mid}"),
        None,
        cookie,
        StatusCode::OK,
    )
    .await;
    op(
        &mut verified,
        "Manga.SearchDrafts",
        &app,
        Method::GET,
        "/api/manga/draft?title=Catalog&limit=100",
        None,
        cookie,
        StatusCode::OK,
    )
    .await;
    let manga=op(&mut verified,"Manga.Update",&app,Method::PUT,&format!("/api/manga/{mid}"),Some(json!({"version":1,"title":"Catalog Vertical Slice","description":"Persisted edit","tags":["CatGenre01","CatTheme01"]})),cookie,StatusCode::OK).await;
    assert_eq!(manga["version"], 2);
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}"),
        Some(json!({"version":1,"description":"Stale edit"})),
        cookie,
        StatusCode::CONFLICT,
    )
    .await;
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}"),
        Some(json!({"version":2,"authors":["Unknown001"],"title":"Partial write forbidden"})),
        cookie,
        StatusCode::BAD_REQUEST,
    )
    .await;
    let unchanged = request(
        &app,
        Method::GET,
        &format!("/api/manga/draft/{mid}"),
        None,
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(unchanged["version"], 2);
    assert_eq!(unchanged["title"], "Catalog Vertical Slice");
    let manga = op(
        &mut verified,
        "Manga.Commit",
        &app,
        Method::POST,
        &format!("/api/manga/draft/{mid}/commit"),
        Some(json!({"version":2})),
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(manga["state"], "submitted");
    let manga = op(
        &mut verified,
        "Manga.Publish",
        &app,
        Method::POST,
        &format!("/api/manga/draft/{mid}/publish"),
        Some(json!({"version":3})),
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(manga["state"], "published");
    assert!(manga["published_at"].is_string());
    assert!(
        manga["relationships"]["thread"]["id"].is_string(),
        "Publishing exposes the genuine persisted discussion root"
    );
    let missing = request(
        &app,
        Method::GET,
        "/api/manga/missing001",
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(missing["code"], "not_found");
    let chapter_id = "CatChap001";
    let mut db = state.db.client().await.unwrap();
    let tx = db.transaction().await.unwrap();
    CatalogRepository::publish_chapter(
        &tx,
        &ChapterPublication {
            id: chapter_id.into(),
            manga_id: mid.into(),
            uploader_id: uid.into(),
            uploader_name: "CatalogAdmin".into(),
            title: Some("Synthetic first chapter".into()),
            volume: Some("1".into()),
            chapter: Some("1.5".into()),
            language: "en".into(),
            node: "https://catalog.fixture.invalid".into(),
            pages: vec![CatalogPage {
                name: "fixture-page.png".into(),
                dimensions: [1, 1],
            }],
            optimized_pages: vec![],
            groups: vec![gid.into()],
            source_id: None,
            protocol_event_id: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(db);
    let detail = op(
        &mut verified,
        "Manga.Get",
        &app,
        Method::GET,
        &format!("/api/manga/{mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        detail["relationships"]["available_languages"],
        json!(["en"])
    );
    assert_eq!(detail["relationships"]["available_groups"][0]["id"], gid);
    assert_eq!(detail["relationships"]["authors"][0]["id"], aid);
    assert_eq!(detail["relationships"]["stats"]["chapters"], 1);
    let feed=op(&mut verified,"Manga.Search",&app,Method::GET,&format!("/api/manga?title=Catalog&yearFrom=2020&yearTo=2025&lang=en&status=ongoing&contentRating=safe&tag=CatGenre01&tag=CatTheme01&tmod=AND&author={aid}&artists={aid}&hasChapters=1&availableTranslatedLang=en&sort=title&order=asc"),None,None,StatusCode::OK).await;
    assert_eq!(feed["total"], 1);
    assert_eq!(feed["data"][0]["id"], mid);
    let none = request(
        &app,
        Method::GET,
        "/api/manga?title=Catalog&tagx=CatGenre01&tagx=CatTheme01&txmod=AND",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(none["total"], 0);
    let alternate = request(
        &app,
        Method::GET,
        "/api/manga?title=%E5%90%88%E6%88%90%E6%BC%AB%E7%94%BB",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(alternate["data"][0]["id"], mid);
    let page = request(
        &app,
        Method::GET,
        "/api/manga?title=Catalog&page=2&limit=1",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["data"], json!([]));
    op(
        &mut verified,
        "Manga.Random",
        &app,
        Method::GET,
        "/api/manga/random?title=Catalog&hasChapters=1",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    op(
        &mut verified,
        "Manga.Trending",
        &app,
        Method::GET,
        "/api/manga/top?rank=read&time=30d",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    let tags = op(
        &mut verified,
        "Tag.Search",
        &app,
        Method::GET,
        "/api/manga/tag?name=Fixture&limit=100",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(tags["total"], 2);
    let chapter = op(
        &mut verified,
        "Chapter.Get",
        &app,
        Method::GET,
        &format!("/api/chapter/{chapter_id}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(chapter["relationships"]["manga"]["id"], mid);
    assert_eq!(chapter["data"][0]["dimensions"], json!([1, 1]));
    assert_eq!(chapter["publication_scope"], "instance-local");
    assert_eq!(chapter["publication_status"], "local");
    assert!(
        chapter.get("data_optimized").is_none(),
        "Empty optimized arrays break the unchanged reader's truthy fallback"
    );
    for (key, path) in [
        ("Chapter.Search", "/api/chapter?tlang=en&limit=100"),
        ("Chapter.LatestFeed", "/api/chapter/feed?tlang=en&limit=100"),
        (
            "Chapter.Trending",
            "/api/chapter/top?contentRating=safe&time=30d",
        ),
    ] {
        let feed = op(
            &mut verified,
            key,
            &app,
            Method::GET,
            path,
            None,
            None,
            StatusCode::OK,
        )
        .await;
        assert_eq!(feed["data"][0]["id"], chapter_id);
        assert_eq!(feed["map"]["manga"][mid]["id"], mid);
    }
    op(
        &mut verified,
        "Chapter.LatestUpdates",
        &app,
        Method::GET,
        "/api/chapter/updates?limit=100",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    let chapters = op(
        &mut verified,
        "Chapter.List",
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/chapters?limit=0&tlang=en"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(chapters["total"], 1);
    assert_eq!(chapters["data"], json!([]));
    let aggregate = op(
        &mut verified,
        "Chapter.Aggregate",
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/aggregate?tlang=en&group={gid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(aggregate["languages"], json!(["en"]));
    assert_eq!(aggregate["groups"][0]["id"], gid);
    assert_eq!(
        aggregate["chapters"][0]["entries"][chapter_id]["language"],
        0
    );
    assert_eq!(
        aggregate["chapters"][0]["entries"][chapter_id]["groups"],
        json!([0])
    );
    let related = make_published(&app, cookie.unwrap(), "Related Synthetic Title", aid).await;
    let relation_owner = request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    let before_relation = relation_owner["version"].as_i64().unwrap();
    let created_relation = op(
        &mut verified,
        "Manga.CreateRelation",
        &app,
        Method::POST,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"type":"sequel","version":before_relation})),
        cookie,
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(created_relation["manga_version"], before_relation + 1);
    assert_eq!(created_relation["relation_scope"], "instance-local");
    for kind in ["sequel", "prequel"] {
        request(
            &app,
            Method::POST,
            &format!("/api/manga/{mid}/relations"),
            Some(json!({"related_id":related,"type":kind})),
            cookie,
            StatusCode::CONFLICT,
        )
        .await;
    }
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"type":"side_story"})),
        cookie,
        StatusCode::BAD_REQUEST,
    )
    .await;
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"type":"side_story","version":before_relation})),
        cookie,
        StatusCode::CONFLICT,
    )
    .await;
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"type":"side_story","version":before_relation+1})),
        None,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    state.db.client().await.unwrap().execute("UPDATE platform.users SET permissions=permissions-'manga.relation.edit' WHERE id='CatReader1'",&[]).await.unwrap();
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"type":"side_story","version":before_relation+1})),
        reader,
        StatusCode::FORBIDDEN,
    )
    .await;
    let unchanged = request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/relations"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(unchanged.as_array().unwrap().len(), 1);
    assert_eq!(unchanged[0]["type"], "sequel");
    let edited_relation = request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"type":"side_story","version":before_relation+1})),
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(edited_relation["id"], related);
    assert_eq!(edited_relation["type"], "side_story");
    assert_eq!(edited_relation["manga_version"], before_relation + 2);
    let unchanged_edit = request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"type":"side_story","version":before_relation+2})),
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(unchanged_edit["manga_version"], before_relation + 2);
    let unrelated = make_published(
        &app,
        cookie.unwrap(),
        "Unrelated relation update target",
        aid,
    )
    .await;
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":unrelated,"type":"sequel","version":before_relation+2})),
        cookie,
        StatusCode::NOT_FOUND,
    )
    .await;
    let relations = op(
        &mut verified,
        "Manga.Relations",
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/relations"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(relations[0]["id"], related);
    assert_eq!(relations[0]["type"], "side_story");
    let db = state.db.client().await.unwrap();
    let persisted=db.query_one("SELECT m.version,r.relation_type,(SELECT count(*) FROM platform.catalog_audit a WHERE a.entity_type='manga' AND a.entity_id=m.id AND a.operation='relation_update') FROM platform.catalog_manga m JOIN platform.catalog_relations r ON r.manga_id=m.id WHERE m.id=$1 AND r.related_id=$2",&[&mid,&related]).await.unwrap();
    assert_eq!(persisted.get::<_, i32>(0) as i64, before_relation + 2);
    assert_eq!(persisted.get::<_, String>(1), "side_story");
    assert_eq!(persisted.get::<_, i64>(2), 1);
    drop(db);
    let db = state.db.client().await.unwrap();
    db.execute(
        "INSERT INTO platform.catalog_recommendations(manga_id,recommended_id) VALUES($1,$2)",
        &[&mid, &related],
    )
    .await
    .unwrap();
    drop(db);
    let rec = op(
        &mut verified,
        "Manga.Recommendations",
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/recommendations"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(rec["total"], 1);
    assert_eq!(rec["data"][0]["id"], related);
    request(
        &app,
        Method::DELETE,
        &format!("/api/manga/{mid}/relations"),
        Some(json!({"related_id":related,"version":before_relation+1})),
        cookie,
        StatusCode::CONFLICT,
    )
    .await;
    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/manga/{mid}/relations"))
                .header("origin", "https://catalog.fixture.invalid")
                .header("cookie", cookie.unwrap())
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"related_id":related,"version":before_relation+2}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        delete.headers()["x-weebdex-manga-version"]
            .to_str()
            .unwrap(),
        (before_relation + 3).to_string()
    );
    assert_eq!(delete.headers()["cache-control"], "no-store");
    assert!(to_bytes(delete.into_body(), 1024).await.unwrap().is_empty());
    verified.insert("Manga.DeleteRelation");
    let locked = op(
        &mut verified,
        "Manga.Lock",
        &app,
        Method::POST,
        &format!("/api/manga/{mid}/lock"),
        None,
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(locked["locked"], true);
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}"),
        Some(json!({"version":locked["version"],"title":"Forbidden locked overwrite"})),
        reader,
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &app,
        Method::POST,
        &format!("/api/manga/{mid}/unlock"),
        None,
        reader,
        StatusCode::FORBIDDEN,
    )
    .await;
    let unlocked = op(
        &mut verified,
        "Manga.Unlock",
        &app,
        Method::POST,
        &format!("/api/manga/{mid}/unlock"),
        None,
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(unlocked["locked"], false);
    let rejected=request(&app,Method::POST,"/api/manga",Some(json!({"title":"Rejected synthetic draft","language":"en","status":"ongoing","content_rating":"safe","authors":[aid],"artists":[]})),cookie,StatusCode::CREATED).await;
    let rid = rejected["id"].as_str().unwrap();
    request(
        &app,
        Method::POST,
        &format!("/api/manga/draft/{rid}/commit"),
        Some(json!({"version":1})),
        cookie,
        StatusCode::OK,
    )
    .await;
    let rejected = op(
        &mut verified,
        "Manga.Reject",
        &app,
        Method::POST,
        &format!("/api/manga/draft/{rid}/reject"),
        Some(json!({"version":2,"reason":"Clarify the synthetic draft description"})),
        cookie,
        StatusCode::OK,
    )
    .await;
    assert_eq!(rejected["state"], "rejected");
    op(
        &mut verified,
        "Manga.Delete",
        &app,
        Method::DELETE,
        &format!("/api/manga/{rid}"),
        None,
        cookie,
        StatusCode::NO_CONTENT,
    )
    .await;

    let db = state.db.client().await.unwrap();
    db.execute("INSERT INTO platform.catalog_tags(id,name,tag_group) SELECT 'ExtraTag'||lpad(i::text,2,'0'),'Bootstrap tag '||i::text,'tag' FROM generate_series(1,25)i",&[]).await.unwrap();
    drop(db);
    let bootstrap_tags = request(
        &app,
        Method::GET,
        "/api/manga/tag",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(bootstrap_tags["total"], 27);
    assert_eq!(
        bootstrap_tags["data"].as_array().unwrap().len(),
        27,
        "The unchanged global store requests the entire taxonomy without limit"
    );
    // The retained Cover.List operation returns the persisted flat cover array.
    let mut cover_db = state.db.client().await.unwrap();
    let tx = cover_db.transaction().await.unwrap();
    CatalogRepository::publish_cover(
        &tx,
        &CoverPublication {
            id: "CatCover01".into(),
            manga_id: mid.into(),
            uploader_id: uid.into(),
            uploader_name: "CatalogAdmin".into(),
            ext: "png".into(),
            main: true,
            dimensions: [1, 1],
            volume: "1".into(),
            language: "en".into(),
            description: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(cover_db);
    let covers = request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/covers"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(covers[0]["id"], "CatCover01");
    assert_eq!(covers[0]["dimensions"], json!([1, 1]));
    assert!(covers[0].get("attributes").is_none());
    // Public discovery honors the unchanged preference defaults, and an explicit
    // all-unchecked overlay sentinel cannot accidentally widen the results.
    let adult = make_published(&app, cookie.unwrap(), "Rating Exclusion Fixture", aid).await;
    let adult_detail = request(
        &app,
        Method::GET,
        &format!("/api/manga/{adult}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{adult}"),
        Some(json!({"version":adult_detail["version"],"content_rating":"pornographic"})),
        cookie,
        StatusCode::OK,
    )
    .await;
    let default_rating = request(
        &app,
        Method::GET,
        "/api/manga?title=Rating%20Exclusion%20Fixture",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(default_rating["total"], 0);
    let explicit_rating = request(
        &app,
        Method::GET,
        "/api/manga?title=Rating%20Exclusion%20Fixture&contentRating=pornographic",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(explicit_rating["total"], 1);
    for path in [
        "/api/manga?contentRating=__none__",
        "/api/manga/top?contentRating=__none__",
        "/api/chapter/feed?contentRating=__none__",
    ] {
        let empty = request(&app, Method::GET, path, None, None, StatusCode::OK).await;
        assert_eq!(empty["total"], 0);
        assert_eq!(empty["data"], json!([]));
    }
    request(
        &app,
        Method::GET,
        "/api/manga?contentRating=__none__&contentRating=safe",
        None,
        None,
        StatusCode::BAD_REQUEST,
    )
    .await;
    // The retained reader navigates aggregate positions descending. Suffixes must
    // remain stable after their numeric prefix rather than sorting 10 below 2.
    let mut db = state.db.client().await.unwrap();
    let tx = db.transaction().await.unwrap();
    for (id, number) in [
        ("CatChap002", "2"),
        ("CatChap010", "10"),
        ("CatChapEx1", "10extra"),
    ] {
        CatalogRepository::publish_chapter(
            &tx,
            &ChapterPublication {
                id: id.into(),
                manga_id: mid.into(),
                uploader_id: uid.into(),
                uploader_name: "CatalogAdmin".into(),
                title: None,
                volume: Some("1".into()),
                chapter: Some(number.into()),
                language: "en".into(),
                node: "https://catalog.fixture.invalid".into(),
                pages: vec![CatalogPage {
                    name: "fixture-page.png".into(),
                    dimensions: [1, 1],
                }],
                optimized_pages: vec![],
                groups: vec![gid.into()],
                source_id: None,
                protocol_event_id: None,
            },
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    drop(db);
    let aggregate = request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/aggregate"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    let chapter_order = aggregate["chapters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["chapter"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(chapter_order, vec!["10extra", "10", "2", "1.5"]);
    let chapter_list = request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/chapters?limit=100"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(chapter_list["data"][0]["chapter"], "10extra");
    assert_eq!(chapter_list["data"][3]["chapter"], "1.5");
    // Device-library hydration applies the same public filters across bounded
    // ID batches and receives the real SQL sort key for a global merge.
    let hydrated = request(
        &app,
        Method::GET,
        &format!("/api/manga?id={mid}&id={related}&sort=lastUploadedChapterAt&order=desc"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(hydrated["total"], 2);
    assert_eq!(hydrated["data"][0]["id"], mid);
    assert!(hydrated["data"][0]["_sort_value"].is_string());
    assert!(hydrated["data"][1]["_sort_value"].is_null());
    let direct_chapter = request(
        &app,
        Method::GET,
        &format!("/api/chapter?id={chapter_id}&manga={mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        direct_chapter["total"], 1,
        "Chapter IDs are not also parent manga ID filters"
    );
    assert_eq!(direct_chapter["data"][0]["id"], chapter_id);
    let current = request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{mid}"),
        Some(json!({"version":current["version"],"links":{"mal":"987654321"}})),
        cookie,
        StatusCode::OK,
    )
    .await;
    let mapped = request(
        &app,
        Method::POST,
        "/api/manga/import-mappings",
        Some(json!({"myanimelist":["987654321","987654322","987654321"]})),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        mapped["data"],
        json!([{"myanimelist":"987654321","id":mid,"title":current["title"]}])
    );
    assert_eq!(mapped["missing"], json!(["987654322"]));
    assert_eq!(mapped["ambiguous"], json!([]));
    let other = request(
        &app,
        Method::GET,
        &format!("/api/manga/{related}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    request(
        &app,
        Method::PUT,
        &format!("/api/manga/{related}"),
        Some(json!({"version":other["version"],"links":{"myanimelist":"987654321"}})),
        cookie,
        StatusCode::OK,
    )
    .await;
    let ambiguous = request(
        &app,
        Method::POST,
        "/api/manga/import-mappings",
        Some(json!({"myanimelist":["987654321"]})),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(ambiguous["data"], json!([]));
    assert_eq!(ambiguous["ambiguous"], json!(["987654321"]));
    request(
        &app,
        Method::POST,
        "/api/manga/import-mappings",
        Some(json!({"myanimelist":["../not-decimal"]})),
        None,
        StatusCode::BAD_REQUEST,
    )
    .await;
    // Soft-hiding a group retains its historical memberships, but those rows
    // cannot continue to confer chapter edit/remove/claim authority.
    let delegated_group = request(
        &app,
        Method::POST,
        "/api/group",
        Some(json!({"name":"Delegation revocation fixture"})),
        cookie,
        StatusCode::CREATED,
    )
    .await;
    let delegated_gid = delegated_group["id"].as_str().unwrap();
    request(
        &app,
        Method::PUT,
        &format!("/api/group/{delegated_gid}"),
        Some(json!({"version":1,"leader":"CatReader1"})),
        cookie,
        StatusCode::OK,
    )
    .await;
    let mut db = state.db.client().await.unwrap();
    let tx = db.transaction().await.unwrap();
    CatalogRepository::publish_chapter(
        &tx,
        &ChapterPublication {
            id: "CatDeleg01".into(),
            manga_id: mid.into(),
            uploader_id: "CatOwner02".into(),
            uploader_name: "CatalogUploader".into(),
            title: None,
            volume: Some("1".into()),
            chapter: Some("20".into()),
            language: "en".into(),
            node: "https://catalog.fixture.invalid".into(),
            pages: vec![CatalogPage {
                name: "delegation.png".into(),
                dimensions: [1, 1],
            }],
            optimized_pages: vec![],
            groups: vec![delegated_gid.into()],
            source_id: None,
            protocol_event_id: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(db);
    request(
        &app,
        Method::PUT,
        "/api/chapter/CatDeleg01",
        Some(json!({"version":1,"title":"Eligible group leader edit"})),
        reader,
        StatusCode::OK,
    )
    .await;
    request(
        &app,
        Method::DELETE,
        &format!("/api/group/{delegated_gid}"),
        None,
        cookie,
        StatusCode::NO_CONTENT,
    )
    .await;
    let without_group = request(
        &app,
        Method::GET,
        "/api/chapter/CatDeleg01",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        without_group["relationships"]["groups"],
        json!([]),
        "Historical group joins never project null eligibility entries"
    );
    request(
        &app,
        Method::PUT,
        "/api/chapter/CatDeleg01",
        Some(json!({"version":2,"title":"Dead group must not delegate"})),
        reader,
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &app,
        Method::DELETE,
        "/api/chapter/CatDeleg01",
        None,
        reader,
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &app,
        Method::POST,
        "/api/chapter/CatDeleg01/claim",
        None,
        reader,
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &app,
        Method::POST,
        "/api/upload/begin",
        Some(json!({"manga":mid,"groups":[delegated_gid]})),
        cookie,
        StatusCode::BAD_REQUEST,
    )
    .await;
    let db = state.db.client().await.unwrap();
    assert!(db.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND user_id='CatReader1' AND is_leader)",&[&delegated_gid]).await.unwrap().get::<_,bool>(0),"Historical membership remains; current group eligibility denies authority");
    drop(db);
    let uploader = Some("weebdex_session=catalog-fixture-uploader-only");
    request(
        &app,
        Method::PUT,
        "/api/chapter/CatDeleg01",
        Some(json!({"version":2,"title":"Actual uploader retains own authority"})),
        uploader,
        StatusCode::OK,
    )
    .await;
    request(
        &app,
        Method::DELETE,
        "/api/chapter/CatDeleg01",
        None,
        uploader,
        StatusCode::NO_CONTENT,
    )
    .await;
    // A standalone native installation has no protocol schema. A source-linked
    // row therefore disappears immediately through every public read, without
    // waiting for the importer or trusting a previously materialized flag.
    let source_id = "a".repeat(64);
    let db = state.db.client().await.unwrap();
    db.execute(
        "UPDATE platform.catalog_manga SET protocol_series_id=$2 WHERE id=$1",
        &[&mid, &source_id],
    )
    .await
    .unwrap();
    drop(db);
    request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &app,
        Method::GET,
        &format!("/api/chapter/{chapter_id}"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/aggregate"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/relations"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &app,
        Method::GET,
        &format!("/api/manga/{mid}/recommendations"),
        None,
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let hidden = request(
        &app,
        Method::GET,
        "/api/manga?title=Catalog",
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(hidden["total"], 0);
    assert_eq!(hidden["data"], json!([]));
    let hidden_chapters = request(
        &app,
        Method::GET,
        &format!("/api/chapter?manga={mid}"),
        None,
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(hidden_chapters["total"], 0);
    assert_eq!(hidden_chapters["data"], json!([]));
    assert_eq!(
        verified.len(),
        36,
        "Every observed catalog operation must be exercised"
    );
    println!("Catalog HTTP operations verified: {}", verified.len());
    if let Ok(path) = std::env::var("PLATFORM_TEST_RUNTIME_DIR") {
        std::fs::write(std::path::Path::new(&path).join("catalog-http-evidence.json"),serde_json::to_vec_pretty(&json!({"synthetic_fixture":true,"observed_operations":verified,"count":36,"backend":"real PostgreSQL plus public Router","UI_browser_test":"not run"})).unwrap()).unwrap();
    }
}
