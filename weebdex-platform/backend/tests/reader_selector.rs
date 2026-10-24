//! Source-authored acceptance tests for the bounded reader. These tests are
//! deliberately ignored outside the owned, disposable PostgreSQL runner.
//! Real loopback HTTP, real catalogue SQL and real serialized responses only.
//! This source file is not evidence that its scenarios have been executed.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use weebdex_platform::{
    AppState, Config, domains,
    public_api::{PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind},
};

struct Fixture {
    state: AppState,
    client: reqwest::Client,
    base: String,
    user: String,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) { self.server.abort(); }
}
impl Fixture {
    async fn new(name: &str) -> Self {
        let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
            .expect("Use the explicitly isolated PostgreSQL fixture runner");
        let pg: tokio_postgres::Config = dsn.parse().unwrap();
        assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
        assert!(!pg.get_hosts().is_empty());
        assert_eq!(pg.get_hosts().len(), 1);
        assert!(pg.get_hosts().iter().all(|host|
            matches!(host, tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1")));
        assert_eq!(pg.get_ports().len(), 1);
        assert!(pg.get_ports().iter().all(|port| *port > 1024 && *port != 5432));
        let runtime = std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
        assert!(runtime.to_string_lossy().contains("/weebdex-platform/test-runtime/"));
        assert!(runtime.to_string_lossy().contains("/weebdex-platform/test-runtime/auth-"));
        // Match the existing library_http and completion fixture admission:
        // consume auth_fixture_guard.py's actual receipt before any listener,
        // AppState connection, migration or fixture mutation. This is not a
        // substitute fixture owner or permission to launch the ignored tests.
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(runtime.join("fixture.json")).expect("Owned disposable fixture receipt"),
        ).unwrap();
        assert_eq!(receipt["database"], "weebdex_platform_test");
        assert_eq!(receipt["fixture_only"], true);
        assert_eq!(receipt["transport"], "private loopback TCP");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = AppState::connect(Config {
            database_plaintext_local: true,
            database_connections: 4,
            public_origin: base.clone(),
            uploads_dir: runtime.join(format!("reader-{name}-media")),
            ui_dir: runtime.join("no-ui"),
            ..Config::default()
        }, dsn).await.unwrap();
        state.db.migrate(domains::MIGRATIONS).await.unwrap();
        state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
        let user = format!("RS{name}User");
        state.db.client().await.unwrap().execute(
            "INSERT INTO platform.users(id,username,email,password_hash,verified) VALUES($1,$2,$3,'synthetic-no-password',true)",
            &[&user, &format!("reader-{}", name.to_lowercase()), &format!("reader-{name}@fixture.invalid")],
        ).await.unwrap();
        let app = weebdex_platform::router::router(state.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        let client = reqwest::Client::builder().no_proxy()
            .timeout(std::time::Duration::from_secs(15)).build().unwrap();
        Self { state, client, base, user, server }
    }
    async fn sql(&self, statement: &str) {
        self.state.db.client().await.unwrap().batch_execute(statement).await.unwrap();
    }
    async fn manga(&self, id: &str, rating: &str) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES($1,$2,'Reader fixture',$1,'ja','ongoing',$3,'published')",
            &[&id, &self.user, &rating],
        ).await.unwrap();
    }
    async fn group(&self, id: &str) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name) VALUES($1,$2,'Reader fixture',$1)",
            &[&id, &self.user],
        ).await.unwrap();
    }
    async fn chapter(&self, id: &str, manga: &str, volume: Option<&str>, chapter: Option<&str>) {
        self.state.db.client().await.unwrap().execute(
            "INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,volume,chapter,language,published_at,created_at,updated_at) VALUES($1,$2,$3,'Reader fixture',$4,$5,'en','2024-01-01','2024-01-01','2024-01-01')",
            &[&id, &manga, &self.user, &volume, &chapter],
        ).await.unwrap();
    }
    async fn reference(&self, public: bool, kind: ResourceKind, native: &str) -> String {
        if !public { return native.to_owned(); }
        PostgresPublicIdResolver::new(self.state.db.clone()).preferred(kind, native)
            .await.unwrap().unwrap().to_string()
    }
    async fn reader_path(&self, public: bool, manga: &str) -> String {
        let id = self.reference(public, ResourceKind::Manga, manga).await;
        format!("{}/manga/{id}/reader", if public { "/api/v1" } else { "/api" })
    }
    async fn wire(&self, path: &str, expected: u16) -> String {
        let response = self.client.get(format!("{}{path}", self.base)).send().await.unwrap();
        let status = response.status().as_u16();
        if expected == 200 && path.contains("/reader") {
            assert_eq!(response.headers().get("cache-control").and_then(|h| h.to_str().ok()), Some("no-store"));
        }
        let raw = response.text().await.unwrap();
        assert_eq!(status, expected, "{path}: {raw}");
        raw
    }
    async fn get_raw(&self, path: &str, expected: u16) -> (Value, String) {
        let raw = self.wire(path, expected).await;
        let value: Value = serde_json::from_str(&raw).unwrap_or_else(|_| panic!("Non-JSON response: {path}: {raw}"));
        if expected == 200 && path.contains("/reader") {
            assert!(raw.len() <= 256 * 1024, "reader byte budget");
            let identity=value["catalogIdentity"].as_object().expect("Every selector state carries catalogue identity");
            assert_eq!(identity.len(),2);
            let namespace=identity["catalogNamespace"].as_str().unwrap();
            assert_eq!(PublicUuid::parse(namespace).unwrap().as_str(),namespace);
            assert!(identity.contains_key("serviceKey"));
            assert!(identity["serviceKey"].is_null() || identity["serviceKey"].as_str().is_some_and(|key|
                key.len()==64 && key.bytes().all(|b|b.is_ascii_digit() || (b'a'..=b'f').contains(&b))));
            assert_eq!(value["consistency"], "live-keyset-v1");
            assert_eq!(value["crossRequestSnapshot"], false);
            assert_eq!(value["history"], "device-local-only");
            assert_eq!(value["continueProof"], "observed-only");
            assert!(value["data"].as_array().unwrap().len() <= 100);
            for row in value["data"].as_array().unwrap() { minimal_row(row); }
        }
        (value, raw)
    }
    async fn get(&self, path: &str, expected: u16) -> Value { self.get_raw(path, expected).await.0 }
}
fn query(path: &str, pairs: &[(&str, &str)]) -> String {
    let mut wire = url::form_urlencoded::Serializer::new(String::new());
    wire.extend_pairs(pairs.iter().copied());
    format!("{path}?{}", wire.finish())
}
fn ids(page: &Value) -> Vec<String> {
    page["data"].as_array().unwrap().iter().map(|row| row["id"].as_str().unwrap().to_owned()).collect()
}
fn labels(page: &Value) -> Vec<Value> {
    page["data"].as_array().unwrap().iter().map(|row| json!([row["volume"], row["chapter"]])).collect()
}
fn minimal_row(row: &Value) {
    let actual = row.as_object().unwrap().keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected: BTreeSet<_> = ["id", "volume", "chapter", "language", "publishedAt", "version", "groupIds", "sharedGroupScore", "witness"].into_iter().collect();
    assert_eq!(actual, expected, "No expanded pages, metadata, names or local history");
    assert_eq!(row["witness"].as_str().unwrap().len(), 64);
    assert!(row["version"].as_i64().unwrap() > 0);
}
fn stale(page: &Value) {
    assert_eq!(page["state"], "stale-anchor", "{page}");
    assert_eq!(page["restartRequired"], true);
    assert!(page["data"].as_array().unwrap().is_empty());
    assert!(page["nextCursor"].is_null());
}
fn cursor(page: &Value) -> String { page["nextCursor"].as_str().expect("Expected another page").to_owned() }
fn public_cursor(page: &Value, forbidden: &[&str]) {
    let encoded = cursor(page);
    let decoded = String::from_utf8(URL_SAFE_NO_PAD.decode(&encoded).unwrap()).unwrap();
    let value: Value = serde_json::from_str(&decoded).unwrap();
    PublicUuid::parse(value["anchor"].as_str().unwrap()).unwrap();
    for native in forbidden {
        assert!(!decoded.contains(native), "Leaked native identity in {decoded}");
        assert!(!page.to_string().contains(native), "Leaked native identity in public response");
    }
    assert_eq!(value.as_object().unwrap().len(), 6);
    assert_eq!(URL_SAFE_NO_PAD.encode(decoded.as_bytes()), encoded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn actual_wire_order_preserves_profile_labels_native_js_indices_and_public_native_ties() {
    let f = Fixture::new("Order").await;
    f.manga("RSOrderLabels", "safe").await;
    f.manga("RSOrderChapterLabels", "safe").await;
    let input = [
        Some("special"), None, Some(""), Some("2a"), Some("02"), Some("2"), Some("2.0"),
        Some("9007199254740992"), Some("9007199254740993"),
        Some("2.0000000000000000000000000001"), Some("2.0000000000000000000000000002"),
        Some("\u{10000}"), Some("\u{e000}"), Some("😀"), Some("\u{ffff}"),
        Some("2\n"), Some("2\r"), Some("2\u{2028}"), Some("2\u{2029}"),
    ];
    for (index, label) in input.iter().enumerate() {
        // Exercise both volume and chapter expressions, including null vs empty.
        f.chapter(&format!("RSOrderLabel{index:02}"), "RSOrderLabels", *label, *label).await;
        f.chapter(&format!("RSOrderChapterLabel{index:02}"), "RSOrderChapterLabels", Some("1"), *label).await;
    }
    let aggregate = f.get("/api/manga/RSOrderLabels/aggregate", 200).await;
    let native = f.get("/api/manga/RSOrderLabels/reader?limit=100", 200).await;
    let old_labels = aggregate["chapters"].as_array().unwrap().iter()
        .map(|row| json!([row["volume"], row["chapter"]])).collect::<Vec<_>>();
    assert_eq!(labels(&native), old_labels, "Native retains actual aggregate database collation");
    let native_rows = native["data"].as_array().unwrap();
    let null_position = native_rows.iter().position(|row| row["volume"].is_null()).unwrap();
    for prefix in ["2a", "2\n", "2\r", "2\u{2028}", "2\u{2029}"] {
        assert!(native_rows.iter().position(|row| row["volume"] == prefix).unwrap() < null_position,
            "Native numeric-prefix classification includes terminal line separators");
    }
    let expected_numeric = [
        "9007199254740993", "9007199254740992", "2.0000000000000000000000000002",
        "2.0000000000000000000000000001",
    ];
    for (row, expected) in native["data"].as_array().unwrap().iter().zip(expected_numeric) {
        assert_eq!(row["volume"], expected);
        assert_eq!(row["chapter"], expected);
    }
    let public_path = f.reader_path(true, "RSOrderLabels").await;
    let public = f.get(&query(&public_path, &[("limit", "100")]), 200).await;
    let expected = [
        Some("9007199254740993"), Some("9007199254740992"),
        Some("2.0000000000000000000000000002"), Some("2.0000000000000000000000000001"),
        Some("2.0"), Some("2"), Some("02"), Some("\u{ffff}"), Some("\u{e000}"),
        Some("😀"), Some("\u{10000}"), Some("special"), Some("2\u{2029}"), Some("2\u{2028}"), Some("2a"),
        Some("2\r"), Some("2\n"), Some(""), None,
    ].into_iter().map(|label| json!([label, label])).collect::<Vec<_>>();
    assert_eq!(labels(&public), expected, "Exact NUMERIC and UTF-16, not f64 or Unicode scalar order");
    // A fixed volume forces the independent chapter terms to do the sorting.
    let chapter_aggregate = f.get("/api/manga/RSOrderChapterLabels/aggregate", 200).await;
    let chapter_native = f.get("/api/manga/RSOrderChapterLabels/reader?limit=100", 200).await;
    assert_eq!(labels(&chapter_native), chapter_aggregate["chapters"].as_array().unwrap().iter()
        .map(|row| json!([row["volume"], row["chapter"]])).collect::<Vec<_>>());
    let chapter_path = f.reader_path(true, "RSOrderChapterLabels").await;
    let chapter_public = f.get(&query(&chapter_path, &[("limit", "100")]), 200).await;
    assert_eq!(labels(&chapter_public), expected.iter().map(|pair| json!(["1", pair[1]])).collect::<Vec<_>>());
    let oldest = f.get(&query(&public_path, &[("limit", "100"), ("direction", "oldest")]), 200).await;
    assert_eq!(labels(&oldest), expected.into_iter().rev().collect::<Vec<_>>());

    f.manga("RSOrderEditions", "safe").await;
    for id in ["2", "10", "02", "4294967294", "4294967295", "RSOrderEditionZ"] {
        f.chapter(id, "RSOrderEditions", Some("1"), Some("1")).await;
    }
    f.sql("UPDATE platform.catalog_chapters SET published_at='2025-01-01' WHERE id='RSOrderEditionZ'").await;
    let wire = f.wire("/api/manga/RSOrderEditions/aggregate", 200).await;
    let expected_wire_keys = ["02", "10", "2", "4294967294", "4294967295", "RSOrderEditionZ"];
    let positions = expected_wire_keys.iter().map(|id| wire.find(&format!("\"{id}\":{{")).unwrap()).collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "Assert serialized object order before parsing JSON");
    let aggregate: Value = serde_json::from_str(&wire).unwrap();
    let btree_keys = aggregate["chapters"][0]["entries"].as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(btree_keys, expected_wire_keys);
    let editions = f.get("/api/manga/RSOrderEditions/reader?operation=editions&groupChapter=2&limit=100", 200).await;
    // ECMAScript Object.entries enumerates canonical array indices first, in
    // numeric order. 02 and 2^32-1 remain ordinary serde BTreeMap string keys.
    assert_eq!(ids(&editions), ["2", "10", "4294967294", "02", "4294967295", "RSOrderEditionZ"]);
    assert_eq!(editions["winner"]["id"], "2");
    assert_eq!(editions["hasMultipleEditions"], true);

    f.manga("RSOrderTies", "safe").await;
    // Pre-register synthetic preferred aliases before chapter insertion. Alias
    // order is intentionally the opposite of the true native-ID tie order.
    f.sql("INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin) VALUES ('chapter','ffffffff-ffff-8fff-bfff-fffffffffff1','RSOrderTieA',true,'native'),('chapter','00000000-0000-8000-8000-000000000001','RSOrderTieB',true,'native')").await;
    f.chapter("RSOrderTieA", "RSOrderTies", Some("1"), Some("1")).await;
    f.chapter("RSOrderTieB", "RSOrderTies", Some("1"), Some("1")).await;
    let a = f.reference(true, ResourceKind::Chapter, "RSOrderTieA").await;
    let b = f.reference(true, ResourceKind::Chapter, "RSOrderTieB").await;
    assert!(a > b);
    let path = f.reader_path(true, "RSOrderTies").await;
    let editions = f.get(&query(&path, &[("operation", "editions"), ("groupChapter", &b)]), 200).await;
    assert_eq!(ids(&editions), [a.clone(), b]);
    assert_eq!(editions["winner"]["id"], a);
    let parent = f.reference(true, ResourceKind::Manga, "RSOrderTies").await;
    let retained = f.get(&query("/api/v1/chapter", &[("manga", &parent), ("includeEmptyPages", "1")]), 200).await;
    assert_eq!(ids(&editions), ids(&retained), "Actual public collection insertion order is retained");
}

// Literal public order extends the unchanged retained-reader goldens above and
// tests/ci/test_bounded_reader_order_goldens.mjs, not reader_order's SQL or a
// replacement comparator. Signs are raw labels, not accepted decimal syntax.
const KEYSET_LABEL_INPUT: [Option<&str>; 26] = [
    Some("special"), None, Some(""), Some("2a"), Some("02"), Some("2"), Some("2.0"),
    Some("9007199254740992"), Some("9007199254740993"),
    Some("2.0000000000000000000000000001"), Some("2.0000000000000000000000000002"),
    Some("\u{10000}"), Some("\u{e000}"), Some("😀"), Some("\u{ffff}"),
    Some("2\n"), Some("2\r"), Some("2\u{2028}"), Some("2\u{2029}"), Some("1"),
    Some("+2"), Some("-2"), Some("+02"), Some("-02"), Some("+2.0"), Some("-2.0"),
];
const KEYSET_PUBLIC_NEWEST: [Option<&str>; 26] = [
    Some("9007199254740993"), Some("9007199254740992"),
    Some("2.0000000000000000000000000002"), Some("2.0000000000000000000000000001"),
    Some("2.0"), Some("2"), Some("02"), Some("1"),
    Some("\u{ffff}"), Some("\u{e000}"), Some("😀"), Some("\u{10000}"), Some("special"),
    Some("2\u{2029}"), Some("2\u{2028}"), Some("2a"), Some("2\r"), Some("2\n"),
    Some("-2.0"), Some("-2"), Some("-02"), Some("+2.0"), Some("+2"), Some("+02"),
    Some(""), None,
];

async fn assert_special_label_keysets(name: &str, chapter_axis: bool) {
    let f = Fixture::new(name).await;
    let manga = format!("RS{name}Manga");
    f.manga(&manga, "safe").await;
    let position = |label: Option<&str>| {
        if chapter_axis { json!(["1", label]) } else { json!([label, "1"]) }
    };
    let native_id = |index: usize| format!("RS{name}Label{index:02}");
    for (index, label) in KEYSET_LABEL_INPUT.iter().copied().enumerate() {
        let (volume, chapter) = if chapter_axis { (Some("1"), label) } else { (label, Some("1")) };
        f.chapter(&native_id(index), &manga, volume, chapter).await;
    }

    // Native's oracle is the real unchanged aggregate route, captured once
    // before traversal. Its database collation must NOT be replaced with the
    // public UTF-16 literal, a Rust sort, or the new selector's full-page output.
    let retained = f.get(&format!("/api/manga/{manga}/aggregate"), 200).await;
    let native_golden = retained["chapters"].as_array().unwrap().iter().map(|row| {
        let entries = row["entries"].as_object().unwrap();
        assert_eq!(entries.len(), 1, "One edition per exact nullable label pair");
        (entries.keys().next().unwrap().clone(), json!([row["volume"], row["chapter"]]))
    }).collect::<Vec<_>>();
    let mut public_golden = Vec::new();
    for label in KEYSET_PUBLIC_NEWEST {
        let index = KEYSET_LABEL_INPUT.iter().position(|input| *input == label).unwrap();
        let id = f.reference(true, ResourceKind::Chapter, &native_id(index)).await;
        public_golden.push((id, position(label)));
    }
    let expected_positions = KEYSET_LABEL_INPUT.into_iter().map(|label| position(label).to_string()).collect::<BTreeSet<_>>();
    let expected_native_ids = (0..KEYSET_LABEL_INPUT.len()).map(native_id).collect::<BTreeSet<_>>();
    for golden in [&native_golden, &public_golden] {
        assert_eq!(golden.len(), KEYSET_LABEL_INPUT.len());
        assert_eq!(golden.iter().map(|(_, labels)| labels.to_string()).collect::<BTreeSet<_>>(), expected_positions);
    }
    assert_eq!(native_golden.iter().map(|(id, _)| id.clone()).collect::<BTreeSet<_>>(), expected_native_ids);
    let native_path = f.reader_path(false, &manga).await;
    let public_path = f.reader_path(true, &manga).await;

    // No fixture writes follow. Exact once-only traversal is asserted only for
    // this unchanged fixture; get() still requires crossRequestSnapshot=false.
    for (public, path, golden) in [
        (false, native_path, native_golden), (true, public_path, public_golden),
    ] {
        let locate = |label| golden.iter().position(|(_, pair)| *pair == position(label)).unwrap();
        if public {
            assert!(locate(Some("1")) < locate(Some("2a")), "Public accepts only complete unsigned decimals");
            assert_eq!(locate(None), golden.len() - 1, "Public null-last retained order");
        } else {
            assert!(locate(Some("2a")) < locate(Some("1")), "Native retains numeric-prefix classification");
            assert!(locate(Some("1")) < locate(None));
            for special in [Some(""), Some("\u{10000}"), Some("\u{e000}"), Some("😀"), Some("\u{ffff}"),
                Some("+2"), Some("-2"), Some("+02"), Some("-02"), Some("+2.0"), Some("-2.0")] {
                assert!(locate(None) < locate(special), "Native null precedes raw nonnumeric labels");
            }
        }
        for (direction, reverse) in [("newest", false), ("oldest", true)] {
            let mut expected = golden.clone();
            if reverse { expected.reverse(); }
            let expected_ids = expected.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
            let expected_labels = expected.iter().map(|(_, labels)| labels.clone()).collect::<Vec<_>>();
            // A one-row page puts every adjacent pair across a real cursor.
            // Changing widths additionally exercises equal-prefix anchors
            // when the next request changes limit without changing its scope.
            for widths in [&[1usize][..], &[2usize, 5, 3][..]] {
                let mut after = None::<String>;
                let mut scope = None::<Value>;
                let mut seen_ids = BTreeSet::new();
                let mut seen_labels = BTreeSet::new();
                let mut traversed = Vec::new();
                let mut offset = 0;
                let mut pages = 0;
                while offset < expected.len() {
                    let width = widths[pages % widths.len()];
                    let limit = width.to_string();
                    let mut pairs = vec![("limit", limit.as_str()), ("direction", direction)];
                    if let Some(after) = &after { pairs.push(("after", after.as_str())); }
                    let page = f.get(&query(&path, &pairs), 200).await;
                    assert_eq!(page["state"], "ready", "{path}, {direction}, offset {offset}");
                    assert_eq!(page["restartRequired"], false);
                    assert_eq!(page["orderingProfile"], if public { "public-retained-v1" } else { "native-aggregate-v1" });
                    assert_eq!(page["limit"], json!(width));
                    if let Some(scope) = &scope { assert_eq!(&page["scope"], scope); }
                    else { scope = Some(page["scope"].clone()); }
                    let end = (offset + width).min(expected.len());
                    let page_ids = ids(&page);
                    let page_labels = labels(&page);
                    assert_eq!(page_ids, expected_ids[offset..end], "Exact ID slice at {direction} cursor boundary {offset}");
                    assert_eq!(page_labels, expected_labels[offset..end], "Exact raw/null label slice at {direction} cursor boundary {offset}");
                    for id in &page_ids { assert!(seen_ids.insert(id.clone()), "Duplicate ID across pages"); }
                    for labels in page_labels { assert!(seen_labels.insert(labels.to_string()), "Duplicate exact label pair across pages"); }
                    traversed.extend(page_ids);
                    offset = end;
                    pages += 1;
                    assert!(pages <= expected.len(), "Cursor must advance on every page");
                    if offset == expected.len() {
                        assert!(page["nextCursor"].is_null(), "No phantom continuation at the exact retained boundary");
                    } else {
                        let encoded = cursor(&page);
                        let decoded: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(&encoded).unwrap()).unwrap();
                        let last = page["data"].as_array().unwrap().last().unwrap();
                        assert_eq!(decoded["anchor"], last["id"], "Cursor anchors the last emitted row, not the lookahead");
                        assert_eq!(decoded["witness"], last["witness"]);
                        assert_eq!(decoded["scope"], page["scope"]);
                        if public { public_cursor(&page, &[&manga, &format!("RS{name}Label"), &f.user]); }
                        after = Some(encoded);
                    }
                }
                assert_eq!(traversed, expected_ids, "No omissions or reorderings over an unchanged fixture");
                assert_eq!(seen_ids.len(), KEYSET_LABEL_INPUT.len());
                assert_eq!(seen_labels, expected_positions, "Equivalent numeric values, signed/raw forms, empty and null remain distinct");
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn actual_sql_keysets_preserve_nullable_unicode_decimal_and_raw_volume_goldens_in_both_directions() {
    assert_special_label_keysets("KeysetVolume", false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn actual_sql_keysets_preserve_nullable_unicode_decimal_and_raw_chapter_goldens_in_both_directions() {
    assert_special_label_keysets("KeysetChapter", true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn bounded_pages_cross_ten_thousand_and_winners_examine_all_editions_without_uncapping_aggregates() {
    let f = Fixture::new("Large").await;
    f.manga("RSLargeGroups", "safe").await;
    f.manga("RSLargeEditions", "safe").await;
    f.group("RSLargePreferredGroup").await;
    f.sql(r#"
      INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,volume,chapter,language,published_at,created_at,updated_at)
      SELECT 'RSLargeG'||lpad(i::text,5,'0'),'RSLargeGroups','RSLargeUser','Reader fixture','1',i::text,'en','2024-01-01','2024-01-01','2024-01-01' FROM generate_series(1,10005) s(i);
      INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,volume,chapter,language,published_at,created_at,updated_at)
      SELECT 'RSLargeE'||lpad(i::text,5,'0'),'RSLargeEditions','RSLargeUser','Reader fixture','1','1','en','2024-01-01','2024-01-01','2024-01-01' FROM generate_series(1,10005) s(i);
    "#).await;
    f.chapter("RSLargeCurrent", "RSLargeEditions", Some("1"), Some("0")).await;
    f.sql("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('RSLargeCurrent','RSLargePreferredGroup'),('RSLargeE10005','RSLargePreferredGroup')").await;
    for public in [false, true] {
        let path = f.reader_path(public, "RSLargeGroups").await;
        let mut after = None::<String>;
        let mut seen = BTreeSet::new();
        let mut expected = 10005;
        let mut pages = 0;
        loop {
            let mut pairs = vec![("limit", "100")];
            if let Some(after) = &after { pairs.push(("after", after)); }
            let page = f.get(&query(&path, &pairs), 200).await;
            assert_eq!(page["state"], "ready");
            assert_eq!(page["restartRequired"], false);
            for row in page["data"].as_array().unwrap() {
                assert_eq!(row["chapter"], expected.to_string());
                assert!(seen.insert(row["id"].as_str().unwrap().to_owned()), "Repeated page row");
                expected -= 1;
            }
            pages += 1;
            if page["nextCursor"].is_null() { break; }
            if public { public_cursor(&page, &["RSLargeG", "RSLargeGroups", "RSLargeUser"]); }
            after = Some(cursor(&page));
            assert!(pages <= 101, "Non-advancing cursor");
        }
        assert_eq!(seen.len(), 10005);
        assert_eq!(expected, 0);
        assert_eq!(pages, 101);
        let first = f.get(&query(&path, &[("operation", "first")]), 200).await;
        let last = f.get(&query(&path, &[("operation", "last")]), 200).await;
        assert_eq!(first["data"][0]["chapter"], "1");
        assert_eq!(last["data"][0]["chapter"], "10005");
        let path = f.reader_path(public, "RSLargeEditions").await;
        let current = f.reference(public, ResourceKind::Chapter, "RSLargeCurrent").await;
        let group = f.reference(public, ResourceKind::Chapter, "RSLargeE00001").await;
        let winner = f.reference(public, ResourceKind::Chapter, "RSLargeE10005").await;
        let request = [("operation", "editions"), ("current", current.as_str()), ("groupChapter", group.as_str()), ("limit", "100")];
        let page = f.get(&query(&path, &request), 200).await;
        assert_eq!(page["data"].as_array().unwrap().len(), 100);
        assert!(!ids(&page).contains(&winner), "Winner is beyond 10K, not in this page");
        assert_eq!(page["winner"]["id"], winner);
        assert_eq!(page["winner"]["sharedGroupScore"], 1);
        assert_eq!(page["hasMultipleEditions"], true);
        let after = cursor(&page);
        let mut request = request.to_vec(); request.push(("after", &after));
        let next = f.get(&query(&path, &request), 200).await;
        assert_eq!(next["data"].as_array().unwrap().len(), 100);
        assert!(ids(&next).iter().all(|id| !ids(&page).contains(id)));
        assert_eq!(next["data"][0]["id"], f.reference(public, ResourceKind::Chapter, "RSLargeE00101").await);
        assert_eq!(next["winner"]["id"], winner);
        let resolved = f.get(&query(&path, &[("operation", "resolve-group"), ("current", &current), ("groupChapter", &group)]), 200).await;
        assert_eq!(resolved["data"][0]["id"], winner);
        assert_eq!(resolved["winner"]["id"], winner);
        assert_eq!(resolved["hasMultipleEditions"], true);
        let first = f.get(&query(&path, &[("operation", "first"), ("current", &current)]), 200).await;
        let last = f.get(&query(&path, &[("operation", "last"), ("current", &current)]), 200).await;
        assert_eq!(first["data"][0]["id"], current);
        assert_eq!(first["hasMultipleEditions"], false);
        assert_eq!(last["data"][0]["id"], winner);
        assert_eq!(last["hasMultipleEditions"], true);
        for manga in ["RSLargeGroups", "RSLargeEditions"] {
            let parent = f.reference(public, ResourceKind::Manga, manga).await;
            let legacy = format!("{}/manga/{parent}/aggregate", if public { "/api/v1" } else { "/api" });
            let failed = f.get(&legacy, if public { 503 } else { 400 }).await;
            assert!(failed.get("data").is_none(), "Oversize aggregate must not quietly become a truncated success");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn live_cursors_accept_unrelated_insertions_but_restart_for_changed_anchor_witness_or_scope() {
    let f = Fixture::new("Cursor").await;
    f.group("RSCursorMembership").await;
    for public in [false, true] {
        let suffix = if public { "P" } else { "N" };
        let manga = format!("RSCursor{suffix}");
        f.manga(&manga, "safe").await;
        for number in 1..=3 {
            f.chapter(&format!("RSCursor{suffix}{number}"), &manga, Some("1"), Some(&number.to_string())).await;
        }
        let path = f.reader_path(public, &manga).await;
        let filter = if public { "translatedLanguage[]" } else { "tlang" };
        let start = || query(&path, &[("limit", "1"), (filter, "en")]);
        let page = f.get(&start(), 200).await;
        let after = cursor(&page);
        if public { public_cursor(&page, &["RSCursorP", "RSCursorUser", "RSCursorMembership"]); }
        f.chapter(&format!("RSCursor{suffix}Inserted"), &manga, Some("1"), Some("2.5")).await;
        let continued = f.get(&query(&path, &[("limit", "100"), (filter, "en"), ("after", &after)]), 200).await;
        assert_eq!(continued["state"], "ready");
        assert_eq!(continued["data"][0]["chapter"], "2.5", "A live cursor is not a cross-request snapshot");
        let id = format!("RSCursor{suffix}3");
        let mutations = [
            (format!("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('{id}','RSCursorMembership')"),
             format!("DELETE FROM platform.catalog_chapter_groups WHERE chapter_id='{id}'")),
            (format!("UPDATE platform.catalog_chapters SET chapter='3.25' WHERE id='{id}'"),
             format!("UPDATE platform.catalog_chapters SET chapter='3' WHERE id='{id}'")),
            (format!("UPDATE platform.catalog_chapters SET deleted_at=clock_timestamp() WHERE id='{id}'"),
             format!("UPDATE platform.catalog_chapters SET deleted_at=NULL WHERE id='{id}'")),
            (format!("UPDATE platform.catalog_chapters SET language='fr' WHERE id='{id}'"),
             format!("UPDATE platform.catalog_chapters SET language='en' WHERE id='{id}'")),
            (format!("UPDATE platform.catalog_chapters SET published_at='2024-01-01 00:00:00.000001+00' WHERE id='{id}'"),
             format!("UPDATE platform.catalog_chapters SET published_at='2024-01-01' WHERE id='{id}'")),
        ];
        for (change, undo) in mutations {
            let before = f.get(&start(), 200).await;
            let after = cursor(&before);
            f.sql(&change).await;
            let changed = f.get(&query(&path, &[("limit", "1"), (filter, "en"), ("after", &after)]), 200).await;
            stale(&changed);
            let restarted = f.get(&start(), 200).await;
            assert_eq!(restarted["state"], "ready");
            f.sql(&undo).await;
        }
        let before = f.get(&start(), 200).await;
        let after = cursor(&before);
        let changed = f.get(&query(&path, &[("limit", "1"), (filter, "en"), ("direction", "oldest"), ("after", &after)]), 200).await;
        stale(&changed);
        let changed = f.get(&query(&path, &[("limit", "1"), (filter, "fr"), ("after", &after)]), 200).await;
        stale(&changed);

        // An insertion into the actual current edition's group membership is
        // also a selection change even when the emitted anchor is untouched.
        let current_native = format!("RSCursor{suffix}1");
        let current = f.reference(public, ResourceKind::Chapter, &current_native).await;
        let page = f.get(&query(&path, &[("limit", "1"), (filter, "en"), ("current", &current)]), 200).await;
        let after = cursor(&page);
        f.sql(&format!("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('{current_native}','RSCursorMembership')")).await;
        stale(&f.get(&query(&path, &[("limit", "1"), (filter, "en"), ("current", &current), ("after", &after)]), 200).await);
        if public {
            let page = f.get(&start(), 200).await;
            let after = cursor(&page);
            f.sql(&format!("UPDATE platform.catalog_chapters SET is_unavailable=true WHERE id='{id}'")).await;
            stale(&f.get(&query(&path, &[("limit", "1"), (filter, "en"), ("after", &after)]), 200).await);
            f.sql(&format!("UPDATE platform.catalog_chapters SET is_unavailable=false WHERE id='{id}'")).await;
        }

        // Editions bind the selected group witness separately from the last
        // emitted edition. Moving that selection cannot silently change groups.
        let extra = format!("RSCursor{suffix}EditionZ");
        f.chapter(&extra, &manga, Some("1"), Some("1")).await;
        let group = f.reference(public, ResourceKind::Chapter, &extra).await;
        let page = f.get(&query(&path, &[("operation", "editions"), ("groupChapter", &group), ("limit", "1")]), 200).await;
        let after = cursor(&page);
        f.sql(&format!("UPDATE platform.catalog_chapters SET chapter='0.5' WHERE id='{extra}'")).await;
        stale(&f.get(&query(&path, &[("operation", "editions"), ("groupChapter", &group), ("limit", "1"), ("after", &after)]), 200).await);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn semantic_navigation_recomputes_neighbors_and_excluded_current_does_not_break_direct_reads() {
    let f = Fixture::new("Live").await;
    for public in [false, true] {
        let suffix = if public { "P" } else { "N" };
        let manga = format!("RSLive{suffix}");
        f.manga(&manga, "safe").await;
        for number in 1..=3 {
            f.chapter(&format!("RSLive{suffix}{number}"), &manga, Some("1"), Some(&number.to_string())).await;
        }
        let actual = format!("RSLive{suffix}2Z");
        f.chapter(&actual, &manga, Some("1"), Some("2")).await;
        let current = f.reference(public, ResourceKind::Chapter, &actual).await;
        let path = f.reader_path(public, &manga).await;
        let around = f.get(&query(&path, &[("operation", "around"), ("current", &current)]), 200).await;
        assert_eq!(around["current"]["id"], current);
        assert_ne!(around["data"][0]["id"], current, "Current is the actual edition, not the group's winner");
        assert_eq!(around["next"]["chapter"], "3");
        assert_eq!(around["previous"]["chapter"], "1");
        let next = f.get(&query(&path, &[("operation", "next"), ("current", &current)]), 200).await;
        assert_eq!(next["data"][0]["chapter"], "3");
        f.chapter(&format!("RSLive{suffix}Inserted"), &manga, Some("1"), Some("2.5")).await;
        let next = f.get(&query(&path, &[("operation", "next"), ("current", &current)]), 200).await;
        assert_eq!(next["data"][0]["chapter"], "2.5", "Re-resolve after insertion, never reuse cached neighbor IDs");
        f.sql(&format!("UPDATE platform.catalog_chapters SET chapter='4',version=version+1 WHERE id='{actual}'")).await;
        let next = f.get(&query(&path, &[("operation", "next"), ("current", &current)]), 200).await;
        assert_eq!(next["state"], "boundary");
        assert_eq!(next["current"]["chapter"], "4");
        assert_eq!(next["restartRequired"], false);
        let previous = f.get(&query(&path, &[("operation", "previous"), ("current", &current)]), 200).await;
        assert_eq!(previous["data"][0]["chapter"], "3");
        let filter = if public { "translatedLanguage[]" } else { "tlang" };
        let excluded = f.get(&query(&path, &[("operation", "around"), ("current", &current), (filter, "fr")]), 200).await;
        assert_eq!(excluded["state"], "excluded-current");
        assert_eq!(excluded["restartRequired"], false);
        assert!(excluded["data"].as_array().unwrap().is_empty());
        assert!(excluded.get("current").is_none());
        let direct = format!("{}/chapter/{current}", if public { "/api/v1" } else { "/api" });
        let direct = f.get(&direct, 200).await;
        assert_eq!(if public { &direct["data"]["id"] } else { &direct["id"] }, &json!(current));
        let again = f.get(&query(&path, &[("operation", "around"), ("current", &current)]), 200).await;
        assert_eq!(again["state"], "ready");
        assert_eq!(again["current"]["id"], current);
        assert_eq!(again["previous"]["chapter"], "3");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires explicitly isolated PostgreSQL runner; source-authored, not execution evidence"]
async fn profile_defaults_filter_controls_and_rejected_device_history_remain_strict() {
    let f = Fixture::new("Control").await;
    f.manga("RSControlManga", "pornographic").await;
    f.group("RSControlGroup").await;
    for (id, chapter) in [("RSControlEmpty", "1"), ("RSControlFuture", "2"), ("RSControlExternal", "3"), ("RSControlUnavailable", "4")] {
        f.chapter(id, "RSControlManga", Some("1"), Some(chapter)).await;
    }
    f.sql(r#"
      UPDATE platform.catalog_chapters SET published_at='2999-01-01',updated_at='2999-01-01' WHERE id='RSControlFuture';
      UPDATE platform.catalog_chapters SET metadata='{"externalUrl":"https://external.fixture.invalid/reader","data":[]}' WHERE id='RSControlExternal';
      UPDATE platform.catalog_chapters SET is_unavailable=true WHERE id='RSControlUnavailable';
      INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES('RSControlEmpty','RSControlGroup');
    "#).await;
    for public in [false, true] {
        let path = f.reader_path(public, "RSControlManga").await;
        let page = f.get(&path, 200).await;
        assert_eq!(page["operation"], "groups");
        assert_eq!(page["limit"], 50);
        assert_eq!(page["data"].as_array().unwrap().len(), if public { 3 } else { 4 });
        assert_eq!(page["data"][0]["chapter"], if public { "3" } else { "4" });
        assert_eq!(page["orderingProfile"], if public { "public-retained-v1" } else { "native-aggregate-v1" });
        for limit in ["1", "100"] {
            let bounded = f.get(&query(&path, &[("limit", limit)]), 200).await;
            assert!(bounded["data"].as_array().unwrap().len() <= limit.parse::<usize>().unwrap());
        }
        let current = f.reference(public, ResourceKind::Chapter, "RSControlEmpty").await;
        let group = f.reference(public, ResourceKind::Group, "RSControlGroup").await;
        let group_filter = if public { "groups[]" } else { "group" };
        let filtered = f.get(&query(&path, &[("operation", "editions"), ("groupChapter", &current), (group_filter, &group)]), 200).await;
        assert_eq!(ids(&filtered), [current.clone()]);
        assert_eq!(filtered["hasMultipleEditions"], false);
        assert_eq!(filtered["winner"]["id"], current);
        for invalid in [
            "limit=0", "limit=101", "limit=-1", "limit=1&limit=2", "direction=sideways",
            "operation=unknown", "operation=next", "operation=previous", "operation=around",
            "operation=editions", "operation=resolve-group", "operation=first&direction=newest",
            "history=anything", "history[]=anything", "completedIds[]=anything", "readIds[]=anything",
            "lastRead=anything", "localHistory=anything", "offset=1", "page=2",
            "after=", "after=not-a-cursor", "current=", "title=%FF", "title=%Q1", "title=%00",
            "limit=1&&limit=2", "order[chapter]=asc", "includes[]=manga",
        ] { f.get(&format!("{path}?{invalid}"), 400).await; }
        f.get(&query(&path, &[("operation", "groups"), ("groupChapter", &current)]), 400).await;
        f.get(&query(&path, &[("operation", "next"), ("current", &current), ("direction", "oldest")]), 400).await;
        let paged = f.get(&query(&path, &[("limit", "1")]), 200).await;
        let after = cursor(&paged);
        f.get(&query(&path, &[("operation", "next"), ("current", &current), ("after", &after)]), 400).await;
        if public {
            let all = f.get(&query(&path, &[("includeUnavailable", "1")]), 200).await;
            assert_eq!(all["data"].as_array().unwrap().len(), 4);
            let only_safe = f.get(&query(&path, &[("contentRating[]", "safe")]), 200).await;
            assert_eq!(only_safe["state"], "boundary");
            let no_future = f.get(&query(&path, &[("includeFuturePublishAt", "0")]), 200).await;
            assert!(no_future["data"].as_array().unwrap().iter().all(|row| row["chapter"] != "2"));
            let no_external = f.get(&query(&path, &[("includeExternalUrl", "0")]), 200).await;
            assert!(no_external["data"].as_array().unwrap().iter().all(|row| row["chapter"] != "3"));
            let no_empty = f.get(&query(&path, &[("includeEmptyPages", "0")]), 200).await;
            assert_eq!(no_empty["state"], "boundary");
            for invalid in ["includeUnavailable=true", "includeEmptyPages=2", "group=RSControlGroup", "groups[]=RSControlGroup"] {
                f.get(&format!("{path}?{invalid}"), 400).await;
            }
            public_cursor(&paged, &["RSControl", "pornographic"]);
        } else {
            f.get(&query(&path, &[("includeUnavailable", "1")]), 400).await;
        }
    }
}

// This scenario deliberately changes the disposable database's global identity.
// Run ALONE with its exact test filter, never alongside other database fixtures.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires a separately admitted isolated identity fixture, not a parallel suite"]
async fn catalogue_replacement_with_matching_ids_changes_every_state_identity_and_rejects_old_cursor() {
    assert_eq!(std::env::var("PLATFORM_READER_IDENTITY_ISOLATED").as_deref(),Ok("1"));
    let f=Fixture::new("Identity").await;
    f.manga("RSIdentityManga","safe").await;
    for number in 1..=3 {
        f.chapter(&format!("RSIdentityChapter{number}"),"RSIdentityManga",Some("1"),Some(&number.to_string())).await;
    }
    let capabilities=f.get("/api/protocol/capabilities",200).await;
    let original=capabilities["catalog_namespace"].as_str().unwrap().to_owned();
    let expected=json!({"catalogNamespace":original,"serviceKey":capabilities["service_key"]});
    assert!(capabilities["service_key"].is_null(),"Native-only fixture keeps the capability null key");
    let mut contexts=Vec::new();
    for public in [false,true] {
        let path=f.reader_path(public,"RSIdentityManga").await;
        let page=f.get(&query(&path,&[("limit","1")]),200).await;
        assert_eq!(page["catalogIdentity"],expected);
        let current=f.reference(public,ResourceKind::Chapter,"RSIdentityChapter2").await;
        contexts.push((public,path,page,current));
    }
    let replacement="eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    assert_ne!(original,replacement);
    f.state.db.client().await.unwrap().execute(
        "UPDATE platform.catalogue_namespace SET namespace=$1::text::uuid WHERE singleton",&[&replacement],
    ).await.unwrap();
    for (public,path,before,current) in contexts {
        let expected=json!({"catalogNamespace":replacement,"serviceKey":null});
        let page=f.get(&query(&path,&[("limit","1")]),200).await;
        assert_eq!(ids(&page),ids(&before),"Matching resource IDs do not establish same catalogue identity");
        assert_eq!(page["catalogIdentity"],expected);
        assert_ne!(page["scope"],before["scope"]);
        let after=cursor(&before);
        let stale_page=f.get(&query(&path,&[("limit","1"),("after",&after)]),200).await;
        stale(&stale_page);
        assert_eq!(stale_page["catalogIdentity"],expected);
        let language=if public {"translatedLanguage[]"} else {"tlang"};
        let boundary=f.get(&query(&path,&[(language,"fr")]),200).await;
        assert_eq!(boundary["state"],"boundary");
        assert_eq!(boundary["catalogIdentity"],expected);
        let excluded=f.get(&query(&path,&[("operation","around"),("current",&current),(language,"fr")]),200).await;
        assert_eq!(excluded["state"],"excluded-current");
        assert_eq!(excluded["catalogIdentity"],expected);
        let semantic=f.get(&query(&path,&[("operation","next"),("current",&current)]),200).await;
        assert_eq!(semantic["state"],"ready");
        assert_eq!(semantic["catalogIdentity"],expected);
    }
    let capabilities=f.get("/api/protocol/capabilities",200).await;
    assert_eq!(capabilities["catalog_namespace"],replacement);
    f.state.db.client().await.unwrap().execute(
        "UPDATE platform.catalogue_namespace SET namespace=$1::text::uuid WHERE singleton",&[&original],
    ).await.unwrap();
}
