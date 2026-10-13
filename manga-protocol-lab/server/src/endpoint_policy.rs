//! Outbound discovery boundary: validate every DNS result, pin the connection,
//! disable proxies/redirects, then require fresh service-key possession.
use crate::{
    auth::{self, Event},
    db::Id,
    transfer,
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeSet,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
#[error("endpoint destination policy rejected")]
pub struct DestinationDenied;
pub fn canonical_origin(value: &str) -> Result<String> {
    Ok(canonical(&origin(value)?))
}
#[derive(Default, Clone)]
pub struct EndpointPolicy {
    private_overrides: BTreeSet<String>,
}
pub struct VerifiedEndpoint {
    origin: String,
    service: Id,
    client: reqwest::Client,
}
fn origin(value: &str) -> Result<url::Url> {
    ensure!(value.len() <= 2048, "endpoint URL budget");
    let url = url::Url::parse(value)?;
    ensure!(
        matches!(url.scheme(), "https" | "http")
            && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "endpoint must be an HTTP(S) origin without credentials or suffix"
    );
    Ok(url)
}
fn canonical(url: &url::Url) -> String {
    url.origin().ascii_serialization()
}
/// Conservative public-unicast policy, including denial of transition/documentation ranges.
fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && ((b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99) || b == 168))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] <= 0x01ff || s[1] == 0x0db8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && (s[1] & 0xf000) == 0)
        }
    }
}
impl EndpointPolicy {
    /// Overrides are exact origins, including their effective port, never suffix/wildcard rules.
    pub fn new(overrides: &[String]) -> Result<Self> {
        ensure!(overrides.len() <= 64, "private endpoint override budget");
        let mut private_overrides = BTreeSet::new();
        for endpoint in overrides {
            private_overrides.insert(canonical(&origin(endpoint)?));
        }
        Ok(Self { private_overrides })
    }
    fn check(&self, url: &url::Url, addresses: &[SocketAddr]) -> Result<()> {
        ensure!(
            !addresses.is_empty() && addresses.len() <= 16,
            "endpoint DNS answer budget"
        );
        let overridden = self.private_overrides.contains(&canonical(url));
        ensure!(url.scheme() == "https" || overridden, DestinationDenied);
        ensure!(
            overridden || addresses.iter().all(|a| public_address(a.ip())),
            DestinationDenied
        );
        Ok(())
    }
    pub async fn connect(&self, endpoint: &str, expected: Option<Id>) -> Result<VerifiedEndpoint> {
        let url = origin(endpoint)?;
        ensure!(
            url.scheme() == "https" || self.private_overrides.contains(&canonical(&url)),
            DestinationDenied
        );
        let port = url.port_or_known_default().context("endpoint port")?;
        let host = url.host().context("endpoint host")?;
        // Reject explicit non-public literals before attempting any connection.
        let addresses: Vec<SocketAddr> = match host {
            url::Host::Ipv4(ip) => vec![SocketAddr::new(ip.into(), port)],
            url::Host::Ipv6(ip) => vec![SocketAddr::new(ip.into(), port)],
            url::Host::Domain(domain) => tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::lookup_host((domain, port)),
            )
            .await
            .context("endpoint DNS timeout")??
            .take(17)
            .collect(),
        };
        self.connect_resolved(url, addresses, expected).await
    }
    async fn connect_resolved(
        &self,
        url: url::Url,
        addresses: Vec<SocketAddr>,
        expected: Option<Id>,
    ) -> Result<VerifiedEndpoint> {
        let host = url.host().context("endpoint host")?;
        self.check(&url, &addresses)?;
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(2);
        if let url::Host::Domain(domain) = host {
            builder = builder.resolve_to_addrs(domain, &addresses);
        }
        let client = builder.build()?;
        let origin = canonical(&url);
        let service = prove(&client, &origin, expected).await?;
        Ok(VerifiedEndpoint {
            origin,
            service,
            client,
        })
    }
}
async fn prove(client: &reqwest::Client, origin: &str, expected: Option<Id>) -> Result<Id> {
    let nonce = hex::encode(rand::random::<[u8; 32]>());
    let response = client
        .get(format!("{origin}/identity?nonce={nonce}"))
        .send()
        .await?;
    // Preserve typed status for operational callers without accepting a proof.
    response.error_for_status_ref()?;
    ensure!(
        response.status() == 200,
        "identity endpoint unavailable or redirected"
    );
    let bytes = transfer::read_bounded(response, 16384, None)
        .await
        .map_err(anyhow::Error::new)?;
    let event: Event = serde_json::from_slice(&bytes)?;
    event.verify().map_err(anyhow::Error::msg)?;
    let service: Id = hex::decode(&event.pubkey)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("service key"))?;
    ensure!(
        expected.is_none_or(|key| key == service),
        crate::service_identity::IdentityChanged
    );
    ensure!(
        event.kind == 20005
            && event.created_at.abs_diff(auth::now()) <= 60
            && event.one("nonce").map_err(anyhow::Error::msg)? == nonce
            && event.one("endpoint").map_err(anyhow::Error::msg)? == origin,
        "service possession proof scope"
    );
    Ok(service)
}
impl VerifiedEndpoint {
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn service_key(&self) -> Id {
        self.service
    }
    pub async fn reverify(&self) -> Result<()> {
        prove(&self.client, &self.origin, Some(self.service)).await?;
        Ok(())
    }
    fn url(&self, path: &str) -> Result<url::Url> {
        ensure!(
            path.starts_with('/')
                && !path.starts_with("//")
                && !path.contains('\\')
                && path.len() <= 32768,
            "endpoint path rejected"
        );
        let url = url::Url::parse(&format!("{}{path}", self.origin))?;
        ensure!(
            canonical(&url) == self.origin && url.fragment().is_none(),
            "endpoint path changed origin"
        );
        Ok(url)
    }
    /// Bounded same-origin library transport. The client layer verifies signed
    /// metadata receipts and content-addressed objects before using the result.
    pub(crate) async fn library_request(
        &self,
        method: reqwest::Method,
        path: &str,
        authorization: &str,
        body: Vec<u8>,
        limit: usize,
    ) -> Result<(u16, reqwest::header::HeaderMap, Vec<u8>)> {
        ensure!(
            matches!(method, reqwest::Method::GET | reqwest::Method::POST),
            "library method"
        );
        ensure!(
            path == "/libraries"
                || path.starts_with("/libraries/")
                || path.starts_with("/library/")
                || path == "/account-metadata"
                || path.starts_with("/accounts/")
                || path.starts_with("/events/"),
            "library route"
        );
        ensure!(
            body.len() <= crate::library_wire::MAX_WIRE && limit <= crate::library_wire::MAX_WIRE,
            "library transport budget"
        );
        let response = self
            .client
            .request(method, self.url(path)?)
            .header("authorization", authorization)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = transfer::read_bounded(response, limit, None)
            .await
            .map_err(anyhow::Error::new)?;
        Ok((status, headers, bytes))
    }

