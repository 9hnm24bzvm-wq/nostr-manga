//! Explicit versioned route boundary from the independently hash-pinned inventory.
//! Known unsupported methods are truthful501, wrongmethods405, unknownpaths404.
use super::errors::PublicApiError;
use axum::{
    Router,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{MethodFilter, MethodRouter},
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Inventory {
    operations: Vec<Operation>,
}
#[derive(Deserialize)]
struct Operation {
    method: String,
    path: String,
}
const INVENTORY: &str = include_str!("../../../contracts/mangadex-5.13.1-inventory.json");

pub fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    let inventory: Inventory =
        serde_json::from_str(INVENTORY).expect("checked-in derived API inventory");
    let mut paths = BTreeMap::<String, MethodRouter<S>>::new();
    for operation in inventory.operations {
        if operation.method == "GET" && operation.path == "/auth/check" {
            continue;
        }
        let route = paths.entry(axum_path(&operation.path)).or_default();
        let filter = match operation.method.as_str() {
            "GET" => MethodFilter::GET,
            "POST" => MethodFilter::POST,
            "PUT" => MethodFilter::PUT,
            "DELETE" => MethodFilter::DELETE,
            "PATCH" => MethodFilter::PATCH,
            "HEAD" => MethodFilter::HEAD,
            "OPTIONS" => MethodFilter::OPTIONS,
            _ => panic!("unsupported inventory method"),
        };
        *route = std::mem::take(route).on(filter, unsupported);
    }
    let mut router = Router::new().fallback(unknown);
    for (path, route) in paths {
        router = router.route(&path, route.fallback(wrong_method));
    }
    router
}
/// Normalize parameter names at common dynamic prefixes so one Axum match tree
/// never receives different names for the same wildcard position.
fn axum_path(path: &str) -> String {
    let mut parameter = 0;
    path.split('/')
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') {
                let value = format!("{{p{parameter}}}");
                parameter += 1;
                value
            } else {
                segment.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}
async fn unsupported() -> PublicApiError {
    PublicApiError::unsupported()
}
async fn unknown() -> PublicApiError {
    PublicApiError::not_found_detail("Unknown public API endpoint")
}
async fn wrong_method(method: Method) -> impl IntoResponse {
    let _ = method;
    PublicApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "Method Not Allowed",
        "Method is not supported for this endpoint",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn inventory_is_bounded_and_fallbacks_are_honest_json() {
        let app = Router::<()>::new().nest("/api/v1", router());
        for (method, path, status) in [
            ("GET", "/api/v1/manga", 501),
            ("POST", "/api/v1/auth/login", 501),
            ("GET", "/api/v1/unknown", 404),
            ("PATCH", "/api/v1/manga", 405),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header("accept", "text/html")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status, "{method} {path}");
            assert_eq!(response.headers()["cache-control"], "no-store");
            let value: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap())
                    .unwrap();
            assert_eq!(value["result"], "error");
            assert!(value["errors"].is_array());
        }
    }
    #[test]
    fn inherited_draft_parameters_are_retained() {
        let inventory: serde_json::Value = serde_json::from_str(INVENTORY).unwrap();
        assert_eq!(inventory["operation_count"], 113);
        let operations = inventory["operations"].as_array().unwrap();
        let detail = operations
            .iter()
            .find(|o| o["method"] == "GET" && o["path"] == "/manga/draft/{id}")
            .unwrap();
        assert!(
            detail["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["name"] == "includes[]")
        );
        let list = operations
            .iter()
            .find(|o| o["method"] == "GET" && o["path"] == "/manga/draft")
            .unwrap();
        for name in ["limit", "offset", "state", "order", "includes[]"] {
            assert!(
                list["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == name),
                "missing inherited/operation parameter {name}"
            );
        }
    }
}
