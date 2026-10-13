//! Real router + real isolated PostgreSQL seams. This never targets an existing
//! user database and must be explicitly selected by scripts/run-auth-tests.sh.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tower::ServiceExt;
use weebdex_platform::{AppState, Config};

#[derive(Default, Clone)]
struct Browser {
    cookies: BTreeMap<String, String>,
    expected_account: Option<String>,
}
impl Browser {
    async fn request(
        &mut self,
        app: &Router,
        method: Method,
        path: &str,
        payload: Option<Value>,
        origin: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if !self.cookies.is_empty() {
            request = request.header(
                header::COOKIE,
                self.cookies
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            );
        }
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        if let Some(expected) = self.expected_account.as_deref() {
            request = request.header("x-weebdex-expected-account", expected);
        }
        let body = if let Some(payload) = payload {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(payload.to_string())
        } else {
            Body::empty()
        };
        let response = app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        assert!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .is_some_and(|v| v.to_str().unwrap().contains("no-store"))
        );
        for cookie in response.headers().get_all(header::SET_COOKIE) {
            let cookie = cookie.to_str().unwrap();
            assert!(cookie.contains("HttpOnly"));
            assert!(cookie.contains("SameSite=Lax"));
            assert!(cookie.contains("Secure"));
            let pair = cookie.split(';').next().unwrap();
            let (name, value) = pair.split_once('=').unwrap();
            if value.is_empty() {
                self.cookies.remove(name);
            } else {
                self.cookies.insert(name.into(), value.into());
            }
        }
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (
            status,
            if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap()
            },
        )
    }
}

fn csrf(flow: &Value) -> String {
    flow["ui"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["attributes"]["name"] == "csrf_token")
        .unwrap()["attributes"]["value"]
        .as_str()
        .unwrap()
        .into()
}
fn flow_id(flow: &Value) -> &str {
    flow["id"].as_str().unwrap()
}
const ORIGIN: &str = "https://auth-test.example.invalid";
const FIXTURE_PASSWORD: &str = "isolated-auth-test-password";

async fn bearer_status(
    app: &Router,
    key: &str,
    method: Method,
    path: &str,
    payload: Option<Value>,
) -> StatusCode {
    bearer_status_with_expected(app, key, method, path, payload, &[]).await
}

