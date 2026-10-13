mod support;
use anyhow::{Context, Result, ensure};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::json;
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
use support::*;
struct Children(Vec<Child>);
impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn port() -> Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port())
}
fn openssl(dir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("openssl")
        .current_dir(dir)
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "fixture certificate generation: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated PostgreSQL, actual TLS nginx proxy and temporary fixture CA"]
async fn tls_proxy_preserves_external_auth_scope_without_trusting_forwarded_headers() -> Result<()>
{
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL")?;
    ensure!(
        dsn.parse::<tokio_postgres::Config>()?.get_dbname() == Some("manga_test"),
        "fixture only"
    );
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let dir = tempfile::tempdir()?;
    // Test-only CA, trusted exclusively by this reqwest client; no OS trust edit.
    openssl(
        dir.path(),
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
            "-days",
            "1",
            "-subj",
            "/CN=Manga Fixture CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
        ],
    )?;
    openssl(
        dir.path(),
        &[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "server.key",
            "-out",
            "server.csr",
            "-subj",
            "/CN=localhost",
        ],
    )?;
    std::fs::write(
        dir.path().join("server.ext"),
        "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,IP:127.0.0.1\n",
    )?;
    openssl(
        dir.path(),
        &[
            "x509",
            "-req",
            "-in",
            "server.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-out",
            "server.pem",
            "-days",
            "1",
            "-extfile",
            "server.ext",
        ],
    )?;
    let api_port = port()?;
    let tls_port = port()?;
    ensure!(api_port != tls_port, "fixture port collision");
    let origin = format!("https://127.0.0.1:{tls_port}");
    let key = dir.path().join("service.key");
    write_key(&key, 11)?;
    let config = dir.path().join("node.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":format!("127.0.0.1:{api_port}"),"base_url":origin,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"sync_worker_enabled":false,"backend":{"binary":root.join("vendor/strfry/strfry"),"data_dir":dir.path().join("backend"),"map_bytes":67108864}}),
        )?,
    )?;
    let node_log = std::fs::File::create(dir.path().join("node.log"))?;
    let node = Command::new(env!("CARGO_BIN_EXE_manga-node"))
        .args(["serve", config.to_str().unwrap()])
        .env("MANGA_DATABASE_URL", &dsn)
        .stdout(node_log.try_clone()?)
        .stderr(node_log)
        .spawn()?;
    let mut children = Children(vec![node]);
    let nginx_config = dir.path().join("nginx.conf");
    std::fs::write(
        &nginx_config,
        format!(
            "daemon off; master_process off; error_log {0}/nginx.log; pid {0}/nginx.pid; events {{worker_connections 64;}} http {{map $http_upgrade $connection_upgrade {{default upgrade; '' close;}} access_log off; client_body_temp_path {0}/body; proxy_temp_path {0}/proxy; server {{listen 127.0.0.1:{tls_port} ssl; ssl_certificate {0}/server.pem; ssl_certificate_key {0}/server.key; ssl_protocols TLSv1.2 TLSv1.3; location / {{proxy_pass http://127.0.0.1:{api_port}; proxy_http_version 1.1; proxy_set_header Upgrade $http_upgrade; proxy_set_header Connection $connection_upgrade; proxy_set_header Host $http_host; proxy_set_header X-Forwarded-Host $http_x_forwarded_host; proxy_set_header X-Forwarded-Proto $http_x_forwarded_proto;}}}}}}",
            dir.path().display()
        ),
    )?;
    children.0.push(
        Command::new(root.join("vendor/nginx-1.28.0/objs/nginx"))
            .args([
                "-p",
                dir.path().to_str().unwrap(),
                "-c",
                nginx_config.to_str().unwrap(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let certificate = reqwest::Certificate::from_pem(&std::fs::read(dir.path().join("ca.pem"))?)?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .add_root_certificate(certificate)
        .timeout(Duration::from_secs(5))
        .build()?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if client
                .get(&origin)
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .with_context(|| std::fs::read_to_string(dir.path().join("nginx.log")).unwrap_or_default())?;
    assert!(
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()?
            .get(&origin)
            .send()
            .await
            .is_err(),
        "untrusted CA is rejected; no certificate bypass"
    );
    let nonce = "ab".repeat(32);
    let proof: auth::Event = client
        .get(format!("{origin}/identity?nonce={nonce}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    proof.verify().map_err(anyhow::Error::msg)?;
    assert_eq!(proof.pubkey, public(11));
    assert_eq!(proof.one("endpoint").unwrap(), origin);
    assert_eq!(proof.one("nonce").unwrap(), nonce);
    let url = format!("{origin}/admin/accounts/{}", public(2));
    let body = serde_json::to_vec(&json!({"allowed":true}))?;
    let token = http(1, &url, &body);
    assert_eq!(
        client
            .post(&url)
            .header("authorization", &token)
            .body(body.clone())
            .send()
            .await?
            .status(),
        200
    );
    assert!(
        client
            .post(&url)
            .header("authorization", &token)
            .body(body.clone())
            .send()
            .await?
            .status()
            .is_client_error(),
        "proxy cannot defeat replay fence"
    );
    let fake_url = url.replace(&origin, "https://attacker.example");
    assert!(
        client
            .post(&url)
            .header("host", "attacker.example")
            .header("x-forwarded-host", "attacker.example")
            .header("x-forwarded-proto", "https")
            .header("authorization", http(1, &fake_url, &body))
            .body(body.clone())
            .send()
            .await?
            .status()
            .is_client_error(),
        "forwarded headers cannot change configured signing origin"
    );
    let internal = url.replace(&origin, &format!("http://127.0.0.1:{api_port}"));
    assert!(
        client
            .post(&url)
            .header("authorization", http(1, &internal, &body))
            .body(body.clone())
            .send()
            .await?
            .status()
            .is_client_error(),
        "internal transport origin is not external signing origin"
    );
    let tampered = serde_json::to_vec(&json!({"allowed":false}))?;
    assert!(
        client
            .post(&url)
            .header("authorization", http(1, &url, &body))
            .body(tampered)
            .send()
            .await?
            .status()
            .is_client_error(),
        "body binding survives proxy"
    );
    assert!(
        repo.account_permit(hex::decode(public(2))?.try_into().unwrap())
            .await
            .is_ok()
    );
    repo.set_peer(hex::decode(public(12))?.try_into().unwrap(), &origin, true)
        .await?;
    let ws = tokio::process::Command::new("python3")
        .current_dir(root)
        .env(
            "PYTHONPATH",
            format!(
                "{}:{}",
                root.join("vendor/pydeps").display(),
                root.display()
            ),
        )
        .args([
            "server/tests/support/tls_peer_client.py",
            &origin,
            dir.path().join("ca.pem").to_str().unwrap(),
            &format!("http://127.0.0.1:{api_port}"),
        ])
        .output()
        .await?;
    ensure!(
        ws.status.success(),
        "TLS peer: {}",
        String::from_utf8_lossy(&ws.stderr)
    );
    std::fs::write(root.join("results/peer-websocket-tls.json"), &ws.stdout)?;
    std::fs::write(
        root.join("results/http-tls-proxy.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["actual TLS nginx reverse proxy","temporary CA trusted only by test client","default client rejects fixture CA","signed identity binds external HTTPS origin","external NIP98 authorization accepted","replay rejected","forged forwarding headers rejected","internal-origin signature rejected","body substitution rejected","peer NIP42 over validated WSS with external-origin binding"],"limits":["loopback development TLS","no public deployment or production certificate lifecycle qualification"]}),
        )?,
    )?;
    Ok(())
}
