//! Public outbound policy with resolver/socket faults injected outside the crate.
//! The Linux child process bounds LD_PRELOAD to this synthetic fixture alone.
#![cfg(target_os = "linux")]
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    endpoint_policy::{DestinationDenied, EndpointPolicy},
};

#[test]
fn resolved_endpoints_obey_destination_policy_and_keep_vetted_dns() -> Result<()> {
    if std::env::var("MANGA_ENDPOINT_DNS_CHILD").as_deref() == Ok("1") {
        return tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?
            .block_on(resolver_child());
    }
    let temp = tempfile::tempdir()?;
    let shim = temp.path().join("resolver.so");
    let compiled = std::process::Command::new("cc")
        .args([
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
        ])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/endpoint-dns/resolver.c"
        ))
        .args(["-o"])
        .arg(&shim)
        .arg("-ldl")
        .output()?;
    ensure!(
        compiled.status.success(),
        "resolver fixture compilation: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let result = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "resolved_endpoints_obey_destination_policy_and_keep_vetted_dns",
            "--nocapture",
        ])
        .env("MANGA_ENDPOINT_DNS_CHILD", "1")
        .env("LD_PRELOAD", &shim)
        .output()?;
    ensure!(
        result.status.success(),
        "public DNS-policy child failed:\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

async fn resolver_child() -> Result<()> {
    for endpoint in [
        "https://mixed.fixture.invalid",
        "https://empty.fixture.invalid",
        "https://many.fixture.invalid",
    ] {
        let error = EndpointPolicy::default()
            .connect(endpoint, None)
            .await
            .err()
            .context("destination unexpectedly accepted")?;
        if endpoint.contains("mixed") {
            assert!(error.is::<DestinationDenied>(), "{endpoint}: {error:#}");
        } else {
            assert_eq!(
                error.to_string(),
                "endpoint DNS answer budget",
                "{endpoint}"
            );
        }
    }
    // The external socket fixture refuses port 443 without sending traffic.
    // A caller-visible transport failure proves these public literals reached
    // connection admission rather than being misclassified as denied addresses.
    for address in [
        "1.1.1.1",
        "8.8.8.8",
        "[2606:4700::1111]",
        "[2001:4860:4860::8888]",
    ] {
        let endpoint = format!("https://{address}");
        let error = EndpointPolicy::default()
            .connect(&endpoint, None)
            .await
            .err()
            .context("fault socket unexpectedly connected")?;
        assert!(!error.is::<DestinationDenied>(), "{endpoint}: {error:#}");
        assert!(
            error.chain().any(|cause| cause
                .downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_connect)),
            "{endpoint}: {error:#}"
        );
    }
    use axum::{Json, Router, extract::Query, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!(
        "http://pinned.fixture.invalid:{}",
        listener.local_addr()?.port()
    );
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
        )
        .route("/metadata", get(|| async { "pinned metadata" }));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let policy = EndpointPolicy::new(std::slice::from_ref(&endpoint))?;
    let verified = policy.connect(&endpoint, None).await?;
    assert_eq!(
        hex::encode(verified.service_key()),
        "774ae7f858a9411e5ef4246b70c65aac5649980be5c17891bbec17895da008cb"
    );
    verified.reverify().await?;
    assert_eq!(
        verified.get_metadata("/metadata").await?,
        b"pinned metadata"
    );
    task.abort();
    Ok(())
}
