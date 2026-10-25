//! Graceful full-runtime teardown followed by a genuinely fresh recovery process.
//! The child uses persisted fixture state, never an inherited actor or reseeding.
use super::*;
use std::{path::PathBuf, process::Stdio};

const MARKER: &str = "owned-public-ta-restart-v1";
const SPEC_ENV: &str = "PLATFORM_PUBLIC_LANGUAGE_RESTART_SPEC";

fn fixture_dsn() -> String {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap();
    let config: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(config.get_dbname(), Some("weebdex_platform_test"));
    assert!(!config.get_hosts().is_empty());
    assert!(config.get_hosts().iter().all(|host| {
        matches!(host, tokio_postgres::config::Host::Tcp(host) if host == "127.0.0.1")
    }));
    dsn
}

async fn stop_native(listener: &mut Option<weebdex_platform::protocol::publication::NativeListener>) {
    if let Some(mut listener) = listener.take() {
        listener.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(20), listener.task)
            .await.unwrap().unwrap().unwrap();
    }
}

fn manifest_language(bytes: &[u8]) -> String {
    let value = manga_node::manifest::decode(bytes).unwrap();
    value.as_map().unwrap().iter()
        .find(|(key, _)| key.as_text() == Some("language"))
        .and_then(|(_, value)| value.as_text()).unwrap().to_owned()
}