async fn bearer_status_with_expected(
    app: &Router,
    key: &str,
    method: Method,
    path: &str,
    payload: Option<Value>,
    expected_accounts: &[&str],
) -> StatusCode {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {key}"))
        .header(header::ORIGIN, ORIGIN);
    for expected in expected_accounts {
        request = request.header("x-weebdex-expected-account", *expected);
    }
    let body = if let Some(payload) = payload {
        request = request.header(header::CONTENT_TYPE, "application/json");
        Body::from(payload.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    assert!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .is_some_and(|h| h.to_str().unwrap().contains("no-store"))
    );
    let status = response.status();
    let _ = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    status
}

async fn isolated_state() -> AppState {
    let dsn = std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL")
        .expect("Run the isolated auth test script; no production database is accepted");
    let config: tokio_postgres::Config = dsn.parse().unwrap();
    assert_eq!(
        config.get_dbname(),
        Some("weebdex_platform_test"),
        "Only the reserved isolated test database is allowed"
    );
    assert!(!config.get_hosts().is_empty());
    let runtime = std::env::var("PLATFORM_TEST_RUNTIME_DIR")
        .expect("An owned test runtime receipt is required");
    assert!(runtime.contains("/weebdex-platform/test-runtime/"));
    assert!(
        std::path::Path::new(&runtime)
            .join("fixture.json")
            .is_file()
    );
    for host in config.get_hosts() {
        match host {
            tokio_postgres::config::Host::Tcp(host) => assert_eq!(host, "127.0.0.1"),
            _ => panic!("Only the owned isolated loopback test listener is allowed"),
        }
    }
    assert!(config.get_ports().iter().all(|p| *p > 1024 && *p != 5432));
    let config = Config {
        public_origin: ORIGIN.into(),
        database_plaintext_local: true,
        registration_enabled: true,
        database_connections: 4,
        ..Config::default()
    };
    let state = AppState::connect(config, dsn).await.unwrap();
    state
        .db
        .migrate(weebdex_platform::domains::MIGRATIONS)
        .await
        .unwrap();
    state
}

async fn registration(app: &Router, browser: &mut Browser, email: &str, username: &str) -> Value {
    let (status, flow) = browser
        .request(
            app,
            Method::GET,
            "/auth/self-service/registration/browser?return_to=%2F",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    for field in ["csrf_token", "traits.email", "traits.username", "password"] {
        assert!(
            flow["ui"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["attributes"]["name"] == field)
        );
    }
    assert!(
        !flow["ui"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["group"] == "oidc")
    );
    let (status, fetched) = browser
        .request(
            app,
            Method::GET,
            &format!(
                "/auth/self-service/registration/flows?id={}",
                flow_id(&flow)
            ),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(csrf(&flow), csrf(&fetched));
    let path = format!("/auth/self-service/registration?flow={}", flow_id(&flow));
    let (status,result)=browser.request(app,Method::POST,&path,Some(json!({"method":"password","csrf_token":csrf(&flow),"traits":{"email":email,"username":username},"password":FIXTURE_PASSWORD})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["session"]["active"], true);
    assert_eq!(
        result["session"]["identity"]["verifiable_addresses"][0]["verified"],
        false
    );
    result
}

#[tokio::test]
#[ignore = "Requires owned isolated PostgreSQL cluster; use scripts/run-auth-tests.sh"]
async fn actual_browser_flows_sessions_csrf_expiry_preferences_and_client_revocation() {
    let state = isolated_state().await;
    let app = weebdex_platform::router::router(state.clone());
    let mut browser = Browser::default();
    let (status, error) = browser
        .request(&app, Method::GET, "/api/auth/check", None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(error.get("identity").is_none());
    let (status, _) = browser
        .request(
            &app,
            Method::GET,
            "/auth/self-service/settings/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = browser
        .request(
            &app,
            Method::GET,
            "/auth/self-service/login/browser?return_to=https%3A%2F%2Fevil.example.invalid",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, flow) = browser
        .request(
            &app,
            Method::GET,
            "/auth/self-service/login/browser?return_to=%2F",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, read) = browser
        .request(
            &app,
            Method::GET,
            &format!("/auth/self-service/login/flows?id={}", flow_id(&flow)),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(csrf(&flow), csrf(&read));
    let mut attacker = Browser::default();
    let (status, _) = attacker
        .request(
            &app,
            Method::GET,
            &format!("/auth/self-service/login/flows?id={}", flow_id(&flow)),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let login_path = format!("/auth/self-service/login?flow={}", flow_id(&flow));
    let body = json!({"method":"password","csrf_token":"invalid","identifier":"fixture","password":FIXTURE_PASSWORD});
    let (status, _) = browser
        .request(&app, Method::POST, &login_path, Some(body), Some(ORIGIN))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let body = json!({"method":"password","csrf_token":csrf(&flow),"identifier":"unknown@example.invalid","password":FIXTURE_PASSWORD});
    let (status, _) = browser
        .request(
            &app,
            Method::POST,
            &login_path,
            Some(body.clone()),
            Some("https://evil.example.invalid"),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, result) = browser
        .request(&app, Method::POST, &login_path, Some(body), Some(ORIGIN))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(result["ui"]["messages"].as_array().unwrap().len() > 0);
    assert!(result.get("session").is_none());
    state.db.client().await.unwrap().execute("UPDATE platform.auth_flows SET expires_at=clock_timestamp()-interval '1 second',created_at=clock_timestamp()-interval '20 minutes' WHERE id=$1",&[&flow_id(&flow)]).await.unwrap();
    let (status, error) = browser
        .request(
            &app,
            Method::GET,
            &format!("/auth/self-service/login/flows?id={}", flow_id(&flow)),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::GONE);
    assert!(error["return_to"].as_str().unwrap().starts_with(ORIGIN));
    let (status, error) = browser
        .request(
            &app,
            Method::GET,
            &format!("/auth/self-service/errors?id={}", flow_id(&flow)),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(error["error"]["message"].is_string());
    let registered = registration(
        &app,
        &mut browser,
        "first@example.invalid",
        "auth-fixture-one",
    )
    .await;
    let user_id = registered["session"]["identity"]["id"].as_str().unwrap();
    let (status, whoami) = browser
        .request(&app, Method::GET, "/auth/sessions/whoami", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(whoami["active"], true);
    let (status, session) = browser
        .request(&app, Method::GET, "/api/auth/check", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["identity"]["is_verified"], false);
    assert_eq!(session["preferences"]["version"], 1);
    assert_eq!(session["follows"], json!({}));
    assert!(
        !session["permissions"]
            .as_array()
            .unwrap()
            .contains(&json!("client.create"))
    );
    let (status, _) = browser
        .request(
            &app,
            Method::POST,
            "/api/client",
            Some(json!({"name":"Unverified must fail"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, prefs) = browser
        .request(
            &app,
            Method::PUT,
            "/api/settings",
            Some(json!({"version":1,"show_safe":false,"items_per_page":"40"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(prefs["version"], 2);
    assert_eq!(prefs["show_safe"], false);
    let (status, _) = browser
        .request(
            &app,
            Method::PUT,
            "/api/settings",
            Some(json!({"version":1,"show_safe":true})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Test-only synthetic verification, never a public verification shortcut.
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.users SET verified=true WHERE id=$1",
            &[&user_id],
        )
        .await
        .unwrap();
    let (status, client) = browser
        .request(
            &app,
            Method::POST,
            "/api/client",
            Some(json!({"name":"Synthetic fixture client"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let client_id = client["id"].as_str().unwrap();
    let first_key = client["key"].as_str().unwrap().to_owned();
    let mut other_owner = Browser::default();
    let other = registration(
        &app,
        &mut other_owner,
        "other-owner@example.invalid",
        "other-owner-fixture",
    )
    .await;
    let other_id = other["session"]["identity"]["id"].as_str().unwrap();
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.users SET verified=true WHERE id=$1",
            &[&other_id],
        )
        .await
        .unwrap();
    // Another tab's cookie/account switch must not turn a reviewed operation
    // into a mutation of a different account. This header constrains authority.
    other_owner.expected_account = Some(user_id.to_owned());
    let (status, _) = other_owner
        .request(
            &app,
            Method::POST,
            &format!("/api/client/{client_id}"),
            Some(json!({"name":"Must not mutate after account switch"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let stored_name: String = state
        .db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT name FROM platform.api_clients WHERE id=$1",
            &[&client_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored_name, "Synthetic fixture client");
    other_owner.expected_account = None;
    let (status, _) = other_owner
        .request(
            &app,
            Method::POST,
            &format!("/api/client/{client_id}"),
            Some(json!({"name":"Cannot rename another owner's client"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = other_owner
        .request(
            &app,
            Method::POST,
            &format!("/api/client/{client_id}/secret"),
            None,
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = other_owner
        .request(
            &app,
            Method::DELETE,
            &format!("/api/client/{client_id}"),
            None,
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, clients) = browser
        .request(&app, Method::GET, "/api/client", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(clients[0].get("key").is_none());
    browser.expected_account = Some(user_id.to_owned());
    let (status, renamed) = browser
        .request(
            &app,
            Method::POST,
            &format!("/api/client/{client_id}"),
            Some(json!({"name":"Renamed fixture"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["version"], 2);
    assert!(renamed.get("key").is_none());
    browser.expected_account = None;
    let mut bearer = axum::http::HeaderMap::new();
    bearer.insert(
        header::AUTHORIZATION,
        format!("Bearer {first_key}").parse().unwrap(),
    );
    assert!(
        weebdex_platform::domains::auth::optional_principal(&state, &bearer)
            .await
            .unwrap()
            .is_some()
    );
    // A real issued library credential remains narrow even when its owning
    // browser account is staff. Test observable public router admission, not a
    // forged successful response or a browser session substituted for bearer.
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.users SET roles='[\"staff\",\"moderator\"]'::jsonb WHERE id=$1",
            &[&user_id],
        )
        .await
        .unwrap();
    let scoped = weebdex_platform::domains::auth::optional_principal(&state, &bearer)
        .await
        .unwrap()
        .unwrap();
    assert!(scoped.is_api_client);
    assert_eq!(scoped.permissions, vec!["self.library.read"]);
    assert!(!scoped.is_staff());
    assert!(!scoped.is_moderator());
    for path in [
        "/api/user/follows/manga",
        "/api/user/history/manga",
        "/api/user/lists",
    ] {
        assert_eq!(
            bearer_status(&app, &first_key, Method::GET, path, None).await,
            StatusCode::OK,
            "library scope should read {path}"
        );
    }
    assert_eq!(
        bearer_status_with_expected(
            &app,
            &first_key,
            Method::GET,
            "/api/user/history/manga",
            None,
            &[user_id]
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        bearer_status_with_expected(
            &app,
            &first_key,
            Method::GET,
            "/api/user/history/manga",
            None,
            &[other_id]
        )
        .await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        bearer_status_with_expected(
            &app,
            &first_key,
            Method::GET,
            "/api/user/history/manga",
            None,
            &[""]
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        bearer_status_with_expected(
            &app,
            &first_key,
            Method::GET,
            "/api/user/history/manga",
            None,
            &[user_id, user_id]
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    // Persist actual public discussion/report targets through the same Router.
    // A scoped token must fail before SQL even when its browser account is staff.
    let (status, group) = other_owner
        .request(
            &app,
            Method::POST,
            "/api/group",
            Some(json!({"name":"Scoped bearer discussion fixture"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let thread = group["relationships"]["thread"]["id"].as_str().unwrap();
    let replies_path = format!("/api/thread/{thread}/replies");
    let (status, reply) = other_owner
        .request(
            &app,
            Method::POST,
            &replies_path,
            Some(json!({"body":"A genuine browser reply"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let reply_id = reply["id"].as_i64().unwrap();
    let reply_path = format!("{replies_path}/{reply_id}");
    let thread_vote = format!("/api/thread/{thread}/vote");
    let reply_vote = format!("{reply_path}/vote");
    for path in [&thread_vote, &reply_vote] {
        assert_eq!(
            bearer_status(
                &app,
                &first_key,
                Method::POST,
                path,
                Some(json!({"vote":1}))
            )
            .await,
            StatusCode::FORBIDDEN
        );
    }
    let db = state.db.client().await.unwrap();
    assert_eq!(db.query_one("SELECT (SELECT count(*) FROM platform.community_thread_votes WHERE thread_id=$1 AND user_id=$3)+(SELECT count(*) FROM platform.community_reply_votes WHERE reply_id=$2 AND user_id=$3)", &[&thread,&reply_id,&user_id]).await.unwrap().get::<_,i64>(0),0);
    drop(db);
    for path in [&thread_vote, &reply_vote] {
        let (status, _) = browser
            .request(
                &app,
                Method::POST,
                path,
                Some(json!({"vote":1})),
                Some(ORIGIN),
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            bearer_status(&app, &first_key, Method::DELETE, path, None).await,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT (SELECT count(*) FROM platform.community_thread_votes WHERE thread_id=$1 AND user_id=$3)+(SELECT count(*) FROM platform.community_reply_votes WHERE reply_id=$2 AND user_id=$3)", &[&thread,&reply_id,&user_id]).await.unwrap().get::<_,i64>(0),2);
    for path in [&thread_vote, &reply_vote] {
        let (status, _) = browser
            .request(&app, Method::DELETE, path, None, Some(ORIGIN))
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    for (method, path, payload) in [
        (
            Method::POST,
            replies_path.clone(),
            Some(json!({"body":"Must not persist"})),
        ),
        (
            Method::PUT,
            reply_path.clone(),
            Some(json!({"version":1,"body":"Must not replace"})),
        ),
        (Method::DELETE, reply_path.clone(), None),
        (Method::DELETE, format!("{reply_path}/hard"), None),
    ] {
        assert_eq!(
            bearer_status(&app, &first_key, method, &path, payload).await,
            StatusCode::FORBIDDEN
        );
    }
    let db = state.db.client().await.unwrap();
    let row = db
        .query_one(
            "SELECT body,version,state FROM platform.community_replies WHERE id=$1",
            &[&reply_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "A genuine browser reply");
    assert_eq!(row.get::<_, i32>(1), 1);
    assert_eq!(row.get::<_, String>(2), "active");
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM platform.community_replies WHERE thread_id=$1",
            &[&thread]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    drop(db);
    let (status, _) = browser
        .request(
            &app,
            Method::POST,
            "/api/report",
            Some(json!({
        "entity_type":"reply","entity_id":reply_id.to_string(),"reason_id":"reply_spam",
        "details":"Private report evidence from the actual browser"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, reports) = browser
        .request(&app, Method::GET, "/api/report", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let report = reports["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["body"] == "Private report evidence from the actual browser")
        .unwrap();
    let report_id = report["id"].as_str().unwrap().to_owned();
    state.db.client().await.unwrap().execute("UPDATE platform.users SET roles='[\"moderator\"]'::jsonb,permissions=permissions || '[\"report.update\"]'::jsonb WHERE id=$1", &[&other_id]).await.unwrap();
    let (status, reviewed) = other_owner
        .request(
            &app,
            Method::POST,
            &format!("/api/report/{report_id}/reject"),
            Some(json!({"version":1,"notes":"Private moderator review notes"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reviewed["notes"], "Private moderator review notes");
    let (status, reports) = browser
        .request(&app, Method::GET, "/api/report", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        reports["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == report_id && r["notes"] == "Private moderator review notes")
    );
    assert_eq!(
        bearer_status(&app, &first_key, Method::GET, "/api/report", None).await,
        StatusCode::FORBIDDEN
    );
    for path in [
        "/auth/sessions/whoami",
        "/api/auth/check",
        "/api/client",
        "/api/manga/draft",
        "/api/manga/draft/authdraft1",
        "/api/manga/authdraft1/covers?draft=true",
        "/api/upload",
        "/uploads/authfile01.png",
        "/api/thread/auththread/vote",
    ] {
        assert_eq!(
            bearer_status(&app, &first_key, Method::GET, path, None).await,
            StatusCode::FORBIDDEN,
            "library scope must not authorize {path}"
        );
    }
    assert_eq!(
        bearer_status(
            &app,
            &first_key,
            Method::PUT,
            "/api/settings",
            Some(json!({"version":2,"show_safe":true}))
        )
        .await,
        StatusCode::FORBIDDEN
    );
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.api_clients SET scopes='[]'::jsonb WHERE id=$1",
            &[&client_id],
        )
        .await
        .unwrap();
    assert_eq!(
        bearer_status(
            &app,
            &first_key,
            Method::GET,
            "/api/user/history/manga",
            None
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.api_clients SET scopes='[\"self.library.read\"]'::jsonb WHERE id=$1",
            &[&client_id],
        )
        .await
        .unwrap();
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.users SET roles='[\"user\"]'::jsonb WHERE id=$1",
            &[&user_id],
        )
        .await
        .unwrap();
    let (status, rotated) = browser
        .request(
            &app,
            Method::POST,
            &format!("/api/client/{client_id}/secret"),
            None,
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let next_key = rotated["key"].as_str().unwrap();
    assert_ne!(first_key, next_key);
    assert!(
        weebdex_platform::domains::auth::optional_principal(&state, &bearer)
            .await
            .unwrap()
            .is_none()
    );
    bearer.insert(
        header::AUTHORIZATION,
        format!("Bearer {next_key}").parse().unwrap(),
    );
    assert!(
        weebdex_platform::domains::auth::optional_principal(&state, &bearer)
            .await
            .unwrap()
            .is_some()
    );
    let (status, _) = browser
        .request(
            &app,
            Method::DELETE,
            &format!("/api/client/{client_id}"),
            None,
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        weebdex_platform::domains::auth::optional_principal(&state, &bearer)
            .await
            .unwrap()
            .is_none()
    );
    for kind in ["recovery", "verification"] {
        let mut mail_browser = Browser::default();
        let (status, mailflow) = mail_browser
            .request(
                &app,
                Method::GET,
                &format!("/auth/self-service/{kind}/browser"),
                None,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = mail_browser
            .request(
                &app,
                Method::GET,
                &format!("/auth/self-service/{kind}/flows?id={}", flow_id(&mailflow)),
                None,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let (status,result)=mail_browser.request(&app,Method::POST,&format!("/auth/self-service/{kind}?flow={}",flow_id(&mailflow)),Some(json!({"method":"link","csrf_token":csrf(&mailflow),"email":"first@example.invalid"})),Some(ORIGIN)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            result["ui"]["messages"][0]["text"]
                .as_str()
                .unwrap()
                .contains("not configured")
        );
        assert!(result.get("session").is_none());
    }
    let mut old_session = browser.clone();
    let (status, settings) = browser
        .request(
            &app,
            Method::GET,
            "/auth/self-service/settings/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = browser
        .request(
            &app,
            Method::GET,
            &format!(
                "/auth/self-service/settings/flows?id={}",
                flow_id(&settings)
            ),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status,updated)=browser.request(&app,Method::POST,&format!("/auth/self-service/settings?flow={}",flow_id(&settings)),Some(json!({"method":"profile","csrf_token":csrf(&settings),"traits":{"email":"changed@example.invalid","username":"auth-fixture-updated"}})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(flow_id(&updated), flow_id(&settings));
    let (status, _) = old_session
        .request(&app, Method::GET, "/auth/sessions/whoami", None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, session) = browser
        .request(&app, Method::GET, "/api/auth/check", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["identity"]["is_verified"], false);
    assert_eq!(session["identity"]["email"], "changed@example.invalid");
    let (status,password_flow)=browser.request(&app,Method::POST,&format!("/auth/self-service/settings?flow={}",flow_id(&updated)),Some(json!({"method":"password","csrf_token":csrf(&updated),"password":"isolated-updated-password"})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(password_flow["state"], "success");
    let old_cookie = browser.clone();
    let (status, logout) = browser
        .request(
            &app,
            Method::GET,
            "/auth/self-service/logout/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let token = logout["logout_token"].as_str().unwrap();
    let (status, _) = browser
        .request(
            &app,
            Method::GET,
            &format!("/auth/self-service/logout?token={token}"),
            None,
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = browser
        .request(&app, Method::GET, "/api/auth/check", None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let mut revoked = old_cookie;
    let (status, _) = revoked
        .request(&app, Method::GET, "/auth/sessions/whoami", None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, login) = browser
        .request(
            &app,
            Method::GET,
            "/auth/self-service/login/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status,result)=browser.request(&app,Method::POST,&format!("/auth/self-service/login?flow={}",flow_id(&login)),Some(json!({"method":"password","csrf_token":csrf(&login),"identifier":"auth-fixture-updated","password":"isolated-updated-password"})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["session"]["active"], true);
    let (status,_)=browser.request(&app,Method::POST,&format!("/auth/self-service/login?flow={}",flow_id(&login)),Some(json!({"method":"password","csrf_token":csrf(&login),"identifier":"auth-fixture-updated","password":"isolated-updated-password"})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::GONE);
    let active_id = result["session"]["id"].as_str().unwrap();
    state.db.client().await.unwrap().execute("UPDATE platform.sessions SET created_at=clock_timestamp()-interval '2 days',expires_at=clock_timestamp()-interval '1 second' WHERE id=$1",&[&active_id]).await.unwrap();
    let (status, _) = browser
        .request(&app, Method::GET, "/auth/sessions/whoami", None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let mut limited = Browser::default();
    let (status, flow) = limited
        .request(
            &app,
            Method::GET,
            "/auth/self-service/login/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    for attempt in 0..9 {
        let (status,_)=limited.request(&app,Method::POST,&format!("/auth/self-service/login?flow={}",flow_id(&flow)),Some(json!({"method":"password","csrf_token":csrf(&flow),"identifier":"limited@example.invalid","password":FIXTURE_PASSWORD})),Some(ORIGIN)).await;
        assert_eq!(
            status,
            if attempt < 8 {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    // All 18 SDK methods are exercised here, including configured-provider
    // absence as an explicit mail failure. The separate delivery test below
    // establishes successful verification/recovery behavior through a real sink.
}

#[derive(Clone)]
struct MailSink {
    messages: std::sync::Arc<tokio::sync::Mutex<Vec<Value>>>,
    reject: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
async fn capture_mail(
    axum::extract::State(sink): axum::extract::State<MailSink>,
    axum::Json(message): axum::Json<Value>,
) -> StatusCode {
    assert!(
        message["to"]
            .as_str()
            .unwrap()
            .ends_with("@example.invalid")
    );
    sink.messages.lock().await.push(message);
    if sink.reject.load(std::sync::atomic::Ordering::SeqCst) {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::ACCEPTED
    }
}
struct OwnedMailServer(tokio::task::JoinHandle<()>);
impl Drop for OwnedMailServer {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn mail_link(
    app: &Router,
    browser: &mut Browser,
    sink: &MailSink,
    kind: &str,
    email: &str,
) -> (StatusCode, Value, String) {
    let (status, flow) = browser
        .request(
            app,
            Method::GET,
            &format!("/auth/self-service/{kind}/browser"),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = browser
        .request(
            app,
            Method::POST,
            &format!("/auth/self-service/{kind}?flow={}", flow_id(&flow)),
            Some(json!({"method":"link","csrf_token":csrf(&flow),"email":email})),
            Some(ORIGIN),
        )
        .await;
    // Tokens are visible only to this reserved synthetic delivery recipient,
    // never in the unauthenticated API response or test output.
    let messages = sink.messages.lock().await;
    let text = messages.last().unwrap()["text"].as_str().unwrap();
    let link = text
        .lines()
        .find(|line| line.starts_with(ORIGIN))
        .unwrap()
        .to_owned();
    let parsed = url::Url::parse(&link).unwrap();
    (
        status,
        body,
        format!("{}?{}", parsed.path(), parsed.query().unwrap()),
    )
}

#[tokio::test]
#[ignore = "Requires owned isolated PostgreSQL cluster; use scripts/run-auth-tests.sh"]
async fn actual_email_delivery_one_time_verification_and_restricted_recovery() {
    let sink = MailSink {
        messages: Default::default(),
        reject: Default::default(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let sink_router = Router::new()
        .route("/messages", axum::routing::post(capture_mail))
        .with_state(sink.clone());
    let _server = OwnedMailServer(tokio::spawn(async move {
        axum::serve(listener, sink_router).await.unwrap();
    }));
    let mut state = isolated_state().await;
    let mut config = (*state.config).clone();
    config.auth_mail = Some(weebdex_platform::domains::auth::MailConfig {
        endpoint: format!("http://{address}/messages"),
        from: "weebdex-test@example.invalid".into(),
        bearer_token_env: None,
        allow_loopback_test_sink: true,
    });
    config.validate().unwrap();
    state.config = std::sync::Arc::new(config);
    let app = weebdex_platform::router::router(state.clone());
    let mut browser = Browser::default();
    let registered = registration(
        &app,
        &mut browser,
        "mail-fixture@example.invalid",
        "mail-fixture-user",
    )
    .await;
    let user_id = registered["session"]["identity"]["id"].as_str().unwrap();
    let mut mail_browser = Browser::default();
    let (status, body, verification_link) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "verification",
        "mail-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state"], "sent_email");
    assert!(body.get("token").is_none());
    assert!(!body.to_string().contains("?token="));
    let (status, _) = mail_browser
        .request(&app, Method::GET, &verification_link, None, None)
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (status, session) = browser
        .request(&app, Method::GET, "/api/auth/check", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["identity"]["is_verified"], true);
    let (status, private_client) = browser
        .request(
            &app,
            Method::POST,
            "/api/client",
            Some(json!({"name":"Recovery private client fixture"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let private_client_id = private_client["id"].as_str().unwrap().to_owned();
    let (status, clients) = browser
        .request(&app, Method::GET, "/api/client", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        clients
            .as_array()
            .unwrap()
            .iter()
            .any(|client| client["id"] == private_client_id)
    );
    // Seed public targets with a different genuine browser account, then create
    // real personal state through the normal owner Router before recovery.
    let mut discussion_owner = Browser::default();
    let discussion_user = registration(
        &app,
        &mut discussion_owner,
        "mail-discussion@example.invalid",
        "mail-discussion-user",
    )
    .await;
    let discussion_id = discussion_user["session"]["identity"]["id"]
        .as_str()
        .unwrap();
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.users SET verified=TRUE WHERE id=$1",
            &[&discussion_id],
        )
        .await
        .unwrap();
    let (status, group) = discussion_owner
        .request(
            &app,
            Method::POST,
            "/api/group",
            Some(json!({"name":"Recovery personal projection fixture"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let thread = group["relationships"]["thread"]["id"].as_str().unwrap();
    let replies_path = format!("/api/thread/{thread}/replies");
    let (status, reply) = discussion_owner
        .request(
            &app,
            Method::POST,
            &replies_path,
            Some(json!({"body":"Public discussion, private viewer vote"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let reply_path = format!("{replies_path}/{}", reply["id"].as_i64().unwrap());
    let (status, _) = browser
        .request(
            &app,
            Method::POST,
            &format!("{reply_path}/vote"),
            Some(json!({"vote":1})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, personal_reply) = browser
        .request(&app, Method::GET, &reply_path, None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(personal_reply["relationships"]["vote_state"], 1);
    state.db.client().await.unwrap().execute(
        "INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state) VALUES('MailRead01',$1,'mail-fixture-user','Recovery reading filter fixture','en','ongoing','safe','published')",
        &[&user_id],
    ).await.unwrap();
    let (status, _) = browser
        .request(
            &app,
            Method::POST,
            "/api/user/follows/manga/MailRead01",
            Some(json!({"status":"reading"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let personal_filters = [
        "/api/manga?reading_status=reading&id=MailRead01",
        "/api/manga/random?reading_status=reading&id=MailRead01",
    ];
    for path in personal_filters {
        assert_eq!(
            browser.request(&app, Method::GET, path, None, None).await.0,
            StatusCode::OK
        );
    }
    let (status, _) = mail_browser
        .request(&app, Method::GET, &verification_link, None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body, unknown_link) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "verification",
        "no-account@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state"], "sent_email");
    let (status, _) = mail_browser
        .request(&app, Method::GET, &unknown_link, None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, expired_link) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "verification",
        "mail-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    state.db.client().await.unwrap().execute("UPDATE platform.auth_email_tokens SET created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 second' WHERE user_id=$1 AND kind='verification' AND used_at IS NULL",&[&user_id]).await.unwrap();
    let (status, _) = mail_browser
        .request(&app, Method::GET, &expired_link, None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    sink.reject.store(true, std::sync::atomic::Ordering::SeqCst);
    let (status, _, failed_link) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "recovery",
        "mail-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, _) = mail_browser
        .request(&app, Method::GET, &failed_link, None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    sink.reject
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let (status, _, recovery_link) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "recovery",
        "mail-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut recovered = Browser::default();
    let (status, _) = recovered
        .request(&app, Method::GET, &recovery_link, None, None)
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (status, session) = recovered
        .request(&app, Method::GET, "/api/auth/check", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["permissions"], json!([]));
    assert_eq!(session["session_purpose"], "recovery");
    assert_eq!(session["recovery"]["can_reset_password"], true);
    for field in ["identity", "follows", "preferences", "groups"] {
        assert!(
            session.get(field).is_none(),
            "Recovery must not project {field}"
        );
    }
    // All private auth/account GETs deny the genuine limited recovery cookie,
    // even though the same still-active normal browser can list its client.
    for path in [
        "/api/client",
        "/api/user/follows/manga",
        "/auth/sessions/whoami",
    ] {
        let (status, denied) = recovered.request(&app, Method::GET, path, None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!denied.to_string().contains(&private_client_id));
        assert!(
            !denied
                .to_string()
                .contains("Recovery private client fixture")
        );
    }
    let (status, clients) = browser
        .request(&app, Method::GET, "/api/client", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        clients
            .as_array()
            .unwrap()
            .iter()
            .any(|client| client["id"] == private_client_id)
    );
    // This is the actual limited cookie minted by mail-token redemption, not
    // a synthesized principal. Public reads remain anonymous projections.
    for path in [&reply_path, &replies_path] {
        let (status, public) = recovered.request(&app, Method::GET, path, None, None).await;
        assert_eq!(status, StatusCode::OK);
        let projected = if path == &reply_path {
            &public
        } else {
            &public["data"][0]
        };
        assert_eq!(projected["relationships"]["stats"]["up"], 1);
        assert!(projected["relationships"].get("vote_state").is_none());
    }
    for path in personal_filters {
        assert_eq!(
            recovered
                .request(&app, Method::GET, path, None, None)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        recovered
            .request(&app, Method::GET, "/api/manga?id=MailRead01", None, None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, _) = recovered
        .request(&app, Method::GET, "/api/user/lists", None, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = recovered
        .request(
            &app,
            Method::POST,
            "/api/client",
            Some(json!({"name":"Recovery may not mint credentials"})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, settings) = recovered
        .request(
            &app,
            Method::GET,
            "/auth/self-service/settings/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        settings["ui"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|node| matches!(
                node["attributes"]["name"].as_str(),
                Some("csrf_token" | "password")
            ))
    );
    let path = format!("/auth/self-service/settings?flow={}", flow_id(&settings));
    let (status,_)=recovered.request(&app,Method::POST,&path,Some(json!({"method":"profile","csrf_token":csrf(&settings),"traits":{"email":"mail-fixture@example.invalid","username":"must-not-change"}})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status,_)=recovered.request(&app,Method::POST,&path,Some(json!({"method":"password","csrf_token":csrf(&settings),"password":"recovered-fixture-password"})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, session) = recovered
        .request(&app, Method::GET, "/api/auth/check", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        session["permissions"]
            .as_array()
            .unwrap()
            .contains(&json!("client.create"))
    );
    let (status, _) = browser
        .request(&app, Method::GET, "/auth/sessions/whoami", None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = mail_browser
        .request(&app, Method::GET, &recovery_link, None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, decoy_recovery) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "recovery",
        "no-account@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = mail_browser
        .request(&app, Method::GET, &decoy_recovery, None, None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut login_browser = Browser::default();
    let (status, login) = login_browser
        .request(
            &app,
            Method::GET,
            "/auth/self-service/login/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status,result)=login_browser.request(&app,Method::POST,&format!("/auth/self-service/login?flow={}",flow_id(&login)),Some(json!({"method":"password","csrf_token":csrf(&login),"identifier":"mail-fixture-user","password":"recovered-fixture-password"})),Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["session"]["active"], true);
    let hash_rows=state.db.client().await.unwrap().query("SELECT octet_length(token_hash),octet_length(recipient_hash),delivery_state FROM platform.auth_email_tokens",&[]).await.unwrap();
    assert!(!hash_rows.is_empty());
    for row in hash_rows {
        assert_eq!(row.get::<_, i32>(0), 32);
        assert_eq!(row.get::<_, i32>(1), 32);
    }
}

async fn rotate_while_client_issuance_waits(
    state: &AppState,
    app: &Router,
    user_id: &str,
    stale_browser: Browser,
    rotating: &mut Browser,
    settings: &Value,
    password: &str,
) {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(format!("api-client:create:{user_id}").as_bytes()).to_vec();
    let mut owner = state.db.client().await.unwrap();
    owner.execute("INSERT INTO platform.auth_rate_limits(key_hash,window_ends,count) VALUES($1,clock_timestamp()+interval '5 minutes',1) ON CONFLICT DO NOTHING",&[&hash]).await.unwrap();
    let gate = owner.transaction().await.unwrap();
    gate.query_one(
        "SELECT count FROM platform.auth_rate_limits WHERE key_hash=$1 FOR UPDATE",
        &[&hash],
    )
    .await
    .unwrap();
    let stale_app = app.clone();
    let creation = tokio::spawn(async move {
        let mut stale_browser = stale_browser;
        stale_browser
            .request(
                &stale_app,
                Method::POST,
                "/api/client",
                Some(json!({"name":"Cannot outlive credential rotation"})),
                Some(ORIGIN),
            )
            .await
    });
    // Paused after actual cookie resolution but before account admission, using
    // the real throttle-row lock. The credential-changing Router stays runnable.
    let control = state.db.client().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let waiting:i64=control.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'INSERT INTO platform.auth_rate_limits%'",&[]).await.unwrap().get(0);
            if waiting > 0 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("Actual client creation must wait at the throttle boundary");
    drop(control);
    let (status, _) = rotating
        .request(
            app,
            Method::POST,
            &format!("/auth/self-service/settings?flow={}", flow_id(settings)),
            Some(json!({"method":"password","csrf_token":csrf(settings),"password":password})),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    gate.commit().await.unwrap();
    drop(owner);
    let (status, body) = creation.await.unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.get("key").is_none());
    assert_eq!(
        state
            .db
            .client()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM platform.api_clients WHERE user_id=$1 AND revoked_at IS NULL",
                &[&user_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
#[ignore = "Requires owned disposable PostgreSQL and loopback mail sink; scripts/run-auth-tests.sh"]
async fn password_rotation_revokes_outstanding_recovery_links_and_overlapping_redemption() {
    let sink = MailSink {
        messages: Default::default(),
        reject: Default::default(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let capture = Router::new()
        .route("/messages", axum::routing::post(capture_mail))
        .with_state(sink.clone());
    let _server = OwnedMailServer(tokio::spawn(async move {
        axum::serve(listener, capture).await.unwrap();
    }));
    let mut state = isolated_state().await;
    let mut config = (*state.config).clone();
    config.auth_mail = Some(weebdex_platform::domains::auth::MailConfig {
        endpoint: format!("http://{address}/messages"),
        from: "rotation-test@example.invalid".into(),
        bearer_token_env: None,
        allow_loopback_test_sink: true,
    });
    config.validate().unwrap();
    state.config = std::sync::Arc::new(config);
    let app = weebdex_platform::router::router(state.clone());
    let mut original = Browser::default();
    let registered = registration(
        &app,
        &mut original,
        "rotation-fixture@example.invalid",
        "rotation-fixture-user",
    )
    .await;
    let user_id = registered["session"]["identity"]["id"].as_str().unwrap();
    state
        .db
        .client()
        .await
        .unwrap()
        .execute(
            "UPDATE platform.users SET verified=TRUE WHERE id=$1",
            &[&user_id],
        )
        .await
        .unwrap();
    let mut mail_browser = Browser::default();
    let (status, _, link_a) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "recovery",
        "rotation-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, link_b) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "recovery",
        "rotation-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut recovered = Browser::default();
    assert_eq!(
        recovered
            .request(&app, Method::GET, &link_a, None, None)
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    let (status, settings) = recovered
        .request(
            &app,
            Method::GET,
            "/auth/self-service/settings/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    rotate_while_client_issuance_waits(
        &state,
        &app,
        user_id,
        original.clone(),
        &mut recovered,
        &settings,
        "first-rotated-fixture-password",
    )
    .await;
    let mut stale = Browser::default();
    assert_eq!(
        stale
            .request(&app, Method::GET, &link_b, None, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        stale
            .request(&app, Method::GET, "/api/auth/check", None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // A newly requested link works after reset, but the next ordinary password
    // change invalidates it too. It cannot reclaim the account afterward.
    let (status, _, link_c) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "recovery",
        "rotation-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, settings) = recovered
        .request(
            &app,
            Method::GET,
            "/auth/self-service/settings/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    rotate_while_client_issuance_waits(
        &state,
        &app,
        user_id,
        recovered.clone(),
        &mut recovered,
        &settings,
        "second-rotated-fixture-password",
    )
    .await;
    assert_eq!(
        stale
            .request(&app, Method::GET, &link_c, None, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (status, _, link_d) = mail_link(
        &app,
        &mut mail_browser,
        &sink,
        "recovery",
        "rotation-fixture@example.invalid",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, settings) = recovered
        .request(
            &app,
            Method::GET,
            "/auth/self-service/settings/browser",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    // Hold the real account row so both HTTP operations overlap at their common
    // serialization boundary. No production test hook or sleep controls policy.
    let mut owner = state.db.client().await.unwrap();
    let lock = owner.transaction().await.unwrap();
    lock.query_one(
        "SELECT id FROM platform.users WHERE id=$1 FOR UPDATE",
        &[&user_id],
    )
    .await
    .unwrap();
    let change_app = app.clone();
    let change = tokio::spawn(async move {
        let result = recovered.request(&change_app, Method::POST, &format!("/auth/self-service/settings?flow={}", flow_id(&settings)),
            Some(json!({"method":"password","csrf_token":csrf(&settings),"password":"overlap-rotated-fixture-password"})), Some(ORIGIN)).await;
        (recovered, result)
    });
    let redeem_app = app.clone();
    let redeem = tokio::spawn(async move {
        let mut browser = Browser::default();
        let result = browser
            .request(&redeem_app, Method::GET, &link_d, None, None)
            .await;
        (browser, result)
    });
    let control = state.db.client().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let waiting:i64=control.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND (query LIKE 'SELECT email,disabled FROM platform.users%' OR query LIKE 'SELECT id FROM platform.users%')",&[]).await.unwrap().get(0);
            if waiting >= 2 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("Both genuine HTTP requests must reach the account lock");
    drop(control);
    lock.commit().await.unwrap();
    drop(owner);
    let (mut current, result) = change.await.unwrap();
    assert_eq!(result.0, StatusCode::OK);
    let (mut candidate, result) = redeem.await.unwrap();
    assert!(matches!(
        result.0,
        StatusCode::SEE_OTHER | StatusCode::BAD_REQUEST
    ));
    // Regardless of who won the lock, no pre-change link/session survives the
    // successful rotation. A freshly issued normal cookie still works.
    assert_eq!(
        candidate
            .request(&app, Method::GET, "/api/auth/check", None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        current
            .request(&app, Method::GET, "/api/auth/check", None, None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(state.db.client().await.unwrap().query_one("SELECT count(*) FROM platform.auth_email_tokens WHERE user_id=$1 AND kind='recovery' AND used_at IS NULL",&[&user_id]).await.unwrap().get::<_,i64>(0),0);
}
