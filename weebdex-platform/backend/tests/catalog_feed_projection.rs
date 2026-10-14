//! Real PostgreSQL/HTTP regression. Invoke only with the existing isolated runner;
//! never silently pass or substitute a successful response when PostgreSQL is absent.
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Instant};
use tower::ServiceExt;
use weebdex_platform::{AppState, Config, domains};

async fn feed(app: &axum::Router, path: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("origin", "https://feed.fixture.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "A real native feed must finish within its unchanged database budget"
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(
        !response
            .headers()
            .contains_key("x-weebdex-fixture-feed-failure")
    );
    serde_json::from_slice(
        &to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap()
}

fn max_discarded_pairs(plan: &Value) -> u64 {
    // PostgreSQL reports a per-loop average, so account for node executions.
    let here = (plan["Rows Removed by Join Filter"].as_f64().unwrap_or(0.0)
        * plan["Actual Loops"].as_f64().unwrap_or(1.0)) as u64;
    plan["Plans"]
        .as_array()
        .into_iter()
        .flatten()
        .map(max_discarded_pairs)
        .fold(here, u64::max)
}

#[test]
fn discarded_pairs_include_repeated_node_executions() {
    assert_eq!(
        max_discarded_pairs(&json!({"Rows Removed by Join Filter":10000.0,"Actual Loops":505})),
        5_050_000
    );
    assert_eq!(
        max_discarded_pairs(
            &json!({"Plans":[{"Rows Removed by Join Filter":0.5,"Actual Loops":2}]})
        ),
        1
    );
    assert_eq!(
        max_discarded_pairs(&json!({"Rows Removed by Join Filter":10000,"Actual Loops":0})),
        0
    );
}

fn has_parent_point_lookup(plan: &Value) -> bool {
    (plan["Relation Name"] == "catalog_manga"
        && plan["Index Name"] == "catalog_manga_pkey"
        && plan["Index Cond"]
            .as_str()
            .is_some_and(|value| value.contains("id = c.manga_id"))
        && plan["Actual Rows"].as_f64().is_some_and(|rows| rows <= 1.0))
        || plan["Plans"]
            .as_array()
            .into_iter()
            .flatten()
            .any(has_parent_point_lookup)
}

async fn assert_thread_visibility_parity(client: &tokio_postgres::Client) {
    for id in [
        "BatchThr01",
        "BatchThr02",
        "BtCp000001",
        "BtCp000002",
        "NoThread00",
    ] {
        assert!(client.query_one("SELECT pg_temp.original_thread_visible($1) IS NOT DISTINCT FROM platform.community_thread_visible($1)", &[&id]).await.unwrap().get::<_,bool>(0), "Every current thread authority result must equal the unchanged canonical predicate");
    }
    assert!(client.query_one("SELECT bool_and(pg_temp.original_thread_visible(id) IS NOT DISTINCT FROM platform.community_thread_visible(id)) FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='BatchUser1'", &[]).await.unwrap().get::<_,bool>(0));
}

fn chapter_without_sort(mut value: Value) -> Value {
    let object = value.as_object_mut().unwrap();
    object.remove("_sort_value");
    object.remove("_sort_text");
    value
}

#[tokio::test]
#[ignore = "Requires scripts/run-auth-tests.sh with its fresh disposable PostgreSQL"]
async fn compact_feed_postgres_projection_parity_visibility_and_pagination() {
    // Whole regression is finite even if a service or native query gets stuck.
    tokio::time::timeout(std::time::Duration::from_secs(180), async {
        let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use the owned PostgreSQL fixture runner");
        let pg: tokio_postgres::Config = dsn.parse().unwrap();
        assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
        assert!(pg.get_hosts().iter().all(|host| matches!(host, tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1")));
        assert!(!pg.get_hosts().is_empty());
        assert!(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap().contains("weebdex-platform/test-runtime/"));
        let state = AppState::connect(Config { public_origin: "https://feed.fixture.invalid".into(), database_plaintext_local: true, fixture_feed_diagnostics: true, ..Config::default() }, dsn).await.unwrap();
        // Reproduce the exact previous87 schema before applying the additive fix.
        let previous = &domains::MIGRATIONS[..domains::MIGRATIONS.len()-1];
        assert_eq!(previous.last().unwrap().version, 87);
        state.db.migrate(previous).await.unwrap();
        let mut repo = manga_node::db::Repository::connect(manga_node::db::DbConfig {
            dsn: std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap(), connections: 1,
            acquisition_timeout: std::time::Duration::from_secs(2), cache_weight: 1024*1024, plaintext_local: true,
        }).await.unwrap();
        repo.pool = state.db.pool().clone();
        repo.migrate().await.unwrap();
        state.protocol.verify_schema().await.unwrap();
        state.protocol.install_visibility_function().await.unwrap();
        let client = state.db.client().await.unwrap();
        let original_thread = include_str!("../migrations/0081_community_visibility.sql").split("CREATE OR REPLACE FUNCTION platform.community_thread_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$\n").nth(1).unwrap().split("\n$$;").next().unwrap();
        client.batch_execute(&format!("CREATE FUNCTION pg_temp.original_thread_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$\n{original_thread}\n$$;")).await.unwrap();
        assert_eq!(client.query_one("SHOW statement_timeout", &[]).await.unwrap().get::<_,String>(0), "5s");
        assert_eq!(client.query_one("SHOW lock_timeout", &[]).await.unwrap().get::<_,String>(0), "1s");
        // Pin deliberately stale fixture statistics. Automatic ANALYZE can hide
        // this regression after its first minute; production policy is untouched.
        client.batch_execute("ALTER TABLE platform.catalog_manga SET(autovacuum_enabled=false); ALTER TABLE platform.catalog_chapters SET(autovacuum_enabled=false)").await.unwrap();
        assert_eq!(client.query_one("SELECT l.lanname FROM pg_proc p JOIN pg_language l ON l.oid=p.prolang WHERE p.oid='platform.protocol_source_is_visible(text)'::regprocedure", &[]).await.unwrap().get::<_,String>(0), "plpgsql");
        // The same relevant cardinalities as the real compact scenario:505
        // Spanish chapters sharing a parent/group, plus10000 metadata-only titles.
        // These are SQL-only synthetic DTO inputs, not genuine media publications.
        client.batch_execute(r#"
INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES('BatchUser1','BatchUser','batch@fixture.invalid','synthetic-unusable',TRUE);
INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,metadata) VALUES('BatchM0001','BatchUser1','BatchUser','Batch compact fixture','ja','ongoing','safe','published','{"description":"synthetic","extra":{"nested":true,"null":null}}'),('BatchM0002','BatchUser1','BatchUser','Batch bilingual fixture','ja','ongoing','safe','published','{}');
INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) SELECT 'BaSr'||lpad(n::text,6,'0'),'BatchUser1','BatchUser','Metadata-only fixture '||n,'ja','ongoing','safe','published' FROM generate_series(1,10000)n;
INSERT INTO platform.catalog_manga_counters(manga_id) SELECT id FROM platform.catalog_manga;
INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name) VALUES('BatchAut01','BatchUser1','BatchUser','Batch author');
INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role) VALUES('BatchM0001','BatchAut01','author');
INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES('BatchGrp01','BatchUser1','BatchUser','Batch selected group'),('BatchGrp02','BatchUser1','BatchUser','Batch outside-page group');
INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader) VALUES('BatchGrp01','BatchUser1','BatchUser',TRUE);
INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,chapter,volume,language,title,published_at,metadata) SELECT 'BaCp'||lpad(n::text,6,'0'),'BatchM0001','BatchUser1','BatchUser',n::text,'1','es','Synthetic chapter '||n,TIMESTAMPTZ '2026-01-01 00:00:00Z'+(505-n)*INTERVAL '1 second','{"node":"https://feed.fixture.invalid","data":[],"extra":{"kept":true,"null":null}}' FROM generate_series(1,505)n;
INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) SELECT id,'BatchGrp01' FROM platform.catalog_chapters;
INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,chapter,language) VALUES('BaOutside1','BatchM0001','BatchUser1','BatchUser','506','fr');
INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('BaOutside1','BatchGrp02'),('BaCp000505','BatchGrp02');
INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject) VALUES('BatchThr01','manga','BatchM0001','BatchUser1','BatchUser','Manga thread'),('BatchThr02','group','BatchGrp01','BatchUser1','BatchUser','Group thread');
INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject) SELECT 'BtCp'||lpad(n::text,6,'0'),'chapter','BaCp'||lpad(n::text,6,'0'),'BatchUser1','BatchUser','Chapter thread' FROM generate_series(1,505)n;
UPDATE platform.catalog_chapters SET is_unavailable=TRUE WHERE id='BaCp000003';
"#).await.unwrap();
        let app = weebdex_platform::router::router(state.clone());
        let baseline_start = Instant::now();
        let baseline = client.query_one(include_str!("fixtures/compact-feed-before-correlated-lookups.sql"), &[&vec!["es".to_owned()], &vec!["safe".to_owned(),"suggestive".to_owned(),"erotica".to_owned()], &100_i64, &0_i64]).await;
        let baseline_ms = baseline_start.elapsed().as_millis();
        let (baseline_outcome, baseline_value) = match baseline {
            Ok(row) => ("success", Some(row.get::<_,Value>(0))),
            Err(error) if error.code() == Some(&tokio_postgres::error::SqlState::QUERY_CANCELED) => {
                assert_eq!(error.as_db_error().unwrap().message(), "canceling statement due to statement timeout", "This owned reproduction must distinguish a real statement timeout from an explicit cancellation");
                ("query-canceled", None)
            },
            Err(_) => panic!("Baseline failed outside its expected success/cancellation observation boundary"),
        };
        state.db.migrate(domains::MIGRATIONS).await.unwrap();
        let batched_start = Instant::now();
        let first = feed(&app, "/api/chapter/feed?tlang=es&limit=100").await;
        let batched_ms = batched_start.elapsed().as_millis();
        assert_eq!(first["total"], 505);
        let stats: i64 = client.query_one("SELECT count(*) FROM pg_statistic WHERE starelid IN('platform.catalog_manga'::regclass,'platform.catalog_chapters'::regclass)", &[]).await.unwrap().get(0);
        assert_eq!(stats, 0, "The regression must pass without ANALYZE warming or a statistics workaround");
        let current_sql = include_str!("fixtures/compact-feed-before-correlated-lookups.sql").replace("JOIN platform.catalog_manga m ON m.id=c.manga_id", "JOIN LATERAL(SELECT * FROM platform.catalog_manga parent WHERE parent.id=c.manga_id OFFSET 0)m ON TRUE");
        let plan: Value = client.query_one(&format!("EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON,TIMING OFF) {current_sql}"), &[&vec!["es".to_owned()], &vec!["safe".to_owned(),"suggestive".to_owned(),"erotica".to_owned()], &100_i64, &0_i64]).await.unwrap().get(0);
        assert!(has_parent_point_lookup(&plan[0]["Plan"]), "The fresh-statistics feed must parameterize the parent primary-key lookup");
        assert!(max_discarded_pairs(&plan[0]["Plan"]) < 10_000, "No repeated unrelated parent catalogue join-filter scan");

        if let Some(baseline) = baseline_value { assert_eq!(first, baseline, "Complete first-page JSON parity, including full map and nested relationships"); }
        // Full first-page canonical parity is mandatory even when the frozen
        // old aggregate cancels. Each canonical row retains its own unchanged5s
        // budget; no long aggregate timeout is introduced for comparison.
        let mut canonical_data = Vec::new();
        for number in 1..=100 {
            let id = format!("BaCp{number:06}");
            canonical_data.push(client.query_one("SELECT platform.catalog_chapter_json(id) || jsonb_build_object('_sort_value',published_at,'_sort_text','') FROM platform.catalog_chapters WHERE id=$1", &[&id]).await.unwrap().get::<_,Value>(0));
        }
        let canonical_parent: Value = client.query_one("SELECT platform.catalog_manga_json('BatchM0001')", &[]).await.unwrap().get(0);
        assert_eq!(first, json!({"total":505,"limit":100,"page":1,"data":canonical_data,"map":{"manga":{"BatchM0001":canonical_parent}}}), "Complete canonical100-row response and map parity is unconditional");
        assert_thread_visibility_parity(&client).await;
        let mut seen = BTreeSet::new();
        for page in 1..=7 {
            let value = if page == 1 { first.clone() } else { feed(&app, &format!("/api/chapter/feed?tlang=es&limit=100&page={page}")).await };
            assert_eq!(value["total"], 505); assert_eq!(value["page"], page); assert_eq!(value["limit"], 100);
            let data = value["data"].as_array().unwrap();
            assert_eq!(data.len(), if page <= 5 {100} else if page == 6 {5} else {0});
            for chapter in data {
                let id = chapter["id"].as_str().unwrap(); assert!(seen.insert(id.to_owned()), "No duplicate native pages or rows");
                assert_eq!(chapter["relationships"]["manga"], value["map"]["manga"]["BatchM0001"]);
                assert_eq!(chapter["relationships"]["groups"][0]["id"], "BatchGrp01");
                if id == "BaCp000003" {
                    assert_eq!(chapter["is_unavailable"], true, "Unavailability is retained as data, not a feed visibility filter");
                    let canonical: Value = client.query_one("SELECT platform.catalog_chapter_json('BaCp000003')", &[]).await.unwrap().get(0);
                    assert_eq!(chapter_without_sort(chapter.clone()), canonical);
                    assert_eq!(chapter["relationships"]["manga"]["relationships"]["available_languages"], json!(["es","fr"]));
                }
                assert_eq!(chapter["relationships"]["manga"]["relationships"]["available_groups"].as_array().unwrap().len(), 2, "Full parent retains groups outside the selected language/page");
            }
            for chapter in data.first().into_iter().chain(data.last()) {
                let canonical: Value = client.query_one("SELECT platform.catalog_chapter_json($1)", &[&chapter["id"].as_str().unwrap()]).await.unwrap().get(0);
                assert_eq!(chapter_without_sort(chapter.clone()), canonical);
            }
        }
        assert_eq!(seen, (1..=505).map(|n|format!("BaCp{n:06}")).collect());
        let ascending = feed(&app, "/api/chapter/feed?tlang=es&limit=1&order=asc").await;
        assert_eq!(ascending["data"][0]["id"], "BaCp000505");
        assert_eq!(ascending["data"][0]["relationships"]["groups"].as_array().unwrap().iter().map(|g|g["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["BatchGrp01","BatchGrp02"]);
        let updates = feed(&app, "/api/chapter/updates?tlang=es&limit=100").await;
        assert_eq!(updates["total"], 1); assert_eq!(updates["data"].as_array().unwrap().len(),1); assert!(updates.get("map").is_none());
        // Missing parents/groups cannot create cache entries: the schema rejects
        // orphan rows and a missing parent route remains a genuine404.
        let orphan_parent = client.batch_execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language) VALUES('BaOrphan01','NoParent01','BatchUser1','BatchUser','es')").await.unwrap_err();
        assert_eq!(orphan_parent.code(), Some(&tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION));
        let orphan_group = client.batch_execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('BaCp000001','NoGroup001')").await.unwrap_err();
        assert_eq!(orphan_group.code(), Some(&tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION));
        let missing = app.clone().oneshot(Request::builder().uri("/api/manga/NoParent01/chapters?limit=1").header("origin","https://feed.fixture.invalid").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let zero_limit = feed(&app, "/api/chapter/feed?tlang=es&limit=0").await;
        assert_eq!(zero_limit["total"], 505); assert_eq!(zero_limit["limit"],0); assert_eq!(zero_limit["page"],1);
        assert_eq!(zero_limit["data"], json!([])); assert_eq!(zero_limit["map"]["manga"],json!({}));
        let empty = feed(&app, "/api/chapter/feed?tlang=uz&limit=100").await;
        assert_eq!(empty["total"],0); assert_eq!(empty["data"],json!([])); assert_eq!(empty["map"]["manga"], json!({}));
        let unmapped = feed(&app, "/api/manga/BatchM0001/chapters?tlang=es&limit=2&sort=chapter&order=asc").await;
        assert!(unmapped.get("map").is_none()); assert_eq!(unmapped["data"][0]["id"], "BaCp000001");
        client.batch_execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,language) VALUES('BaBi000001','BatchM0002','BatchUser1','BatchUser','es')").await.unwrap();
        let bilingual = feed(&app, "/api/chapter/feed?tlang=es&limit=100").await;
        assert_eq!(bilingual["total"], 506);
        assert_eq!(bilingual["map"]["manga"].as_object().unwrap().len(), 2);
        for chapter in bilingual["data"].as_array().unwrap() {
            let parent = chapter["relationships"]["manga"]["id"].as_str().unwrap();
            assert_eq!(chapter["relationships"]["manga"], bilingual["map"]["manga"][parent]);
            if chapter["id"] == "BaBi000001" { assert_eq!(chapter["relationships"]["groups"], json!([])); }
        }
        client.batch_execute("DELETE FROM platform.catalog_chapters WHERE id='BaBi000001'; UPDATE platform.catalog_groups SET deleted_at=clock_timestamp() WHERE id='BatchGrp01'").await.unwrap();
        assert_thread_visibility_parity(&client).await;
        let removed_group = feed(&app, "/api/chapter/feed?tlang=es&limit=1").await;
        assert_eq!(removed_group["data"][0]["relationships"]["groups"], json!([]));
        let canonical: Value = client.query_one("SELECT platform.catalog_chapter_json('BaCp000001')", &[]).await.unwrap().get(0);
        assert_eq!(chapter_without_sort(removed_group["data"][0].clone()), canonical);
        for withdrawal in ["deleted_at=clock_timestamp()", "state='draft'", "protocol_source_visible=FALSE", "protocol_series_id=repeat('a',64)"] {
            client.batch_execute(&format!("UPDATE platform.catalog_manga SET {withdrawal} WHERE id='BatchM0001'")).await.unwrap();
            assert_thread_visibility_parity(&client).await;
            let hidden = feed(&app, "/api/chapter/feed?tlang=es&limit=100").await;
            assert_eq!(hidden["total"], 0); assert_eq!(hidden["data"], json!([])); assert_eq!(hidden["map"]["manga"], json!({}));
            let stale_parent: Option<Value> = client.query_one("SELECT platform.catalog_chapter_json('BaCp000001',$1::jsonb,$2::jsonb)", &[&first["map"]["manga"]["BatchM0001"], &first["data"][0]["relationships"]["groups"]]).await.unwrap().get(0);
            assert!(stale_parent.is_none(), "Cached arguments cannot bypass the original current parent visibility predicate");
            client.batch_execute("UPDATE platform.catalog_manga SET deleted_at=NULL,state='published',protocol_source_visible=TRUE,protocol_series_id=NULL WHERE id='BatchM0001'").await.unwrap();
        }
        client.batch_execute("UPDATE platform.catalog_chapters SET deleted_at=clock_timestamp() WHERE id='BaCp000002'").await.unwrap();
        assert_thread_visibility_parity(&client).await;
        let deleted_chapter = feed(&app, "/api/chapter/feed?tlang=es&limit=100").await;
        assert_eq!(deleted_chapter["total"], 504); assert!(deleted_chapter["data"].as_array().unwrap().iter().all(|row|row["id"] != "BaCp000002"));
        client.batch_execute("UPDATE platform.catalog_chapters SET deleted_at=NULL WHERE id='BaCp000002'; UPDATE platform.catalog_chapters SET protocol_event_id=repeat('b',64) WHERE id='BaCp000001'").await.unwrap();
        assert_thread_visibility_parity(&client).await;
        let hidden_chapter = feed(&app, "/api/chapter/feed?tlang=es&limit=100").await;
        assert_eq!(hidden_chapter["total"], 504); assert!(hidden_chapter["data"].as_array().unwrap().iter().all(|row|row["id"] != "BaCp000001"));
        std::fs::write(std::path::Path::new(&std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap()).join("compact-feed-correlation-evidence.json"), serde_json::to_vec_pretty(&json!({"fixture_only":true,"installed_policy":"plpgsql","statistics_rows":stats,"parent_point_lookup":true,"maximum_discarded_join_pairs":max_discarded_pairs(&plan[0]["Plan"]),"full_canonical_first_page_comparison":true,"baseline_outcome":baseline_outcome,"baseline_ms":baseline_ms,"candidate_first_page_ms":batched_ms})).unwrap()).unwrap();
        client.batch_execute("UPDATE platform.users SET disabled=TRUE WHERE id='BatchUser1'").await.unwrap();
        assert_thread_visibility_parity(&client).await;
        assert!(!client.query_one("SELECT platform.community_thread_visible(id) FROM platform.catalog_threads WHERE entity_type='user' AND entity_id='BatchUser1'", &[]).await.unwrap().get::<_,bool>(0));
        client.batch_execute("UPDATE platform.users SET disabled=FALSE WHERE id='BatchUser1'").await.unwrap();
        println!("{}", json!({"scenario":"compact-feed505-plus10000-metadata","baseline_outcome":baseline_outcome,"baseline_ms":baseline_ms,"batched_first_page_ms":batched_ms,"all_505_ids_reached":true,"bilingual506_preserved":true,"canonical_sample_parity":true,"visibility_withdrawals_verified":true,"statement_timeout_unchanged":true}));
    }).await.expect("Finite180-second PostgreSQL regression deadline");
}
