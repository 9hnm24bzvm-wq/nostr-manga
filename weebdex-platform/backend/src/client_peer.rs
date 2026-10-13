//! Transport-grounded client identity. Forwarded headers are authoritative only
//! for explicitly configured immediate proxies and an exact trusted hop chain.
use crate::{
    Config,
    error::{ApiError, ApiResult},
};
use axum::http::HeaderMap;
use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientPeer(pub Option<IpAddr>);
impl ClientPeer {
    pub fn rate_identity(self) -> String {
        self.0
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "unavailable-transport".into())
    }
}
pub fn normalized(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip)),
        other => other,
    }
}
pub fn valid_unicast(ip: IpAddr) -> bool {
    let ip = normalized(ip);
    !ip.is_unspecified() && !ip.is_multicast() && !matches!(ip,IpAddr::V4(ip) if ip.is_broadcast())
}
fn trusted(ip: IpAddr, addresses: &[IpAddr]) -> bool {
    addresses
        .iter()
        .any(|candidate| normalized(*candidate) == normalized(ip))
}
/// One XFF value contains [client, upstream_proxy...] in edge-to-application
/// order. The actual socket is the final proxy, never a header-derived address.
/// Nginx's single-hop example overwrites XFF with its genuine transport peer.
fn resolve(
    headers: &HeaderMap,
    transport: Option<SocketAddr>,
    addresses: &[IpAddr],
    hops: usize,
) -> ApiResult<ClientPeer> {
    let actual = transport.map(|socket| normalized(socket.ip()));
    let Some(actual) = actual else {
        return Ok(ClientPeer(None));
    };
    // Untrusted callers cannot multiply identities, even with malformed values.
    if !trusted(actual, addresses) {
        return Ok(ClientPeer(Some(actual)));
    }
    let mut values = headers.get_all("x-forwarded-for").iter();
    // Direct local probes with no edge header retain their real socket identity.
    let Some(value) = values.next() else {
        return Ok(ClientPeer(Some(actual)));
    };
    if values.next().is_some() || !(1..=8).contains(&hops) {
        return Err(ApiError::bad_request("Invalid trusted proxy chain"));
    }
    let value = value
        .to_str()
        .map_err(|_| ApiError::bad_request("Invalid trusted proxy chain"))?;
    if value.len() > 512 || !value.is_ascii() {
        return Err(ApiError::bad_request("Invalid trusted proxy chain"));
    }
    let mut chain = Vec::with_capacity(hops);
    for part in value.split(',') {
        if chain.len() >= hops {
            return Err(ApiError::bad_request("Invalid trusted proxy chain"));
        }
        let part = part.trim_matches([' ', '\t']);
        let ip = part
            .parse::<IpAddr>()
            .map_err(|_| ApiError::bad_request("Invalid trusted proxy chain"))?;
        if !valid_unicast(ip) {
            return Err(ApiError::bad_request("Invalid trusted proxy chain"));
        }
        chain.push(normalized(ip));
    }
    if chain.len() != hops || chain[1..].iter().any(|ip| !trusted(*ip, addresses)) {
        return Err(ApiError::bad_request("Invalid trusted proxy chain"));
    }
    Ok(ClientPeer(Some(chain[0])))
}
pub async fn install(
    State(config): State<Arc<Config>>,
    mut request: Request,
    next: Next,
) -> Response {
    let transport = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|peer| peer.0);
    let peer = match resolve(
        request.headers(),
        transport,
        &config.trusted_proxy_peers,
        config.trusted_proxy_hops,
    ) {
        Ok(peer) => peer,
        Err(error) => return error.into_response(),
    };
    // Replace any preexisting downstream extension with verified transport data.
    request.extensions_mut().insert(peer);
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Extension, Router,
        body::{Body, to_bytes},
        http::{Request as HttpRequest, StatusCode},
        routing::get,
    };
    use tower::ServiceExt;
    fn policy(addresses: &[&str], hops: usize) -> Arc<Config> {
        Arc::new(Config {
            trusted_proxy_peers: addresses
                .iter()
                .map(|value| value.parse().unwrap())
                .collect(),
            trusted_proxy_hops: hops,
            ..Config::default()
        })
    }
    async fn endpoint(Extension(peer): Extension<ClientPeer>) -> String {
        peer.rate_identity()
    }
    async fn through(
        config: Arc<Config>,
        actual: Option<&str>,
        forwarded: &[&str],
        preforged: bool,
    ) -> (StatusCode, String) {
        let app = Router::new()
            .route("/client", get(endpoint))
            .layer(axum::middleware::from_fn_with_state(config, install));
        let mut request = HttpRequest::builder()
            .uri("/client")
            .body(Body::empty())
            .unwrap();
        if let Some(actual) = actual {
            request
                .extensions_mut()
                .insert(ConnectInfo(actual.parse::<SocketAddr>().unwrap()));
        }
        if preforged {
            request
                .extensions_mut()
                .insert(ClientPeer(Some("192.0.2.250".parse().unwrap())));
        }
        for value in forwarded {
            request
                .headers_mut()
                .append("x-forwarded-for", value.parse().unwrap());
        }
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        (
            status,
            String::from_utf8(to_bytes(response.into_body(), 2048).await.unwrap().to_vec())
                .unwrap(),
        )
    }
    #[tokio::test]
    async fn real_middleware_preserves_direct_defaults_and_ignores_spoofed_headers() {
        for values in [
            vec!["198.51.100.4"],
            vec!["unknown"],
            vec!["198.51.100.4", "203.0.113.3"],
        ] {
            assert_eq!(
                through(policy(&[], 1), Some("192.0.2.8:4400"), &values, true).await,
                (StatusCode::OK, "192.0.2.8".into())
            );
        }
        assert_eq!(
            through(policy(&["127.0.0.1"], 1), None, &["198.51.100.4"], true).await,
            (StatusCode::OK, "unavailable-transport".into())
        );
        assert_eq!(
            through(
                policy(&["127.0.0.1"], 1),
                Some("127.0.0.1:4000"),
                &[],
                false
            )
            .await,
            (StatusCode::OK, "127.0.0.1".into())
        );
    }
    #[tokio::test]
    async fn trusted_edge_uses_distinct_real_clients_but_rejects_ambiguous_chains() {
        for client in ["198.51.100.4", "203.0.113.9", "2001:db8::4"] {
            assert_eq!(
                through(
                    policy(&["127.0.0.1"], 1),
                    Some("127.0.0.1:4000"),
                    &[client],
                    false
                )
                .await,
                (StatusCode::OK, client.into())
            );
        }
        for values in [
            vec!["unknown"],
            vec![""],
            vec!["198.51.100.4:80"],
            vec!["[2001:db8::4]"],
            vec!["0.0.0.0"],
            vec!["224.0.0.1"],
            vec!["255.255.255.255"],
            vec!["::ffff:0.0.0.0"],
            vec!["::ffff:224.0.0.1"],
            vec!["198.51.100.4, 203.0.113.9"],
            vec!["198.51.100.4", "203.0.113.9"],
            vec!["198.51.100.4,"],
        ] {
            assert_eq!(
                through(
                    policy(&["127.0.0.1"], 1),
                    Some("127.0.0.1:4000"),
                    &values,
                    false
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
        }
    }
    #[tokio::test]
    async fn explicit_multi_hop_requires_exact_count_and_every_intermediate_peer() {
        let valid = policy(&["127.0.0.1", "10.0.0.2"], 2);
        assert_eq!(
            through(
                valid.clone(),
                Some("127.0.0.1:4000"),
                &["198.51.100.4, 10.0.0.2"],
                false
            )
            .await,
            (StatusCode::OK, "198.51.100.4".into())
        );
        for chain in [
            "198.51.100.4",
            "198.51.100.4, 10.0.0.3",
            "198.51.100.4, 10.0.0.2, 127.0.0.1",
        ] {
            assert_eq!(
                through(valid.clone(), Some("127.0.0.1:4000"), &[chain], false)
                    .await
                    .0,
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            through(
                valid,
                Some("192.0.2.50:4000"),
                &["198.51.100.4, 10.0.0.2"],
                false
            )
            .await,
            (StatusCode::OK, "192.0.2.50".into())
        );
    }
    #[tokio::test]
    async fn mapped_ipv6_has_one_ipv4_identity_and_cannot_bypass_trust_or_limits() {
        assert_eq!(
            through(
                policy(&["127.0.0.1"], 1),
                Some("[::ffff:127.0.0.1]:4000"),
                &["::ffff:198.51.100.4"],
                false
            )
            .await,
            (StatusCode::OK, "198.51.100.4".into())
        );
    }
}
