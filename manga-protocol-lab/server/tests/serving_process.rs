use anyhow::Result;
use manga_node::serving::{ManagedServing, ServingConfig};
use std::{path::PathBuf, time::Duration};
#[tokio::test]
async fn rust_parent_controls_actual_reader_family() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let dir = tempfile::tempdir()?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let serving = ManagedServing::prepare(ServingConfig {
        python: PathBuf::from("python3"),
        helper: root.join("src/serving_supervisor.py"),
        nginx: root.join("vendor/nginx-1.28.0/objs/nginx"),
        runtime: dir.path().join("public/.serving"),
        public: dir.path().join("public"),
        bind: bind.clone(),
        owner: [42; 32],
        workers: 1,
        shutdown_seconds: 1,
    })
    .await?;
    serving.activate().await?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?;
    assert_eq!(
        client
            .get(format!("http://{bind}/_serving"))
            .send()
            .await?
            .status(),
        204
    );
    let drained = serving.drain().await?;
    assert!(!drained.workers().is_empty());
    for worker in drained.workers() {
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{}/stat", worker.pid)) {
            let fields: Vec<_> = stat[stat.rfind(')').unwrap() + 2..]
                .split_whitespace()
                .collect();
            assert!(fields[0] == "Z" || fields[19].parse::<u64>()? != worker.birth);
        }
    }
    serving.shutdown_verified().await?;
    assert!(
        client
            .get(format!("http://{bind}/_serving"))
            .send()
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn cancelling_a_drain_closes_the_owner_pipe() -> Result<()> {
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let dir = tempfile::tempdir()?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let config = ServingConfig {
        python: PathBuf::from("python3"),
        helper: root.join("src/serving_supervisor.py"),
        nginx: root.join("vendor/nginx-1.28.0/objs/nginx"),
        runtime: dir.path().join("public/.serving"),
        public: dir.path().join("public"),
        bind: bind.clone(),
        owner: [43; 32],
        workers: 1,
        shutdown_seconds: 4,
    };
    let serving = ManagedServing::prepare(config.clone()).await?;
    serving.activate().await?;
    let bytes = vec![7u8; 16 * 1024 * 1024];
    let hash = hex::encode(Sha256::digest(&bytes));
    let parent = config.public.join(&hash[..2]).join(&hash[2..4]);
    std::fs::create_dir_all(&parent)?;
    std::fs::write(parent.join(format!("{hash}.png")), bytes)?;
    let mut slow = tokio::net::TcpStream::connect(&bind).await?;
    slow.write_all(
        format!("GET /{hash}.png HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )
    .await?;
    let mut header = Vec::new();
    while !header.windows(4).any(|x| x == b"\r\n\r\n") {
        let mut byte = [0];
        slow.read_exact(&mut byte).await?;
        header.push(byte[0]);
        assert!(header.len() < 4096);
    }
    assert!(header.starts_with(b"HTTP/1.1 200"));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), serving.drain())
            .await
            .is_err()
    );
    assert!(serving.drain().await.is_err());
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(300))
        .build()?;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match client
                .get(format!("http://{bind}/{hash}.png"))
                .header("Range", "bytes=0-0")
                .send()
                .await
            {
                Err(_) => break,
                Ok(response) => {
                    assert!(response.status().is_success());
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }
    })
    .await?;
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let state: serde_json::Value =
                serde_json::from_slice(&std::fs::read(config.runtime.join("state.json"))?)?;
            if state["phase"] == "stopped" {
                break Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    drop(slow);
    assert!(tokio::net::TcpStream::connect(&bind).await.is_err());
    Ok(())
}

#[tokio::test]
async fn prepared_owner_keeps_listener_closed_until_explicit_activation() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let dir = tempfile::tempdir()?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let cfg = ServingConfig {
        python: PathBuf::from("python3"),
        helper: root.join("src/serving_supervisor.py"),
        nginx: root.join("vendor/nginx-1.28.0/objs/nginx"),
        runtime: dir.path().join("public/.serving"),
        public: dir.path().join("public"),
        bind: bind.clone(),
        owner: [44; 32],
        workers: 1,
        shutdown_seconds: 1,
    };
    let serving = ManagedServing::prepare(cfg.clone()).await?;
    assert!(tokio::net::TcpStream::connect(&bind).await.is_err());
    assert!(serving.drain().await?.workers().is_empty());
    assert!(
        ManagedServing::prepare(cfg.clone()).await.is_err(),
        "prepared owner must exclude another serving family"
    );
    serving.activate().await?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?;
    assert_eq!(
        client
            .get(format!("http://{bind}/_serving"))
            .send()
            .await?
            .status(),
        204
    );
    serving.activate().await?;
    serving.shutdown_verified().await?;
    assert!(tokio::net::TcpStream::connect(&bind).await.is_err());
    let serving = ManagedServing::prepare(cfg.clone()).await?;
    assert!(serving.drain().await?.workers().is_empty());
    serving.activate().await?;
    assert_eq!(
        client
            .get(format!("http://{bind}/_serving"))
            .send()
            .await?
            .status(),
        204
    );
    serving.shutdown_verified().await?;
    let again = ManagedServing::prepare(cfg).await?;
    assert!(tokio::net::TcpStream::connect(&bind).await.is_err());
    again.shutdown_verified().await?;
    Ok(())
}

#[tokio::test]
async fn missing_master_is_not_healthy_even_with_multiple_orphan_workers() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let dir = tempfile::tempdir()?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let bind = listener.local_addr()?.to_string();
    drop(listener);
    let runtime = dir.path().join("public/.serving");
    let serving = ManagedServing::prepare(ServingConfig {
        python: PathBuf::from("python3"),
        helper: root.join("src/serving_supervisor.py"),
        nginx: root.join("vendor/nginx-1.28.0/objs/nginx"),
        runtime: runtime.clone(),
        public: dir.path().join("public"),
        bind: bind.clone(),
        owner: [45; 32],
        workers: 2,
        shutdown_seconds: 1,
    })
    .await?;
    serving.activate().await?;
    serving.check_active().await?;
    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(runtime.join("state.json"))?)?;
    let pid = state["master"]["pid"].as_u64().unwrap();
    assert!(
        std::process::Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()?
            .success()
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(serving.check_active().await.is_err());
    serving.shutdown_verified().await?;
    assert!(tokio::net::TcpStream::connect(&bind).await.is_err());
    Ok(())
}