async fn child_call(client: &reqwest::Client, origin: &str, key: Option<u8>, method: &str,
    path: &str, payload: Option<Value>, expected: u16) -> Value {
    let body = payload.map_or_else(Vec::new, |value| serde_json::to_vec(&value).unwrap());
    let url = format!("{origin}{path}");
    let mut request = client.request(method.parse().unwrap(), &url)
        .header("origin", origin).header("content-type", "application/json");
    if let Some(key) = key { request = request.header("authorization", proof(key, 11, &url, method, &body)); }
    let response = request.body(body).send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    assert_eq!(status, expected, "fresh-process {method} {path}: {text}");
    serde_json::from_str(&text).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires owned isolated PostgreSQL and verified pinned strfry"]
async fn public_ta_pending_publication_survives_fresh_process_recovery() {
    assert!(std::env::var_os(SPEC_ENV).is_none(), "Parent probe cannot run as its own recovery child");
    let dsn = fixture_dsn();
    let mut f = fixture().await;
    f.http = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
    // Source acceptance stays real, while its actual projector is deliberately stopped.
    stop_native(&mut f.native).await;
    let manga = alias(&f, ResourceKind::Manga, FIXTURE_MANGA_ID).await;
    let session = f.call(Some(1), "POST", "/api/v1/upload/begin",
        Some(json!({"manga":manga,"groups":[]})), 200).await;
    let session = session["id"].as_str().unwrap().to_owned();
    let file = attach(&f, &session).await;
    let payload = json!({"chapterDraft":chapter_draft("ta"),"pageOrder":[file],"termsAccepted":true});
    let commit_path = format!("/api/v1/upload/{session}/commit");
    let receipt = f.call(Some(1), "POST", &commit_path, Some(payload.clone()), 200).await;
    assert_eq!(receipt["attributes"]["translatedLanguage"], "ta");
    assert_eq!(receipt["attributes"]["nostr"]["publicationStatus"], "accepted_pending_projection");
    assert_eq!(receipt["attributes"]["isUnavailable"], true);
    let public_chapter = receipt["id"].as_str().unwrap().to_owned();
    let event = receipt["attributes"]["nostr"]["eventId"].as_str().unwrap().to_owned();
    let native_session = f.native(ResourceKind::UploadSession, &session).await;
    let native_chapter = f.native(ResourceKind::Chapter, &public_chapter).await;
    let db = f.state.db.client().await.unwrap();
    let row = db.query_one("SELECT p.state,p.source_manifest,p.native_commit_payload,p.native_commit_hash,e.raw_event,c.language,s.committed_result,s.version FROM platform.protocol_publication_commands p JOIN platform.upload_sessions s ON s.id=p.upload_session_id JOIN platform.catalog_chapters c ON c.id=s.committed_chapter_id JOIN public.events e ON e.id=decode(p.event_id,'hex') WHERE p.upload_session_id=$1 AND p.event_id=$2", &[&native_session, &event]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "materialized");
    let manifest: Vec<u8> = row.get(1);
    let native_payload: Value = row.get(2);
    let commit_hash: String = row.get(3);
    let raw_event: Value = row.get(4);
    assert_eq!(manifest_language(&manifest), "ta");
    assert_eq!(native_payload["draft"]["language"], "ta");
    assert_eq!(row.get::<_, String>(5), "ta");
    let stored: Value = row.get(6);
    assert_eq!(stored["language"], "ta");
    assert_eq!(stored["publication_status"], "accepted_pending_projection");
    let session_version: i32 = row.get(7);
    let signed: auth::Event = serde_json::from_value(raw_event.clone()).unwrap();
    signed.verify().unwrap();
    assert_eq!(signed.id, event);
    assert_eq!(signed.pubkey, public(1));
    assert_eq!(signed.kind, 1001);
    let pointer: Value = serde_json::from_str(&signed.content).unwrap();
    assert_eq!(pointer["manifest"], auth::hash(&manifest));
    assert!(db.query_one("SELECT count(*) FROM public.backend_outbox WHERE event_id=decode($1,'hex') AND completed_at IS NULL", &[&event]).await.unwrap().get::<_, i64>(0) > 0);
    let event_bytes: [u8; 32] = hex::decode(&event).unwrap().try_into().unwrap();
    assert!(f.state.protocol.entry(event_bytes).await.unwrap().is_none());
    f.call(None, "GET", &format!("/api/v1/chapter/{public_chapter}"), None, 404).await;
    let pending = f.call(Some(1), "GET", &format!("/api/v1/nostr/uploads/{session}"), None, 200).await;
    assert_eq!(pending["session"]["receipt"]["attributes"]["translatedLanguage"], "ta");
    drop(db);

    let publication = f.state.config.protocol_publication.as_ref().unwrap();
    // Paths and public identities only. Private key bytes and actors are never serialized.
    let config = json!({
        "bind":f.origin.strip_prefix("http://").unwrap(), "public_origin":f.origin,
        "database_plaintext_local":true, "database_connections":6,
        "uploads_dir":f.state.config.uploads_dir,
        "protocol_publication":{
            "operator_key_file":publication.operator_key_file,"expected_publisher":publication.expected_publisher,
            "storage_owner":publication.storage_owner,"storage_holder":publication.storage_holder,
            "storage":publication.storage,"group_keys_dir":publication.group_keys_dir,
            "backend":publication.backend,"native_node":publication.native_node
        }
    });
    let spec_path = f._storage.path().join("public-ta-restart-input.json");
    let result_path = f._storage.path().join("public-ta-restart-result.json");
    let spec = json!({"marker":MARKER,"parentPid":std::process::id(),"config":config,
        "session":session,"nativeSession":native_session,"chapter":public_chapter,"nativeChapter":native_chapter,
        "event":event,"rawEvent":raw_event,"manifestHex":hex::encode(&manifest),
        "manifestSha256":auth::hash(&manifest),"nativePayload":native_payload,"commitHash":commit_hash,
        "payload":payload,"resultPath":result_path,"sessionVersion":session_version});
    let old_protocol = std::sync::Arc::downgrade(&f.state.protocol);
    let old_runtime = std::sync::Arc::downgrade(&f.state.runtime);
    let old_config = std::sync::Arc::downgrade(&f.state.config);
    let old_publication = std::sync::Arc::downgrade(&f.state.protocol.publication_runtime().await.unwrap());
    // Fixture has no Drop implementation. Retain just its directory guard while
    // dropping every old router, state, publication runtime and connection pool.
    let Fixture { state, repository, http, origin: _, mut _server, _storage, native } = f;
    assert!(native.is_none());
    _server.0.abort();
    assert!((&mut _server.0).await.unwrap_err().is_cancelled());
    drop(_server);
    drop(http);
    drop(repository);
    drop(state);
    // Axum connection tasks may briefly outlive the listener task. Do not start
    // recovery until even those old state and runtime references are gone.
    tokio::time::timeout(Duration::from_secs(10), async {
        while old_protocol.strong_count() != 0 || old_runtime.strong_count() != 0
            || old_config.strong_count() != 0 || old_publication.strong_count() != 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("Old HTTP connections or publication runtime survived teardown");
    std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

    let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "language_contract::restart::recover_public_ta_in_fresh_process", "--nocapture", "--test-threads=1"])
        .env_clear().env(SPEC_ENV, &spec_path)
        .env("PLATFORM_AUTH_TEST_DATABASE_URL", dsn)
        .env("PLATFORM_TEST_RUNTIME_DIR", std::env::var_os("PLATFORM_TEST_RUNTIME_DIR").unwrap())
        .stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit())
        .kill_on_drop(true).spawn().unwrap();
    let child_pid = child.id().unwrap();
    assert_ne!(child_pid, std::process::id());
    let status = match tokio::time::timeout(Duration::from_secs(60), child.wait()).await {
        Ok(status) => status.unwrap(),
        Err(_) => {
            let _ = child.start_kill();
            tokio::time::timeout(Duration::from_secs(5), child.wait()).await
                .expect("Owned recovery child could not be reaped after kill").unwrap();
            panic!("Fresh-process public ta recovery exceeded its owned 60-second budget");
        }
    };
    assert!(status.success(), "Fresh recovery child failed: {status}");
    let result: Value = serde_json::from_slice(&std::fs::read(&result_path).unwrap()).unwrap();
    assert_eq!(result["marker"], MARKER);
    assert_eq!(result["success"], true);
    assert_eq!(result["childPid"], child_pid);
    assert_eq!(result["parentPid"], std::process::id());
    assert_eq!(result["event"], spec["event"]);
    assert_eq!(result["manifestSha256"], spec["manifestSha256"]);
    drop(_storage);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Private fresh-process helper; only the owned parent probe supplies its marker"]
async fn recover_public_ta_in_fresh_process() {
    let dsn = fixture_dsn();
    let path = PathBuf::from(std::env::var_os(SPEC_ENV).expect("Owned restart marker is required"));
    let runtime = PathBuf::from(std::env::var_os("PLATFORM_TEST_RUNTIME_DIR").unwrap()).canonicalize().unwrap();
    assert!(runtime.to_string_lossy().contains("/weebdex-platform/test-runtime/"));
    assert!(path.canonicalize().unwrap().starts_with(&runtime));
    assert_eq!(path.file_name().unwrap(), "public-ta-restart-input.json");
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file() && metadata.len() < 256 * 1024);
    let spec: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(spec["marker"], MARKER);
    let parent_pid = spec["parentPid"].as_u64().unwrap();
    assert_ne!(parent_pid, u64::from(std::process::id()));
    let result_path = PathBuf::from(spec["resultPath"].as_str().unwrap());
    assert_eq!(result_path.parent(), path.parent());
    assert_eq!(result_path.file_name().unwrap(), "public-ta-restart-result.json");
    assert!(!result_path.exists());
    let config: Config = serde_json::from_value(spec["config"].clone()).unwrap();
    assert!(!config.production && config.database_plaintext_local);
    assert!(config.bind.ip().is_loopback());
    assert!(config.uploads_dir.starts_with(path.parent().unwrap()));
    let origin = config.public_origin.clone();
    let bind = config.bind;
    let publication = config.protocol_publication.clone().unwrap();
    let state = AppState::connect(config, dsn).await.unwrap();
    state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    state.protocol.verify_schema().await.unwrap();
    state.protocol.install_visibility_function().await.unwrap();
    state.protocol.configure_publication(publication).await.unwrap();
    let event = spec["event"].as_str().unwrap();
    let session = spec["session"].as_str().unwrap();
    let native_session = spec["nativeSession"].as_str().unwrap();
    let chapter = spec["chapter"].as_str().unwrap();
    let native_chapter = spec["nativeChapter"].as_str().unwrap();
    let event_bytes: [u8; 32] = hex::decode(event).unwrap().try_into().unwrap();
    let db = state.db.client().await.unwrap();
    assert!(db.query_one("SELECT count(*) FROM public.backend_outbox WHERE event_id=decode($1,'hex') AND completed_at IS NULL", &[&event]).await.unwrap().get::<_, i64>(0) > 0);
    assert!(state.protocol.entry(event_bytes).await.unwrap().is_none());
    drop(db);
    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    let app = weebdex_platform::router::router(state.clone());
    let mut server = Server(tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); }));
    let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();
    // Fresh signed proof, fresh bound-account read, before any source recovery.
    let snapshot = child_call(&client, &origin, Some(1), "GET",
        &format!("/api/v1/nostr/uploads/{session}"), None, 200).await;
    assert_eq!(snapshot["session"]["receipt"]["attributes"]["translatedLanguage"], "ta");
    assert_eq!(snapshot["session"]["targetStatus"], "accepted_pending_projection");
    child_call(&client, &origin, None, "GET", &format!("/api/v1/chapter/{chapter}"), None, 404).await;

    let mut native = state.protocol.publication_runtime().await.unwrap().start_native_listener().await.unwrap();
    assert!(native.is_some());
    let source = tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            let report = weebdex_platform::protocol::bridge::import_page(&state).await.unwrap();
            assert_eq!(report.rejected, 0);
            let db = state.db.client().await.unwrap();
            let ready: bool = db.query_one("SELECT committed_result->>'publication_status'='locally_hosted' AND committed_result->>'language'='ta' FROM platform.upload_sessions WHERE id=$1 AND state='committed'", &[&native_session]).await.unwrap().get(0);
            drop(db);
            if ready { break state.protocol.entry(event_bytes).await.unwrap().unwrap(); }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    }).await.expect("Genuine original outbox and importer did not reconcile public ta");
    source.event.verify().unwrap();
    assert_eq!(serde_json::to_value(&source.event).unwrap(), spec["rawEvent"]);
    assert_eq!(source.metadata["language"], "ta");
    let public_chapter = child_call(&client, &origin, None, "GET",
        &format!("/api/v1/chapter/{chapter}"), None, 200).await;
    assert_eq!(public_chapter["data"]["attributes"]["translatedLanguage"], "ta");
    let replay = child_call(&client, &origin, Some(1), "POST",
        &format!("/api/v1/upload/{session}/commit"), Some(spec["payload"].clone()), 200).await;
    assert_eq!(replay["id"], chapter);
    assert_eq!(replay["attributes"]["translatedLanguage"], "ta");
    assert_eq!(replay["attributes"]["nostr"]["eventId"], event);
    assert_eq!(replay["attributes"]["nostr"]["publicationStatus"], "locally_hosted");
    let snapshot = child_call(&client, &origin, Some(1), "GET",
        &format!("/api/v1/nostr/uploads/{session}"), None, 200).await;
    assert_eq!(snapshot["session"]["receipt"]["attributes"]["translatedLanguage"], "ta");

    // The fresh process still rejects ta on the unchanged native mutation route.
    let response = client.put(format!("{origin}/api/chapter/{native_chapter}"))
        .header("origin", &origin).header("cookie", "weebdex_session=synthetic-public-upload-cookie")
        .json(&json!({"version":public_chapter["data"]["attributes"]["version"],"language":"ta"}))
        .send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    assert_eq!(status, 400, "Native ta contract changed after recovery: {text}");
    assert!(text.contains("Unsupported language code"), "Native rejection was not the retained language enum: {text}");
    let db = state.db.client().await.unwrap();
    let row = db.query_one("SELECT p.source_manifest,p.native_commit_payload,p.native_commit_hash,e.raw_event,c.language,s.committed_result,s.version,p.state FROM platform.protocol_publication_commands p JOIN platform.upload_sessions s ON s.id=p.upload_session_id JOIN platform.catalog_chapters c ON c.id=s.committed_chapter_id JOIN public.events e ON e.id=decode(p.event_id,'hex') WHERE p.upload_session_id=$1 AND p.event_id=$2", &[&native_session, &event]).await.unwrap();
    let manifest: Vec<u8> = row.get(0);
    assert_eq!(hex::encode(&manifest), spec["manifestHex"]);
    assert_eq!(auth::hash(&manifest), spec["manifestSha256"]);
    assert_eq!(manifest_language(&manifest), "ta");
    assert_eq!(row.get::<_, Value>(1), spec["nativePayload"]);
    assert_eq!(row.get::<_, String>(2), spec["commitHash"]);
    assert_eq!(row.get::<_, Value>(3), spec["rawEvent"]);
    assert_eq!(row.get::<_, String>(4), "ta");
    let stored: Value = row.get(5);
    assert_eq!(stored["language"], "ta");
    assert_eq!(stored["publication_status"], "locally_hosted");
    let session_version: i32 = row.get(6);
    assert_eq!(i64::from(session_version), spec["sessionVersion"].as_i64().unwrap() + 1,
        "Pending receipt must reconcile once, not replay its commit");
    assert_eq!(row.get::<_, String>(7), "materialized");
    assert_eq!(db.query_one("SELECT count(*) FROM public.backend_outbox WHERE event_id=decode($1,'hex') AND completed_at IS NULL", &[&event]).await.unwrap().get::<_, i64>(0), 0);
    assert_eq!(db.query_one("SELECT count(*) FROM public.events WHERE kind=1001 AND publisher=decode($1,'hex') AND series=(SELECT series FROM public.events WHERE id=decode($2,'hex'))", &[&public(1), &event]).await.unwrap().get::<_, i64>(0), 1);
    drop(db);
    for _ in 0..2 {
        let mut complete = false;
        for _ in 0..8 {
            let report = weebdex_platform::protocol::bridge::import_page(&state).await.unwrap();
            assert_eq!(report.rejected, 0);
            if report.completed_sweep { complete = true; break; }
        }
        assert!(complete, "Bounded fixture import sweep did not complete");
    }
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT version FROM platform.upload_sessions WHERE id=$1", &[&native_session]).await.unwrap().get::<_, i32>(0), session_version);
    stop_native(&mut native).await;
    server.0.abort();
    assert!((&mut server.0).await.unwrap_err().is_cancelled());
    std::fs::write(result_path, serde_json::to_vec(&json!({"marker":MARKER,"success":true,
        "parentPid":parent_pid,"childPid":std::process::id(),"event":event,
        "manifestSha256":auth::hash(&manifest),"gracefulRuntimeRestart":true,"crashInjected":false})).unwrap()).unwrap();
}