    /// Only crate-internal peer transport may reuse this DNS-pinned client.
    pub(crate) fn pinned_client(&self) -> reqwest::Client {
        self.client.clone()
    }

    /// Unauthenticated discovery metadata only. Callers must verify returned signed records.
    pub async fn get_metadata(&self, path: &str) -> Result<Vec<u8>> {
        self.get_metadata_inner(path, None).await
    }
    pub async fn get_authorized_metadata(
        &self,
        path: &str,
        authorization: &str,
    ) -> Result<Vec<u8>> {
        self.get_metadata_inner(path, Some(authorization)).await
    }
    async fn get_metadata_inner(&self, path: &str, authorization: Option<&str>) -> Result<Vec<u8>> {
        let mut request = self.client.get(self.url(path)?);
        if let Some(authorization) = authorization {
            request = request.header("authorization", authorization);
        }
        let response = request.send().await?;
        ensure!(
            response.status() == 200,
            "discovery metadata unavailable or redirected"
        );
        transfer::read_bounded(response, 65536, None)
            .await
            .map_err(anyhow::Error::new)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn explicit_override_and_fresh_identity_are_both_required() -> Result<()> {
        use axum::{Json, Router, extract::Query, routing::get};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let advertised = endpoint.clone();
        let app = Router::new()
            .route(
                "/identity",
                get(
                    move |Query(q): Query<std::collections::HashMap<String, String>>| {
                        let endpoint = advertised.clone();
                        async move {
                            Json(
                                auth::sign(
                                    &format!("{:064x}", 11),
                                    20005,
                                    vec![
                                        vec!["nonce".into(), q.get("nonce").unwrap().clone()],
                                        vec!["endpoint".into(), endpoint],
                                    ],
                                    "service identity fixture".into(),
                                )
                                .unwrap(),
                            )
                        }
                    },
                ),
            )
            .route("/metadata", get(|| async { "{}" }));
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        assert!(
            EndpointPolicy::default()
                .connect(&endpoint, None)
                .await
                .is_err()
        );
        let policy = EndpointPolicy::new(std::slice::from_ref(&endpoint))?;
        let verified = policy.connect(&endpoint, None).await?;
        verified.reverify().await?;
        assert_eq!(verified.get_metadata("/metadata").await?, b"{}");
        assert!(verified.get_metadata("//another.example").await.is_err());
        assert!(policy.connect(&endpoint, Some([1; 32])).await.is_err());
        assert!(
            policy
                .connect(&endpoint, Some(verified.service_key()))
                .await
                .is_ok()
        );
        task.abort();
        Ok(())
    }
    #[tokio::test]
    async fn public_connection_rejects_special_literal_destinations() {
        for address in [
            "0.0.0.0",
            "10.1.2.3",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.0.0.9",
            "192.0.2.1",
            "192.168.1.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "64:ff9b::7f00:1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "2001:db8::1",
            "2002:7f00:1::",
            "3fff::1",
        ] {
            let endpoint = if address.contains(':') {
                format!("https://[{address}]")
            } else {
                format!("https://{address}")
            };
            let error = EndpointPolicy::default()
                .connect(&endpoint, None)
                .await
                .err()
                .expect("special destinations must be refused before connection");
            assert!(error.is::<DestinationDenied>(), "{endpoint}: {error:#}");
        }
    }

    #[test]
    fn public_origin_normalization_rejects_credentials_and_suffixes() {
        for value in [
            "file:///tmp/key",
            "https://user:pass@example.com",
            "https://example.com/path",
            "https://example.com?x=1",
            "https://example.com#fragment",
        ] {
            assert!(canonical_origin(value).is_err(), "{value}");
        }
        assert_eq!(
            canonical_origin("HTTPS://EXAMPLE.COM:443/").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            canonical_origin("http://example.com:8080").unwrap(),
            "http://example.com:8080"
        );
    }

    #[tokio::test]
    async fn public_connection_refuses_redirects_and_invalid_identity_proofs() -> Result<()> {
        use axum::{Json, Router, extract::Query, response::IntoResponse, routing::get};
        for mode in [
            "valid",
            "redirect",
            "wrong_nonce",
            "wrong_endpoint",
            "oversize",
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let endpoint = format!("http://{}", listener.local_addr()?);
            let advertised = endpoint.clone();
            let redirected_origin = endpoint.clone();
            let app = Router::new()
                .route(
                    "/identity",
                    get(
                        move |Query(q): Query<std::collections::HashMap<String, String>>| {
                            let endpoint = advertised.clone();
                            async move {
                                if mode == "redirect" {
                                    return (
                                        axum::http::StatusCode::FOUND,
                                        [("location", format!("/redirected?nonce={}", q["nonce"]))],
                                    )
                                        .into_response();
                                }
                                if mode == "oversize" {
                                    return "x".repeat(17000).into_response();
                                }
                                let nonce = if mode == "wrong_nonce" {
                                    "00".repeat(32)
                                } else {
                                    q["nonce"].clone()
                                };
                                let endpoint = if mode == "wrong_endpoint" {
                                    "http://another.invalid".to_owned()
                                } else {
                                    endpoint
                                };
                                Json(
                                    auth::sign(
                                        &format!("{:064x}", 11),
                                        20005,
                                        vec![
                                            vec!["nonce".into(), nonce],
                                            vec!["endpoint".into(), endpoint],
                                        ],
                                        "proof".into(),
                                    )
                                    .unwrap(),
                                )
                                .into_response()
                            }
                        },
                    ),
                )
                .route(
                    "/redirected",
                    get(
                        move |Query(q): Query<std::collections::HashMap<String, String>>| {
                            let endpoint = redirected_origin.clone();
                            async move {
                                // Following this redirect would yield a valid identity proof,
                                // so a rejection establishes the public no-redirect behavior.
                                Json(
                                    auth::sign(
                                        &format!("{:064x}", 11),
                                        20005,
                                        vec![
                                            vec!["nonce".into(), q["nonce"].clone()],
                                            vec!["endpoint".into(), endpoint],
                                        ],
                                        "proof".into(),
                                    )
                                    .unwrap(),
                                )
                            }
                        },
                    ),
                );
            let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let policy = EndpointPolicy::new(std::slice::from_ref(&endpoint))?;
            let result = policy.connect(&endpoint, None).await;
            assert_eq!(result.is_ok(), mode == "valid", "{mode}");
            if let Ok(endpoint) = result {
                assert_eq!(
                    hex::encode(endpoint.service_key()),
                    "774ae7f858a9411e5ef4246b70c65aac5649980be5c17891bbec17895da008cb"
                );
            }
            task.abort();
        }
        Ok(())
    }
}
