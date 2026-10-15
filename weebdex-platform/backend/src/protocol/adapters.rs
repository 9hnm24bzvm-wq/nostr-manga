//! Browser-selected signer adapters. Native accounts confer no original signer
//! authority; the original HTTP/WS module owns signed publication and import.
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use manga_node::auth;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/protocol/capabilities", get(capabilities))
        .route("/protocol/resources", post(resources))
}
fn configured_service(state: &AppState) -> (Option<String>, Option<String>, bool) {
    let Some(config) = state.config.protocol_publication.as_ref() else {
        return (None, None, false);
    };
    let Some(node) = config.native_node.as_ref() else {
        return (None, None, false);
    };
    let origin = node
        .get("base_url")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let key = auth::valid_hash(&config.storage_holder).then(|| config.storage_holder.clone());
    let configured =
        origin.is_some() && key.is_some() && node.get("backend").is_some_and(|v| !v.is_null());
    (origin, key, configured)
}
async fn capabilities(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let (origin, key, configured) = configured_service(&state);
    let namespace: String = state
        .db
        .client()
        .await?
        .query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",
            &[],
        )
        .await?
        .get(0);
    let mut connections = state.config.protocol_connect_origins.clone();
    if let Some(origin) = &origin {
        connections.push(origin.clone());
    }
    let connections = crate::privacy::approved_connect_origins(&connections);
    let cors = state
        .config
        .protocol_publication
        .as_ref()
        .and_then(|config| config.native_node.as_ref())
        .and_then(|node| node.get("cors_origins"))
        .and_then(Value::as_array)
        .is_some_and(|origins| {
            origins
                .iter()
                .any(|origin| origin.as_str() == Some(state.config.public_origin.as_str()))
        });
    let same_origin = origin
        .as_ref()
        .is_some_and(|origin| origin == &state.config.public_origin);
    let browser_transport_allowed = configured && (same_origin || cors);
    Ok(Json(
        json!({"catalog_namespace":namespace,"schema_ready":state.protocol.is_ready(),"origin":origin,"service_key":key,"configured":configured,"browser_transport_allowed":browser_transport_allowed,"available":browser_transport_allowed&&state.protocol.is_ready()&&state.runtime.is_ready()&&!state.runtime.is_draining(),"external_signer_required":false,"supported_signer_modes":["local-session","nip07"],"account_admission_required":true,"native_account_is_signer":false,"connect_origins":connections,"paths":{"libraries":"/libraries","library_batches":"/library/batches","library_read":"/library/read","library_objects":"/library/objects","library_blobs":"/library/blobs/{hash}","library_staging":"/library/staging/{hash}","account_metadata":"/account-metadata","account_heads":"/accounts/{key}/metadata/{kind}","social":"/social","account_deletions":"/account-deletions","service_announcement":"/service/announcement"}}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceRequest {
    resources: Vec<ResourceId>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceId {
    kind: String,
    id: String,
}
fn validate_request(request: &ResourceRequest) -> ApiResult<()> {
    if request.resources.len() > 256 {
        return Err(ApiError::bad_request(
            "Resource mapping accepts at most 256 IDs",
        ));
    }
    let mut seen = BTreeSet::new();
    for resource in &request.resources {
        if !matches!(resource.kind.as_str(), "manga" | "chapter")
            || resource.id.len() != 10
            || !resource.id.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err(ApiError::bad_request("Invalid public resource mapping"));
        }
        if !seen.insert((&resource.kind, &resource.id)) {
            return Err(ApiError::bad_request("Duplicate resource mapping"));
        }
    }
    Ok(())
}
async fn resources(
    State(state): State<AppState>,
    Json(request): Json<ResourceRequest>,
) -> ApiResult<Json<Value>> {
    validate_request(&request)?;
    let _read = state
        .reads
        .acquire(manga_node::read_admission::Class::Catalogue)
        .await
        .map_err(|_| ApiError::busy())?;
    if !state.protocol.is_ready() {
        return Err(ApiError::unavailable());
    }
    let mangas: Vec<String> = request
        .resources
        .iter()
        .filter(|r| r.kind == "manga")
        .map(|r| r.id.clone())
        .collect();
    let chapters: Vec<String> = request
        .resources
        .iter()
        .filter(|r| r.kind == "chapter")
        .map(|r| r.id.clone())
        .collect();
    let mut client = state.db.client().await?;
    let tx = client.transaction().await?;
    let namespace: String = tx
        .query_one(
            "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton",
            &[],
        )
        .await?
        .get(0);
    tx.batch_execute("SET LOCAL statement_timeout='2000ms'; SET LOCAL lock_timeout='500ms'")
        .await?;
    let rows=tx.query("SELECT 'manga'::text,m.id,m.protocol_series_id FROM platform.catalog_manga m WHERE m.id=ANY($1) AND m.state='published' AND m.deleted_at IS NULL AND m.protocol_source_visible AND m.protocol_series_id IS NOT NULL AND platform.protocol_source_is_visible(m.protocol_series_id) UNION ALL SELECT 'chapter',c.id,c.protocol_event_id FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=ANY($2) AND c.deleted_at IS NULL AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND c.protocol_event_id IS NOT NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&mangas,&chapters]).await?;
    let mut local = Vec::new();
    for row in rows {
        let event: String = row.get(2);
        if !auth::valid_hash(&event) {
            return Err(ApiError::unavailable());
        }
        let id: [u8; 32] = hex::decode(&event)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(ApiError::unavailable)?;
        local.push((row.get::<_, String>(0), row.get::<_, String>(1), event, id));
    }
    let ids: Vec<[u8; 32]> = local.iter().map(|(_, _, _, id)| *id).collect();
    let verified = manga_node::platform_bridge::visible_resource_ids(&*tx, &ids)
        .await
        .map_err(|_| ApiError::unavailable())?;
    let verified: BTreeMap<String, _> = verified
        .into_iter()
        .map(|r| (r.event.id.clone(), r))
        .collect();
    let mut data = Vec::new();
    for (kind, id, event, _) in local {
        let Some(source) = verified.get(&event) else {
            continue;
        };
        if (kind == "manga" && source.event.kind != 1002)
            || (kind == "chapter" && source.event.kind != 1001)
        {
            return Err(ApiError::unavailable());
        }
        data.push(json!({"kind":kind,"id":id,"event_id":event,"publisher":source.event.pubkey,"source_kind":source.event.kind,"compatibility_id":source.compatibility_id}));
    }
    data.sort_by_key(|row| {
        (
            row["kind"].as_str().unwrap().to_owned(),
            row["id"].as_str().unwrap().to_owned(),
        )
    });
    tx.commit().await?;
    let (origin, key, _) = configured_service(&state);
    Ok(Json(
        json!({"data":data,"origin":origin,"service_key":key,"catalog_namespace":namespace}),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bounded_explicit_native_resource_mappings_are_accepted() {
        let request = |kind: &str, id: &str| ResourceRequest {
            resources: vec![ResourceId {
                kind: kind.into(),
                id: id.into(),
            }],
        };
        assert!(validate_request(&request("manga", "Abcdef1234")).is_ok());
        for (kind, id) in [
            ("user", "Abcdef1234"),
            ("manga", "../../secr"),
            ("chapter", "ab"),
            ("chapter", "https://peer.invalid/"),
        ] {
            assert!(validate_request(&request(kind, id)).is_err());
        }
        let repeated = ResourceRequest {
            resources: vec![
                ResourceId {
                    kind: "manga".into(),
                    id: "Abcdef1234".into(),
                },
                ResourceId {
                    kind: "manga".into(),
                    id: "Abcdef1234".into(),
                },
            ],
        };
        assert!(validate_request(&repeated).is_err());
        let oversized = ResourceRequest {
            resources: (0..257)
                .map(|_| ResourceId {
                    kind: "chapter".into(),
                    id: "Abcdef1234".into(),
                })
                .collect(),
        };
        assert!(validate_request(&oversized).is_err());
    }
}
