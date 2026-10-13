//! Public dispatcher/event-source contracts; no private counters or test hooks.
use anyhow::Result;
use manga_node::{
    auth,
    db::Id,
    relay_dispatch::{Dispatcher, EventSource},
    relay_query::RelayFilter,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc, oneshot};
struct ChannelSource(mpsc::Sender<(Id, oneshot::Sender<Option<auth::Event>>)>);
impl EventSource for ChannelSource {
    async fn fetch(&self, id: Id) -> Result<Option<auth::Event>> {
        let (reply, response) = oneshot::channel();
        self.0.send((id, reply)).await?;
        Ok(response.await?)
    }
}
fn event() -> auth::Event {
    auth::sign(
        &format!("{:064x}", 1),
        1,
        vec![],
        "shared live event".into(),
    )
    .unwrap()
}
fn filter(kind: u16) -> Vec<RelayFilter> {
    vec![RelayFilter::parse(&json!({"kinds":[kind]})).unwrap()]
}
#[tokio::test]
async fn one_source_response_fans_out_and_filters_then_role_cancellation_signals_loss() -> Result<()>
{
    let dispatcher = Arc::new(Dispatcher::default());
    let mut first = dispatcher.subscribe()?;
    first.replace(filter(1))?;
    let mut second = dispatcher.subscribe()?;
    second.replace(filter(1))?;
    let mut other = dispatcher.subscribe()?;
    other.replace(filter(7))?;
    let (hints, receiver) = broadcast::channel(8);
    let (requests, mut source) = mpsc::channel(1);
    let worker = tokio::spawn(dispatcher.clone().run(receiver, ChannelSource(requests)));
    let event = event();
    let id: Id = hex::decode(&event.id)?.try_into().unwrap();
    hints.send(Some(id))?;
    let (requested, reply) = source.recv().await.unwrap();
    assert_eq!(requested, id);
    reply.send(Some(event.clone())).unwrap();
    for session in [&mut first, &mut second] {
        let received = tokio::time::timeout(Duration::from_secs(1), session.recv()).await??;
        received.verify().map_err(anyhow::Error::msg)?;
        assert_eq!(received.id, event.id);
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(20), source.recv())
            .await
            .is_err(),
        "one notification must require only one authoritative source lookup"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(20), other.recv())
            .await
            .is_err(),
        "nonmatching session must not receive event"
    );
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    for session in [&mut first, &mut second, &mut other] {
        assert!(session.recv().await.is_err());
    }
    Ok(())
}
struct FixedSource(auth::Event);
impl EventSource for FixedSource {
    async fn fetch(&self, _: Id) -> Result<Option<auth::Event>> {
        Ok(Some(self.0.clone()))
    }
}
#[tokio::test]
async fn slow_session_overflows_independently_and_discontinuity_requires_resubscribe() -> Result<()>
{
    let dispatcher = Arc::new(Dispatcher::default());
    let mut slow = dispatcher.subscribe()?;
    slow.replace(filter(1))?;
    let mut fast = dispatcher.subscribe()?;
    fast.replace(filter(1))?;
    let event = event();
    let id: Id = hex::decode(&event.id)?.try_into().unwrap();
    let (hints, receiver) = broadcast::channel(128);
    let worker = tokio::spawn(dispatcher.clone().run(receiver, FixedSource(event.clone())));
    for _ in 0..65 {
        hints.send(Some(id))?;
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), fast.recv())
                .await??
                .id,
            event.id
        );
    }
    assert!(
        slow.recv().await.is_err(),
        "bounded slow-session queue must report loss, never silently drop"
    );
    hints.send(None)?;
    assert!(
        tokio::time::timeout(Duration::from_secs(1), fast.recv())
            .await?
            .is_err()
    );
    let mut recovered = dispatcher.subscribe()?;
    recovered.replace(filter(1))?;
    hints.send(Some(id))?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), recovered.recv())
            .await??
            .id,
        event.id
    );
    worker.abort();
    let _ = worker.await;
    Ok(())
}
#[test]
fn disconnected_sessions_release_registration_capacity() -> Result<()> {
    let dispatcher = Arc::new(Dispatcher::default());
    let sessions = (0..40)
        .map(|_| dispatcher.subscribe())
        .collect::<Result<Vec<_>>>()?;
    assert!(dispatcher.subscribe().is_err());
    drop(sessions);
    dispatcher.subscribe()?;
    Ok(())
}

#[tokio::test]
async fn unresolved_source_deadline_reports_loss_without_delivering_unverified_data() -> Result<()>
{
    let dispatcher = Arc::new(Dispatcher::default());
    let mut session = dispatcher.subscribe()?;
    session.replace(filter(1))?;
    let (hints, receiver) = broadcast::channel(8);
    let (requests, mut source) = mpsc::channel(1);
    let worker = tokio::spawn(dispatcher.clone().run(receiver, ChannelSource(requests)));
    hints.send(Some([7; 32]))?;
    let (_, reply) = source.recv().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), session.recv())
            .await?
            .is_err()
    );
    assert!(
        reply.send(Some(event())).is_err(),
        "expired source response cannot become delivery"
    );
    worker.abort();
    let _ = worker.await;
    Ok(())
}
