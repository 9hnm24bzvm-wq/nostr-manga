use anyhow::{Result, ensure};
use std::{future::IntoFuture, time::Duration};
use weebdex_platform::{AppState, Config, domains, router};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let action = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    ensure!(
        std::env::args().count() <= 2,
        "usage: weebdex-platform [check-config|verify|migrate|serve]"
    );
    ensure!(
        matches!(
            action.as_str(),
            "check-config" | "verify" | "migrate" | "serve"
        ),
        "unknown platform action"
    );
    let config = Config::load()?;
    ensure!(
        !config.restore_review_required || action == "check-config" || action == "verify",
        "restored publisher is fenced: complete offline recovery-point and exact-command review, then explicitly set restore_review_required=false before migrate or serve"
    );
    if action != "migrate" {
        config.validate_runtime_paths()?;
    }
    if action == "check-config" {
        tracing::info!(
            production = config.production,
            protocol_read_only = config.protocol_read_only,
            "configuration and frontend artifacts verified; no database or signer was opened"
        );
        return Ok(());
    }
    // Hold the native mutation owner until requests and all workers are gone.
    // The offline backup helper acquires this same lock before its snapshot.
    let _storage_owner = if action == "serve" || action == "migrate" {
        Some(weebdex_platform::runtime::acquire_storage_owner(
            &config.uploads_dir,
        )?)
    } else {
        None
    };
    let state = AppState::connect(config, Config::database_dsn()?).await?;
    if action == "migrate" {
        state.db.migrate(domains::MIGRATIONS).await?;
        tracing::info!("platform migrations applied; original protocol migrations remain separate");
        return Ok(());
    }
    state.db.verify_schema(domains::MIGRATIONS).await?;
    let original_ready = state.protocol.verify_schema().await.is_ok();
    ensure!(
        !state.config.production || original_ready,
        "production requires verified original protocol schema"
    );
    if action == "verify" {
        tracing::info!(
            protocol_schema_ready = original_ready,
            "schema and configuration verified; publication ownership was not opened"
        );
        return Ok(());
    }
    // Install original-module visibility or its fail-closed reset before any read
    // worker or HTTP listener. This cannot bless a remote-only protocol record.
    state.protocol.install_visibility_function().await?;
    let bind = state.config.bind;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let mut native_listener = None;
    let importer = if original_ready {
        if let Some(config) = state.config.protocol_publication.clone() {
            state.protocol.configure_publication(config).await?;
            let runtime =
                state.protocol.publication_runtime().await.map_err(|_| {
                    anyhow::anyhow!("configured publication runtime is unavailable")
                })?;
            native_listener = runtime.start_native_listener().await?;
        }
        Some(weebdex_platform::protocol::bridge::spawn_importer(
            state.clone(),
        ))
    } else {
        ensure!(
            state.config.protocol_publication.is_none(),
            "configured protocol publication requires a verified original schema"
        );
        tracing::warn!(
            "protocol schema unavailable; protocol-dependent API operations fail closed"
        );
        None
    };
    domains::uploads::recover_storage(&state).await;
    let maintenance = weebdex_platform::runtime::spawn_storage_maintenance(state.clone());
    state.runtime.mark_ready();
    tracing::info!(address = %bind, production = state.config.production, protocol_read_only = state.config.protocol_read_only, "WeebDex listener ready");
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(
        listener,
        router::router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = shutdown_receiver.await;
    })
    .into_future();
    tokio::pin!(server);
    enum Stop {
        Http(std::io::Result<()>),
        Signal,
        Native,
    }
    let stop = tokio::select! {
        result = &mut server => Stop::Http(result),
        _ = shutdown() => Stop::Signal,
        _ = native_exit(&mut native_listener) => Stop::Native,
    };
    state.runtime.begin_draining();
    let http_finished = matches!(&stop, Stop::Http(_));
    let native_finished = matches!(&stop, Stop::Native);
    let fault = match stop {
        Stop::Http(result) => result.err().map(anyhow::Error::from),
        Stop::Signal => None,
        Stop::Native => {
            tracing::error!("required original protocol listener exited; readiness withdrawn");
            Some(anyhow::anyhow!(
                "original protocol listener exited unexpectedly"
            ))
        }
    };
    let _ = shutdown_sender.send(());
    if let Some(native) = native_listener.as_mut() {
        if let Some(shutdown) = native.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
    tracing::info!("readiness withdrawn; draining both HTTP listeners and admitted requests");
    let result = tokio::time::timeout(
        Duration::from_secs(state.config.shutdown_grace_seconds),
        async {
            if !http_finished {
                (&mut server).await?;
            }
            // Network drain alone misses disconnected, owned SQL operations.
            while state.requests.available_permits() != state.config.maximum_requests {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            if !native_finished {
                if let Some(native) = native_listener.as_mut() {
                    (&mut native.task)
                        .await
                        .map_err(|_| anyhow::anyhow!("original protocol listener join failed"))??;
                }
            }
            Ok::<(), anyhow::Error>(())
        },
    )
    .await;
    if let Some(native) = native_listener.as_mut() {
        if !native.task.is_finished() {
            native.task.abort();
            let _ = (&mut native.task).await;
        }
    }
    let result = match result {
        Ok(result) => result,
        Err(_) => {
            tracing::error!(
                "shutdown deadline exceeded; unfinished durable commands require restart reconciliation, no commit success is asserted"
            );
            Err(anyhow::anyhow!("request drain deadline exceeded"))
        }
    };
    maintenance.abort();
    let _ = maintenance.await;
    state.runtime.begin_draining();
    if let Some(importer) = importer {
        importer.abort();
        let _ = importer.await;
    }
    state.db.pool().close();
    result?;
    if let Some(error) = fault {
        return Err(error);
    }
    tracing::info!("WeebDex stopped after request drain");
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        if let Ok(mut term) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

// A required listener is a supervised runtime component, not a detached task.
async fn native_exit(
    listener: &mut Option<weebdex_platform::protocol::publication::NativeListener>,
) {
    if let Some(listener) = listener.as_mut() {
        let _ = (&mut listener.task).await;
    } else {
        std::future::pending::<()>().await;
    }
}
