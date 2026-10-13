//! Commit-delivered PostgreSQL notifications are hints, never visibility authority.
use crate::db::{Id, Repository};
use anyhow::{Result, anyhow};
use futures_util::future::poll_fn;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::broadcast,
};
pub struct Feed {
    pub sender: broadcast::Sender<Option<Id>>,
    pub ready: AtomicBool,
}
struct ResetOnDrop(Arc<Feed>);
impl Drop for ResetOnDrop {
    fn drop(&mut self) {
        self.0.ready.store(false, Ordering::Release);
        let _ = self.0.sender.send(None);
    }
}
struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Feed {
    pub fn new() -> Arc<Self> {
        let (sender, _) = broadcast::channel(1024);
        Arc::new(Self {
            sender,
            ready: AtomicBool::new(false),
        })
    }
    /// Owned worker lifecycle: cancellation invalidates every current receiver
    /// and drops the connection driver, exactly as a transport discontinuity does.
    pub async fn run(self: Arc<Self>, repo: Repository) {
        let _reset = ResetOnDrop(self.clone());
        loop {
            let result = self.connect(&repo).await;
            self.ready.store(false, Ordering::Release);
            let _ = self.sender.send(None);
            if let Err(error) = result {
                tracing::warn!(error=%error,"relay notification feed reset");
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    async fn connect(self: &Arc<Self>, repo: &Repository) -> Result<()> {
        let mut config = (*repo.connection_config).clone();
        config.application_name("manga-relay-feed");
        if repo.plaintext_local {
            let (client, connection) = config.connect(tokio_postgres::NoTls).await?;
            self.drive(client, connection).await
        } else {
            let (tls, _) = tokio_postgres_rustls::MakeRustlsConnect::with_native_certs()
                .map_err(|_| anyhow!("no TLS roots for notification feed"))?;
            let (client, connection) = config.connect(tls).await?;
            self.drive(client, connection).await
        }
    }
    async fn drive<S, T>(
        self: &Arc<Self>,
        client: tokio_postgres::Client,
        mut connection: tokio_postgres::Connection<S, T>,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let sender = self.sender.clone();
        let mut driver = crate::node::spawn_infrastructure_owned_scope(async move {
            while let Some(message) = poll_fn(|cx| connection.poll_message(cx)).await {
                if let tokio_postgres::AsyncMessage::Notification(n) = message?
                    && n.channel() == "manga_events"
                    && crate::auth::valid_hash(n.payload())
                {
                    let id: Id = hex::decode(n.payload())?.try_into().unwrap();
                    let _ = sender.send(Some(id));
                }
            }
            Err::<(), anyhow::Error>(anyhow!("notification connection closed"))
        });
        let _abort = AbortOnDrop(driver.abort_handle());
        if let Err(error) = client.batch_execute("LISTEN manga_events").await {
            driver.abort();
            return Err(error.into());
        }
        self.ready.store(true, Ordering::Release);
        let heartbeat = async {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    client.simple_query("SELECT 1"),
                )
                .await??;
            }
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        };
        let result = tokio::select! {result=&mut driver=>result?,result=heartbeat=>result};
        driver.abort();
        result
    }
}
