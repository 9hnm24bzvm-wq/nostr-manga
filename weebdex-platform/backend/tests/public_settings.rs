//! Actual TCP, original request authorization and single-source private preferences.
//! Every key/account below is synthetic and belongs only to this disposable DB.
use base64::{Engine, engine::general_purpose::STANDARD};
use manga_node::{
    auth::{self, Event},
    db::{DbConfig, Id, Repository},
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use weebdex_platform::{AppState, Config, domains, protocol::publication::OperatorRuntimeConfig};
fn public(key: u8) -> String {
    auth::sign(&format!("{key:064x}"), 1, vec![], String::new())
        .unwrap()
        .pubkey
}
fn id(key: u8) -> Id {
    hex::decode(public(key)).unwrap().try_into().unwrap()
}
fn proof_for(key: u8, service: u8, url: &str, method: &str, body: &[u8]) -> String {
    static NONCE: AtomicUsize = AtomicUsize::new(0);
    let secret = SecretKey::from_slice(&hex::decode(format!("{key:064x}")).unwrap()).unwrap();
    let secp = Secp256k1::new();
    let pair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: String::new(),
        pubkey: public(key),
        created_at: auth::now(),
        kind: 27235,
        tags: vec![
            vec!["u".into(), url.into()],
            vec!["method".into(), method.into()],
            vec!["service".into(), public(service)],
            vec!["payload".into(), auth::hash(body)],
            vec![
                "nonce".into(),
                NONCE.fetch_add(1, Ordering::Relaxed).to_string(),
            ],
        ],
        content: String::new(),
        sig: String::new(),
    };
    let digest = event.digest();
    event.id = hex::encode(&digest);
    event.sig = secp
        .sign_schnorr_no_aux_rand(&Message::from_digest_slice(&digest).unwrap(), &pair)
        .to_string();
    format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    )
}
fn proof(key: u8, url: &str, method: &str, body: &[u8]) -> String {
    proof_for(key, 11, url, method, body)
}
async fn call(
    client: &reqwest::Client,
    origin: &str,
    method: &str,
    path: &str,
    payload: Option<Value>,
    key: Option<u8>,
    expected: u16,
) -> Value {
    let url = format!("{origin}{path}");
    let body = payload.map_or_else(Vec::new, |value| serde_json::to_vec(&value).unwrap());
    let mut request = client
        .request(method.parse().unwrap(), &url)
        .header("content-type", "application/json");
    if let Some(key) = key {
        request = request.header("authorization", proof(key, &url, method, &body));
    }
    let response = request.body(body).send().await.unwrap();
    let status = response.status().as_u16();
    assert!(
        response.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    let value: Value = response.json().await.unwrap();
    assert_eq!(status, expected, "{method} {path}: {value}");
    value
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires scripts/run-auth-tests.sh and its owned isolated PostgreSQL"]
async fn signed_settings_roundtrip_native_site_state_without_authority_or_concurrency_bypasses() {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(), Some("weebdex_platform_test"));
    assert!(
        !pg.get_hosts().is_empty(),
        "Explicit owned loopback host required"
    );
    assert!(
        pg.get_hosts()
            .iter()
            .all(|h| matches!(h,tokio_postgres::config::Host::Tcp(h) if h=="127.0.0.1"))
    );
    assert!(
        std::env::var("PLATFORM_TEST_RUNTIME_DIR")
            .unwrap()
            .contains("/weebdex-platform/test-runtime/")
    );
    let runtime = std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap();
    assert!(runtime.contains("/weebdex-platform/test-runtime/auth-"));
    let receipt: Value = serde_json::from_slice(
        &std::fs::read(std::path::Path::new(&runtime).join("fixture.json"))
            .expect("Owned fixture receipt"),
    )
    .unwrap();
    assert_eq!(receipt["database"], "weebdex_platform_test");
    assert_eq!(receipt["fixture_only"], true);
    let repository = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 2,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await
    .unwrap();
    repository.migrate().await.unwrap();
    repository.bind_instance(id(11), id(1)).await.unwrap();
    for key in 2..=10 {
        repository
            .set_account_admission(id(key), true)
            .await
            .unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let state=AppState::connect(Config{public_origin:origin.clone(),database_plaintext_local:true,database_connections:6,
        protocol_publication:Some(OperatorRuntimeConfig{operator_key_file:"fixture-unused/operator.key".into(),expected_publisher:public(1),storage_owner:public(1),storage_holder:public(11),storage:json!({"path":"fixture-unused/storage","volume":"settingsfixture","physical_bytes":1048576,"staging_bytes":1048576,"max_blob_bytes":262144}),group_keys_dir:None,backend:None,native_node:Some(json!({"bind":"127.0.0.1:9","base_url":"http://127.0.0.1:9","service_key_file":"fixture-unused/service.key","admin_pubkey":public(1)}))}),..Config::default()},dsn).await.unwrap();
    let position = domains::MIGRATIONS
        .iter()
        .position(|m| m.version == 95)
        .unwrap();
    state
        .db
        .migrate(&domains::MIGRATIONS[..position])
        .await
        .unwrap();
    let sql = state.db.client().await.unwrap();
    for (key, roles, permissions, verified, disabled) in [
        (
            2,
            json!([]),
            json!(["self.preferences.update"]),
            true,
            false,
        ),
        (
            4,
            json!([]),
            json!(["self.preferences.update"]),
            true,
            false,
        ),
        (5, json!(["sysop"]), json!([]), true, false),
        (
            6,
            json!(["moderator"]),
            json!(["settings.template.create"]),
            true,
            false,
        ),
        (7, json!(["sysop"]), json!([]), true, false),
        (
            8,
            json!(["restricted"]),
            json!(["self.preferences.update"]),
            true,
            false,
        ),
        (9, json!([]), json!(["self.preferences.update"]), true, true),
        (
            10,
            json!([]),
            json!(["self.preferences.update"]),
            false,
            false,
        ),
    ] {
        let user = format!("SettingsUser{key}");
        sql.execute("INSERT INTO platform.users(id,username,email,password_hash,roles,permissions,verified,disabled) VALUES($1,$1,$2,'synthetic-no-password',$3,$4,$5,$6)",&[&user,&format!("settings-{key}@fixture.invalid"),&roles,&permissions,&verified,&disabled]).await.unwrap();
        sql.execute(
            "INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)",
            &[&public(key), &user],
        )
        .await
        .unwrap();
    }
    drop(sql);
    state.db.migrate(domains::MIGRATIONS).await.unwrap();
    state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    state.protocol.verify_schema().await.unwrap();
    let sql = state.db.client().await.unwrap();
    let grant:bool=sql.query_one("SELECT permissions ? 'settings.template.create' FROM platform.users WHERE id='SettingsUser5'",&[]).await.unwrap().get(0);
    assert!(grant);
    let ordinary:bool=sql.query_one("SELECT permissions ? 'settings.template.create' FROM platform.users WHERE id='SettingsUser2'",&[]).await.unwrap().get(0);
    assert!(!ordinary);
    sql.execute(
        "UPDATE platform.users SET permissions='[]'::jsonb WHERE id='SettingsUser7'",
        &[],
    )
    .await
    .unwrap();
    use sha2::{Digest, Sha256};
    let token = "synthetic-settings-native-session";
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    sql.execute("INSERT INTO platform.sessions(id,token_hash,user_id,expires_at) VALUES('settings-session',$1,'SettingsUser2',clock_timestamp()+interval '1 hour')",&[&hash]).await.unwrap();
    drop(sql);
    let app = weebdex_platform::router::router(state.clone());
    let _server = Server(tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    }));
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let path = "/api/v1/settings";
    let template_path = "/api/v1/settings/template";
    call(&client, &origin, "GET", path, None, None, 401).await;
    call(&client, &origin, "GET", path, None, Some(3), 403).await;
    for key in [8, 9, 10] {
        call(&client, &origin, "GET", path, None, Some(key), 403).await;
    }
    call(&client, &origin, "PUT", path, None, Some(2), 405).await;
    let before = call(&client, &origin, "GET", path, None, Some(2), 200).await;
    assert_eq!(before["settings"]["version"], 1);
    assert_eq!(before["settings"]["data_saver"], false);
    let baseline = call(&client, &origin, "GET", template_path, None, Some(2), 200).await;
    let baseline_id = before["template"].as_str().unwrap();
    assert_eq!(
        baseline,
        call(
            &client,
            &origin,
            "GET",
            &format!("{template_path}/{baseline_id}"),
            None,
            Some(2),
            200
        )
        .await
    );
    call(
        &client,
        &origin,
        "GET",
        &format!("{template_path}/bad-uuid"),
        None,
        Some(2),
        400,
    )
    .await;
    call(
        &client,
        &origin,
        "GET",
        &format!("{template_path}/11111111-1111-4111-8111-111111111111"),
        None,
        Some(2),
        404,
    )
    .await;
    call(
        &client,
        &origin,
        "GET",
        "/api/v1/settings?user=SettingsUser4",
        None,
        Some(2),
        400,
    )
    .await;
    for key in [2, 6, 7] {
        call(
            &client,
            &origin,
            "POST",
            template_path,
            Some(baseline.clone()),
            Some(key),
            403,
        )
        .await;
    }
    let u = format!("{origin}{path}");
    let response = client
        .get(&u)
        .header("cookie", format!("weebdex_session={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let response = client
        .get(&u)
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let response = client
        .get(&u)
        .header("authorization", proof_for(2, 12, &u, "GET", b""))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let authorization = proof(2, &u, "GET", b"");
    assert_eq!(
        client
            .get(&u)
            .header("authorization", &authorization)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(&u)
            .header("authorization", &authorization)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let invalid = client
        .post(&u)
        .header("content-type", "application/json")
        .header("authorization", proof(2, &u, "POST", b"{"))
        .body("{")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 400);
    let invalid = client
        .post(&u)
        .header("content-type", "text/plain")
        .header("authorization", proof(2, &u, "POST", b"{}"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 415);
    let invalid = client
        .post(&u)
        .header("content-type", "application/json")
        .header("authorization", proof(2, &u, "POST", b"{}"))
        .body("{\"settings\":{}}")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 401);
    let oversized = vec![b'x'; 131073];
    let invalid = client
        .post(&u)
        .header("content-type", "application/json")
        .header("authorization", proof(2, &u, "POST", &oversized))
        .body(oversized)
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 413);
    let mut settings = before["settings"].clone();
    settings["data_saver"] = json!(true);
    settings["reader_theme"] = json!({"mode":"sepia","scale":1.125,"fonts":["system","serif"]});
    let input = json!({"updatedAt":before["updatedAt"],"settings":settings});
    let saved = call(
        &client,
        &origin,
        "POST",
        path,
        Some(input.clone()),
        Some(2),
        200,
    )
    .await;
    assert_eq!(saved["settings"]["version"], 2);
    assert_eq!(saved["settings"]["reader_theme"], settings["reader_theme"]);
    assert!(saved["updatedAt"].as_str().unwrap() > before["updatedAt"].as_str().unwrap());
    call(&client, &origin, "POST", path, Some(input), Some(2), 409).await;
    let other = call(&client, &origin, "GET", path, None, Some(4), 200).await;
    assert!(other["settings"].get("reader_theme").is_none());
    let mut invalid = saved["settings"].clone();
    invalid["items_per_page"] = json!("101");
    call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":invalid,"updatedAt":saved["updatedAt"]})),
        Some(2),
        400,
    )
    .await;
    let mut invalid = saved["settings"].clone();
    invalid["groups"] = json!(["unknown-group"]);
    call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":invalid,"updatedAt":saved["updatedAt"]})),
        Some(2),
        400,
    )
    .await;
    let mut invalid = saved["settings"].clone();
    invalid.as_object_mut().unwrap().remove("languages");
    call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":invalid,"updatedAt":saved["updatedAt"]})),
        Some(2),
        400,
    )
    .await;
    let mut invalid = saved["settings"].clone();
    invalid["version"] = json!(999);
    call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":invalid,"updatedAt":saved["updatedAt"]})),
        Some(2),
        409,
    )
    .await;
    call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":saved["settings"],"updatedAt":"private-invalid-time"})),
        Some(2),
        400,
    )
    .await;
    assert_eq!(
        saved,
        call(&client, &origin, "GET", path, None, Some(2), 200).await,
        "failed writes are atomic"
    );
    for schema in [
        json!({"type":"invalid"}),
        json!({"$ref":"https://example.invalid/private"}),
        json!({"$ref":"file:///etc/passwd"}),
        json!({"$ref":"#"}),
        json!({"type":"string","pattern":"(?=lookahead)"}),
    ] {
        call(
            &client,
            &origin,
            "POST",
            template_path,
            Some(schema),
            Some(5),
            400,
        )
        .await;
    }
    assert_eq!(
        baseline,
        call(&client, &origin, "GET", template_path, None, Some(2), 200).await,
        "invalid templates never advance the head"
    );
    let oversized = vec![b' '; 65537];
    let oversized_response = client
        .post(format!("{origin}{template_path}"))
        .header("content-type", "application/json")
        .header(
            "authorization",
            proof(5, &format!("{origin}{template_path}"), "POST", &oversized),
        )
        .body(oversized)
        .send()
        .await
        .unwrap();
    assert_eq!(oversized_response.status(), 413);
    let mut custom = baseline.clone();
    custom["properties"]["ui_density"] = json!({"enum":["compact","comfortable"]});
    custom["required"]
        .as_array_mut()
        .unwrap()
        .push(json!("ui_density"));
    let schema_url = format!("{origin}{template_path}");
    let bytes = serde_json::to_vec(&custom).unwrap();
    let response = client
        .post(&schema_url)
        .header("content-type", "application/json")
        .header("authorization", proof(5, &schema_url, "POST", &bytes))
        .body(bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let custom_location = response.headers()["location"].to_str().unwrap().to_owned();
    let custom_id = custom_location.rsplit('/').next().unwrap().to_owned();
    assert_eq!(response.json::<Value>().await.unwrap(), custom);
    assert_eq!(
        custom,
        call(&client, &origin, "GET", template_path, None, Some(2), 200).await
    );
    assert_eq!(
        custom,
        call(
            &client,
            &origin,
            "GET",
            &custom_location,
            None,
            Some(2),
            200
        )
        .await
    );
    assert_eq!(
        baseline,
        call(
            &client,
            &origin,
            "GET",
            &format!("{template_path}/{baseline_id}"),
            None,
            Some(2),
            200
        )
        .await
    );
    call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":saved["settings"],"updatedAt":saved["updatedAt"]})),
        Some(2),
        400,
    )
    .await;
    let mut settings = saved["settings"].clone();
    settings["ui_density"] = json!("compact");
    let custom_saved = call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":settings,"updatedAt":saved["updatedAt"]})),
        Some(2),
        200,
    )
    .await;
    assert_eq!(custom_saved["template"], custom_id);
    assert_eq!(custom_saved["settings"]["version"], 3);
    let mut reopened_config = (*state.config).clone();
    reopened_config.database_connections = 4;
    let reopened = AppState::connect(
        reopened_config,
        std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").unwrap(),
    )
    .await
    .unwrap();
    let persisted=reopened.db.client().await.unwrap().query_one("SELECT preferences,preferences_template_id::text FROM platform.users WHERE id='SettingsUser2'",&[]).await.unwrap();
    assert_eq!(persisted.get::<_, Value>(0), custom_saved["settings"]);
    assert_eq!(persisted.get::<_, String>(1), custom_id);
    drop(reopened);
    let native = client
        .put(format!("{origin}/api/settings"))
        .header("cookie", format!("weebdex_session={token}"))
        .header("origin", &origin)
        .json(&json!({"version":3,"data_saver":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(native.status(), 200, "{}", native.text().await.unwrap());
    let after_native = call(&client, &origin, "GET", path, None, Some(2), 200).await;
    assert_eq!(after_native["settings"]["version"], 4);
    assert_eq!(after_native["settings"]["data_saver"], false);
    assert_eq!(
        after_native["settings"]["reader_theme"],
        settings["reader_theme"]
    );
    assert_eq!(after_native["settings"]["ui_density"], "compact");
    assert_eq!(after_native["template"], baseline_id);
    assert!(
        after_native["updatedAt"].as_str().unwrap() > custom_saved["updatedAt"].as_str().unwrap()
    );
    let projected: Value = client
        .get(format!("{origin}/api/auth/check"))
        .header("cookie", format!("weebdex_session={token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        projected["preferences"]["reader_theme"],
        settings["reader_theme"]
    );
    assert_eq!(projected["preferences"]["data_saver"], false);
    let input = json!({"settings":after_native["settings"],"updatedAt":after_native["updatedAt"]});
    let bytes = serde_json::to_vec(&input).unwrap();
    let one = client
        .post(&u)
        .header("content-type", "application/json")
        .header("authorization", proof(2, &u, "POST", &bytes))
        .body(bytes.clone())
        .send();
    let two = client
        .post(&u)
        .header("content-type", "application/json")
        .header("authorization", proof(2, &u, "POST", &bytes))
        .body(bytes)
        .send();
    let (one, two) = tokio::join!(one, two);
    let mut statuses = [
        one.unwrap().status().as_u16(),
        two.unwrap().status().as_u16(),
    ];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    let current = call(&client, &origin, "GET", path, None, Some(2), 200).await;
    let mut without_version = current["settings"].clone();
    without_version.as_object_mut().unwrap().remove("version");
    let authoritative = call(
        &client,
        &origin,
        "POST",
        path,
        Some(json!({"settings":without_version,"updatedAt":current["updatedAt"]})),
        Some(2),
        200,
    )
    .await;
    assert_eq!(authoritative["settings"]["version"], 6);
    let native_noop = client
        .put(format!("{origin}/api/settings"))
        .header("cookie", format!("weebdex_session={token}"))
        .header("origin", &origin)
        .json(&json!({"version":6}))
        .send()
        .await
        .unwrap();
    assert_eq!(native_noop.status(), 200);
    assert_eq!(
        authoritative,
        call(&client, &origin, "GET", path, None, Some(2), 200).await,
        "native no-op retains revision, timestamp and schema linkage"
    );
    let mut racing = authoritative["settings"].clone();
    racing["data_saver"] = json!(true);
    let bytes =
        serde_json::to_vec(&json!({"settings":racing,"updatedAt":authoritative["updatedAt"]}))
            .unwrap();
    let public_race = client
        .post(&u)
        .header("content-type", "application/json")
        .header("authorization", proof(2, &u, "POST", &bytes))
        .body(bytes)
        .send();
    let native_race = client
        .put(format!("{origin}/api/settings"))
        .header("cookie", format!("weebdex_session={token}"))
        .header("origin", &origin)
        .json(&json!({"version":6,"data_saver":false}))
        .send();
    let (public_race, native_race) = tokio::join!(public_race, native_race);
    let mut mixed = [
        public_race.unwrap().status().as_u16(),
        native_race.unwrap().status().as_u16(),
    ];
    mixed.sort();
    assert_eq!(mixed, [200, 409]);
    let authoritative = call(&client, &origin, "GET", path, None, Some(2), 200).await;
    assert_eq!(authoritative["settings"]["version"], 7);
    let sql = state.db.client().await.unwrap();
    sql.execute(
        "UPDATE platform.users SET updated_at=clock_timestamp() WHERE id='SettingsUser2'",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        authoritative,
        call(&client, &origin, "GET", path, None, Some(2), 200).await,
        "unrelated account timestamp changes do not invalidate preference tokens"
    );
    let before_rows: i64 = sql
        .query_one("SELECT count(*) FROM platform.public_resource_ids", &[])
        .await
        .unwrap()
        .get(0);
    call(&client, &origin, "GET", path, None, Some(2), 200).await;
    call(&client, &origin, "GET", template_path, None, Some(2), 200).await;
    let after_rows: i64 = sql
        .query_one("SELECT count(*) FROM platform.public_resource_ids", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(before_rows, after_rows);
    assert!(
        sql.execute(
            "UPDATE platform.settings_templates SET document='{}'::jsonb WHERE id=$1::text::uuid",
            &[&custom_id]
        )
        .await
        .is_err()
    );
    drop(sql);
    repository
        .set_account_admission(id(2), false)
        .await
        .unwrap();
    call(&client, &origin, "GET", path, None, Some(2), 403).await;
}
