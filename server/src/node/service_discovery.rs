//! Signed announcement transport. Cached metadata never changes peer permissions.
use super::*;
use crate::service_announcement::Announcement;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoints {
    endpoints: Vec<String>,
}
pub(super) async fn publish(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<auth::Event>, ApiError> {
    let body = operator_json(&a, &uri, &headers, request).await?;
    let requested: Endpoints =
        serde_json::from_value(body).map_err(|_| bad("endpoint array required"))?;
    let candidate =
        Announcement::signed(&a.signer.secret, requested.endpoints.clone(), auth::now())
            .map_err(|_| bad("invalid endpoint announcement"))?;
    let record = a
        .repo
        .publish_service_endpoints(&a.signer.secret, candidate.endpoints().to_vec())
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(record.event().clone()))
}
pub(super) async fn own(State(a): State<Shared>) -> Result<Json<auth::Event>, ApiError> {
    cached_record(&a, id(&a.signer.public).map_err(|_| unavailable())?).await
}
async fn cached_record(a: &App, key: Id) -> Result<Json<auth::Event>, ApiError> {
    let record = a
        .repo
        .service_announcement(key)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"service_announcement_unknown"})),
            )
        })?;
    Ok(Json(record.event().clone()))
}
pub(super) async fn cached(
    State(a): State<Shared>,
    Path(key): Path<String>,
) -> Result<Json<auth::Event>, ApiError> {
    cached_record(&a, id(&key).map_err(|_| bad("invalid service key"))?).await
}
pub(super) async fn peer_cached(
    State(a): State<Shared>,
    Path(key): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<Json<auth::Event>, ApiError> {
    peer_authorized(&a, &uri, &headers).await?;
    cached_record(&a, id(&key).map_err(|_| bad("invalid service key"))?).await
}
pub(super) async fn cache(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let body = operator_json(&a, &uri, &headers, request).await?;
    let event: auth::Event =
        serde_json::from_value(body).map_err(|_| bad("signed event required"))?;
    let record = Announcement::parse(event)
        .map_err(|_| bad("invalid or unsupported service announcement"))?;
    let outcome = a
        .repo
        .cache_service_announcement(&record)
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(
        json!({"id":record.event().id,"service_pubkey":record.event().pubkey,"outcome":outcome,"peer_permissions_granted":false}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fetch {
    seed_endpoint: String,
    expected_seed_key: Option<String>,
    service_key: String,
    #[serde(default)]
    peer_authenticated: bool,
}
pub(super) async fn fetch(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let body = operator_json(&a, &uri, &headers, request).await?;
    let request: Fetch = serde_json::from_value(body)
        .map_err(|_| bad("seed endpoint and target service key required"))?;
    let _slot = a.discovery_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"discovery_busy"})),
        )
    })?;
    Ok(Json(fetch_inner(&a, request).await?))
}
async fn fetch_inner(a: &Shared, request: Fetch) -> Result<Value, ApiError> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let target = id(&request.service_key).map_err(|_| bad("invalid target service key"))?;
    let origin = crate::endpoint_policy::canonical_origin(&request.seed_endpoint)
        .map_err(|_| bad("invalid seed endpoint"))?;
    let remembered = a
        .repo
        .remembered_service_key(&origin)
        .await
        .map_err(|_| unavailable())?;
    let expected = request
        .expected_seed_key
        .as_deref()
        .map(id)
        .transpose()
        .map_err(|_| bad("invalid seed service key"))?;
    if matches!((remembered,expected),(Some(old),Some(new)) if old!=new) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error":"remembered_service_key_changed"})),
        ));
    }
    let seed = a
        .endpoint_policy
        .connect(&origin, remembered.or(expected))
        .await
        .map_err(|_| unavailable())?;
    a.repo
        .remember_service_identity(&seed)
        .await
        .map_err(|_| unavailable())?;
    let bytes = if request.peer_authenticated {
        a.repo
            .approved_peer(seed.service_key())
            .await
            .map_err(|_| forbidden("seed peer not approved"))?;
        let path = format!("/federation/service-announcements/{}", request.service_key);
        let authorization = auth::sign(
            &a.signer.secret,
            27235,
            vec![
                vec!["u".into(), format!("{origin}{path}")],
                vec!["method".into(), "GET".into()],
                vec!["nonce".into(), hex::encode(rand::random::<[u8; 32]>())],
            ],
            String::new(),
        )
        .map_err(|_| unavailable())?;
        let token = format!(
            "Nostr {}",
            STANDARD.encode(serde_json::to_vec(&authorization).map_err(|_| unavailable())?)
        );
        seed.get_authorized_metadata(&path, &token)
            .await
            .map_err(|_| unavailable())?
    } else {
        seed.get_metadata(&format!("/service/announcements/{}", request.service_key))
            .await
            .map_err(|_| unavailable())?
    };
    let record = Announcement::parse(
        serde_json::from_slice(&bytes).map_err(|_| bad("invalid seed response"))?,
    )
    .map_err(|_| bad("invalid or unsupported signed announcement"))?;
    if record.service_key() != target {
        return Err(bad("seed returned another service identity"));
    }
    let outcome = a
        .repo
        .cache_service_announcement(&record)
        .await
        .map_err(|_| unavailable())?;
    let latest = a
        .repo
        .service_announcement(target)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    Ok(
        json!({"outcome":outcome,"event":latest.event(),"withdrawn":latest.endpoints().is_empty(),"peer_permissions_granted":false,"peer_endpoints_changed":false}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Seed {
    endpoint: String,
    expected_key: Option<String>,
    #[serde(default)]
    peer_authenticated: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Discover {
    service_key: String,
    #[serde(default)]
    seeds: Vec<Seed>,
}
pub(super) async fn discover(
    State(a): State<Shared>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, ApiError> {
    let body = operator_json(&a, &uri, &headers, request).await?;
    let request: Discover =
        serde_json::from_value(body).map_err(|_| bad("service key and seed array required"))?;
    let target = id(&request.service_key).map_err(|_| bad("invalid target service key"))?;
    if request.seeds.len() > 8 {
        return Err(bad("at most eight discovery seeds"));
    }
    let _slot = a.discovery_slots.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"discovery_busy"})),
        )
    })?;
    tokio::time::timeout(Duration::from_secs(30),async {
        let mut outcomes=Vec::new();
        for (index,seed) in request.seeds.into_iter().enumerate() {
            let result=fetch_inner(&a,Fetch{seed_endpoint:seed.endpoint,expected_seed_key:seed.expected_key,service_key:request.service_key.clone(),peer_authenticated:seed.peer_authenticated}).await;
            outcomes.push(match result {Ok(value)=>json!({"seed":index,"ok":true,"outcome":value["outcome"]}),Err((status,_))=>json!({"seed":index,"ok":false,"status":status.as_u16()})});
        }
        let record=a.repo.service_announcement(target).await.map_err(|_|unavailable())?.ok_or_else(||(StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":"announcement_unknown_or_unavailable","seed_outcomes":outcomes}))))?;
        if record.endpoints().is_empty(){return Err((StatusCode::GONE,Json(json!({"error":"service_endpoints_withdrawn","announcement_id":record.event().id,"seed_outcomes":outcomes,"permanent_key_deletion":false}))))}
        for (index,endpoint) in record.endpoints().iter().enumerate() {
            let Ok(verified)=a.endpoint_policy.connect(endpoint,Some(target)).await else {continue};
            let first=match a.repo.remember_service_identity(&verified).await {
                Ok(first)=>first,
                Err(error) if error.is::<crate::service_identity::IdentityChanged>()=>continue,
                Err(_)=>return Err(unavailable()),
            };
            let current=a.repo.service_announcement(target).await.map_err(|_|unavailable())?.ok_or_else(unavailable)?;
            if current.event().id!=record.event().id {return Err((StatusCode::CONFLICT,Json(json!({"error":"announcement_changed_retry"}))))}
            return Ok(Json(json!({"service_pubkey":request.service_key,"endpoint":verified.origin(),"announcement_id":record.event().id,"first_observation":first,"seed_outcomes":outcomes,"attempted_endpoints":index+1,"peer_permissions_granted":false,"peer_endpoints_changed":false,"freshness":"latest_supported_announcement_observed_not_global_consensus"})));
        }
        Err((StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":"no_verified_service_endpoint","announcement_id":record.event().id,"seed_outcomes":outcomes}))))
    }).await.map_err(|_|(StatusCode::GATEWAY_TIMEOUT,Json(json!({"error":"discovery_deadline"}))))?
}
