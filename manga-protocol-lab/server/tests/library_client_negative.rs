mod support;
use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
};
use manga_node::{
    auth,
    endpoint_policy::EndpointPolicy,
    library_client::{self, DeliveryLimits, Outcome, Recipient, Signer, SigningRequest},
    library_client_bundle::SaveBundle,
    library_client_recovery,
    library_manifest::{Descriptor, Family, FamilyRoot, Limits, Root},
    service_proof,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use support::*;
#[derive(Clone)]
struct Fake {
    origin: String,
    event: auth::Event,
    root: Option<Vec<u8>>,
}
struct TestSigner;
#[async_trait::async_trait]
impl Signer for TestSigner {
    async fn sign(&self, r: SigningRequest) -> Result<auth::Event> {
        Ok(sign_at(2, r.kind, r.tags, r.content, r.created_at))
    }
}
fn id(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
async fn identity(
    State(state): State<Fake>,
    Query(query): Query<HashMap<String, String>>,
) -> Json<auth::Event> {
    Json(
        auth::sign(
            &format!("{:064x}", 11),
            20005,
            vec![
                vec!["nonce".into(), query["nonce"].clone()],
                vec!["endpoint".into(), state.origin],
            ],
            String::new(),
        )
        .unwrap(),
    )
}
fn response(
    state: &Fake,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    value: Value,
    corrupt: bool,
) -> axum::response::Response {
    let request = auth::token(headers["authorization"].to_str().unwrap()).unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    let proof = service_proof::sign(
        &format!("{:064x}", 11),
        &service_proof::Scope {
            url: &format!("{}{path}", state.origin),
            method,
            request: &request.id,
            status: 200,
        },
        &bytes,
    )
    .unwrap();
    let mut result = if corrupt {
        Json(json!({"tampered":true})).into_response()
    } else {
        Json(value).into_response()
    };
    result
        .headers_mut()
        .insert(service_proof::HEADER, proof.parse().unwrap());
    result
}
async fn batch(State(state): State<Fake>, headers: HeaderMap) -> axum::response::Response {
    response(
        &state,
        &headers,
        "POST",
        "/library/batches",
        json!({"hashes":[],"staged":true}),
        true,
    )
}
async fn head(
    State(state): State<Fake>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> axum::response::Response {
    response(
        &state,
        &headers,
        "GET",
        &format!("/libraries/{key}"),
        json!({"event":state.event,"projected":true}),
        false,
    )
}
async fn object(State(state): State<Fake>, Path(hash): Path<String>) -> Vec<u8> {
    if let Some(bytes) = state.root
        && auth::hash(&bytes) == hash
    {
        return bytes;
    }
    b"corrupt or missing bytes".to_vec()
}
async fn fake(
    event: auth::Event,
    root: Option<Vec<u8>>,
) -> Result<(String, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let app = Router::new()
        .route("/identity", get(identity))
        .route("/library/batches", post(batch))
        .route("/libraries/{key}", get(head))
        .route("/library/blobs/{hash}", get(object))
        .with_state(Fake {
            origin: origin.clone(),
            event,
            root,
        });
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Ok((origin, server))
}
#[tokio::test]
async fn newer_unavailable_head_is_reported_when_recovering_older_complete_copy() -> Result<()> {
    let root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![Descriptor::empty()],
            })
            .collect(),
    }
    .encode(&Limits::default())?;
    let old = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        auth::now() - 2,
    );
    let newer = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":"ab".repeat(32),"version":1}).to_string(),
        auth::now() - 1,
    );
    let (a, server_a) = fake(newer.clone(), None).await?;
    let (b, server_b) = fake(old.clone(), Some(root)).await?;
    let policy = EndpointPolicy::new(&[a.clone(), b.clone()])?;
    let recipients = [a, b]
        .into_iter()
        .map(|endpoint| Recipient {
            relay_url: "wss://test.example".into(),
            endpoint,
            service_key: id(&public(11)),
        })
        .collect();
    let recovered =
        library_client_recovery::recover(id(&public(2)), recipients, Arc::new(TestSigner), policy)
            .await?;
    assert_eq!(recovered.bundle.event().id, old.id);
    assert_eq!(recovered.newer_unavailable, vec![newer.id]);
    server_a.abort();
    server_b.abort();
    Ok(())
}
#[tokio::test]
async fn corrupted_signed_metadata_and_content_never_count_as_success() -> Result<()> {
    let root = Root {
        families: Family::ALL
            .into_iter()
            .map(|family| FamilyRoot {
                family,
                chunks: vec![Descriptor::empty()],
            })
            .collect(),
    }
    .encode(&Limits::default())?;
    let event = sign_at(
        2,
        11999,
        vec![],
        json!({"manifest":auth::hash(&root),"version":1}).to_string(),
        auth::now() - 1,
    );
    let bundle = Arc::new(SaveBundle::validate(event.clone(), root, BTreeMap::new())?);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let app = Router::new()
        .route("/identity", get(identity))
        .route("/library/batches", post(batch))
        .route("/libraries/{key}", get(head))
        .route(
            "/library/blobs/{hash}",
            get(|| async { b"corrupt root bytes".to_vec() }),
        )
        .with_state(Fake {
            origin: origin.clone(),
            event,
            root: None,
        });
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let policy = EndpointPolicy::new(std::slice::from_ref(&origin))?;
    let recipient = Recipient {
        relay_url: "wss://test.example".into(),
        endpoint: origin,
        service_key: id(&public(11)),
    };
    let signer: Arc<dyn Signer> = Arc::new(TestSigner);
    let delivery = library_client::deliver(
        bundle.clone(),
        vec![recipient.clone()],
        signer.clone(),
        policy.clone(),
        DeliveryLimits::default(),
    )?;
    let result = delivery.completion.await?;
    assert_eq!(
        result[0].outcome,
        Outcome::Retryable {
            reason: "unverified_batch_response".into()
        }
    );
    assert!(delivery.first_acceptance.await.is_err());
    use manga_node::library_client_outbox::{Outbox, OutboxLimits, run_job};
    let dir = tempfile::tempdir()?;
    let mut outbox = Outbox::open(
        dir.path(),
        OutboxLimits {
            attempts: 1,
            ..OutboxLimits::default()
        },
    )?;
    let job = outbox.enqueue(&bundle, vec![recipient.clone()], vec![])?;
    let outbox = Arc::new(std::sync::Mutex::new(outbox));
    for expected in [1, 2] {
        let run = run_job(
            outbox.clone(),
            job.clone(),
            signer.clone(),
            policy.clone(),
            DeliveryLimits::default(),
        )
        .await?;
        assert!(run.first_acceptance.await.is_err());
        let saved = run.completion.await??;
        assert_eq!(saved.recipients[0].attempts, expected);
        assert!(matches!(
            saved.recipients[0].outcome,
            Some(Outcome::Retryable { .. })
        ));
    }

    assert!(
        library_client_recovery::recover(id(&public(2)), vec![recipient], signer, policy)
            .await
            .is_err()
    );
    server.abort();
    Ok(())
}
