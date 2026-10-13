//! Bounded Nostr transport over the guarded local state, never raw strfry.
use crate::{
    auth::{self, Event},
    db::Repository,
    relay_feed::Feed,
    relay_query::RelayFilter,
};
use anyhow::{Result, ensure};
use axum::extract::ws::{Message, WebSocket};
use futures_util::SinkExt;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::sync::Semaphore;
#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadPolicy {
    #[default]
    Public,
    Authenticated,
    Admitted,
}
#[derive(Clone)]
pub struct Relay {
    pub peer_only: bool,
    pub read_policy: ReadPolicy,
    pub repo: Repository,
    pub feed: Arc<Feed>,
    pub dispatcher: Arc<crate::relay_dispatch::Dispatcher>,
    pub url: String,
    pub connections: Arc<Semaphore>,
    pub writes: Arc<Semaphore>,
    pub reads: Arc<crate::read_admission::Admission>,
    pub publications: Option<Arc<crate::publication::Publications>>,
    pub libraries: Option<Arc<crate::library_storage::LibraryObjects>>,
    pub projection_enabled: bool,
}
async fn send(socket: &mut WebSocket, value: Value) -> Result<()> {
    tokio::time::timeout(
        Duration::from_secs(5),
        socket.send(Message::Text(value.to_string().into())),
    )
    .await??;
    Ok(())
}
impl Relay {
    pub async fn session(
        self: Arc<Self>,
        mut socket: WebSocket,
        _slot: tokio::sync::OwnedSemaphorePermit,
    ) {
        if let Err(error) = self.run(&mut socket).await {
            tracing::debug!(error=%error,"relay connection closed");
        }
        let _ = tokio::time::timeout(Duration::from_secs(1), socket.close()).await;
    }
    // Spawn only after admission: outstanding owned policy reads are bounded by
    // the active read budget even if their caller times out or is cancelled.
    fn policy_owned(
        &self,
        keys: &BTreeSet<String>,
        permit: crate::read_admission::Permit,
    ) -> tokio::task::JoinHandle<Result<bool>> {
        let relay = self.clone();
        let keys = keys.clone();
        crate::node::spawn_owned_scope(async move {
            let _permit = permit;
            relay.can_read(&keys).await
        })
    }
    // The response deadline includes admission and authorization. Actual SQL
    // work retains its permit/client until the configured DB deadline ends it.
    // Socket I/O owns no DB admission credit.
    async fn delivery_allowed(&self, keys: &BTreeSet<String>) -> Result<bool> {
        tokio::time::timeout(Duration::from_secs(2), async {
            let permit = loop {
                match self.reads.acquire(crate::read_admission::Class::Live).await {
                    Ok(permit) => break permit,
                    Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            };
            self.policy_owned(keys, permit).await?
        })
        .await?
    }
    async fn run(&self, socket: &mut WebSocket) -> Result<()> {
        let challenge = hex::encode(rand::random::<[u8; 32]>());
        let mut keys = BTreeSet::new();
        let mut subscriptions: BTreeMap<String, Vec<RelayFilter>> = BTreeMap::new();
        // Register before history starts. Commit notifications received during history
        // remain queued; duplicate delivery is permitted, missing delivery is not.
        let mut receiver = self.dispatcher.subscribe()?;
        send(socket, json!(["AUTH", challenge])).await?;
        let mut command_tokens = 200.0f64;
        let mut last_command = tokio::time::Instant::now();
        let mut policy_check = tokio::time::interval(Duration::from_secs(2));
        let authentication_deadline = tokio::time::sleep(Duration::from_secs(15));
        tokio::pin!(authentication_deadline);
        loop {
            tokio::select! {
                     biased;
                     _=crate::node::embedded_shutdown()=>{
                      let _=tokio::time::timeout(Duration::from_secs(1),socket.send(Message::Close(None))).await;
                      return Ok(());
                     }
                     _=&mut authentication_deadline, if self.peer_only && keys.is_empty()=>{anyhow::bail!("peer authentication deadline");}
                     _=policy_check.tick(), if !subscriptions.is_empty() || (self.peer_only && !keys.is_empty())=>{
             let Ok(_query)=self.reads.acquire(crate::read_admission::Class::Live).await else {continue;};
             let allowed=tokio::time::timeout(Duration::from_secs(2),self.policy_owned(&keys,_query)).await???;
             if !allowed {
              for name in subscriptions.keys(){send(socket,json!(["CLOSED",name,"restricted: read permission revoked"])).await?;}
              subscriptions.clear(); receiver.replace(Vec::new())?;
              if self.peer_only {anyhow::bail!("peer approval revoked");}
             }
            }
            notification=receiver.recv()=>{
                      let event=match notification {Ok(event)=>event,Err(_)=>{
                       for name in subscriptions.keys(){send(socket,json!(["CLOSED",name,"error: live feed interrupted; reconnect and resubscribe"])).await?;}
                       anyhow::bail!("live feed interrupted");
                      }};
                      if subscriptions.is_empty(){continue;}
                      let allowed=match self.delivery_allowed(&keys).await {
                        Ok(allowed)=>allowed,
                        Err(_)=>{
                          for name in subscriptions.keys(){send(socket,json!(["CLOSED",name,"error: live authorization unavailable or deadline; resubscribe"])).await?;}
                          anyhow::bail!("live authorization unavailable");
                        }
                      };
                      if !allowed {
                        for name in subscriptions.keys(){send(socket,json!(["CLOSED",name,"restricted: read permission revoked"])).await?;}
                        subscriptions.clear(); receiver.replace(Vec::new())?; continue;
                      }
                      for (name,filters) in &subscriptions {if filters.iter().any(|f|f.matches(&event)){send(socket,json!(["EVENT",name,&*event])).await?;}}
                     }
                     incoming=socket.recv()=>{
                      let Some(incoming)=incoming else{return Ok(())};
                      let now=tokio::time::Instant::now();
                      command_tokens=(command_tokens+now.duration_since(last_command).as_secs_f64()*100.0).min(200.0);
                      last_command=now;
                      if command_tokens<1.0{send(socket,json!(["NOTICE","rate-limited: command budget exceeded"])).await?;return Ok(());}
                      command_tokens-=1.0;
                      let text=match incoming? {Message::Text(text)=>text,Message::Close(_)=>return Ok(()),Message::Ping(_)|Message::Pong(_)=>continue,_=>{send(socket,json!(["NOTICE","invalid: text frames required"])).await?;continue;}};
                      let value:Value=match serde_json::from_str(&text){Ok(v)=>v,Err(_)=>{send(socket,json!(["NOTICE","invalid: JSON required"])).await?;continue;}};
                      let Some(parts)=value.as_array() else{send(socket,json!(["NOTICE","invalid: command array required"])).await?;continue;};
                      match parts.first().and_then(Value::as_str) {
                       Some("CLOSE") if parts.len()==2=>{if let Some(name)=parts[1].as_str(){subscriptions.remove(name);receiver.replace(subscriptions.values().flatten().cloned().collect())?;}}
                       Some("AUTH") if parts.len()==2=>{
                        let event=serde_json::from_value::<Event>(parts[1].clone());
                        match event {
                         Ok(event)=>{
                          let _query=match self.reads.acquire(crate::read_admission::Class::Live).await{
                           Ok(permit)=>permit,
                           Err(_)=>{send(socket,json!(["OK",event.id,false,"rate-limited: read budget exhausted"])).await?;continue;}
                          };
                          let outcome=self.authenticate(&event,&challenge,&keys).await;
                          if outcome.is_ok(){keys.insert(event.pubkey.clone());}
                          drop(_query);
                          send(socket,json!(["OK",event.id,outcome.is_ok(),outcome.err().map(|error|if crate::db::is_database_error(&error){"error: metadata store unavailable"}else{"restricted: invalid authentication"}).unwrap_or("")])).await?;
                         }
                         Err(_)=>send(socket,json!(["NOTICE","invalid: AUTH event required"])).await?,
                        }
                       }
                       Some("REQ") if parts.len()>=3=>{
                        let Some(name)=parts[1].as_str().filter(|s|!s.is_empty()&&s.len()<=crate::protocol_limits::RELAY_SUBID_BYTES) else{send(socket,json!(["NOTICE","invalid: subscription ID"])).await?;continue;};
                        subscriptions.remove(name);
                        receiver.replace(subscriptions.values().flatten().cloned().collect())?;
                        let _query=match self.reads.acquire(crate::read_admission::Class::Live).await{
                         Ok(permit)=>permit,
                         Err(_)=>{send(socket,json!(["CLOSED",name,"rate-limited: read budget exhausted"])).await?;continue;}
                        };
               if !self.can_read(&keys).await? {drop(_query);send(socket,json!(["CLOSED",name,if keys.is_empty(){"auth-required: authenticate before reading"}else{"restricted: account not admitted"}])).await?;continue;}
                        let outcome:Result<Vec<RelayFilter>>=(||{ensure!(parts.len()<=2+crate::protocol_limits::RELAY_FILTERS&&subscriptions.len()<crate::protocol_limits::RELAY_SUBSCRIPTIONS,"subscription budget");parts[2..].iter().map(RelayFilter::parse).collect()})();
                        let filters=match outcome {Ok(f)=>f,Err(_)=>{drop(_query);send(socket,json!(["CLOSED",name,"invalid: filter or subscription budget"])).await?;continue;}};
                        if !self.feed.ready.load(Ordering::Acquire) || !self.dispatcher.is_ready(){drop(_query);send(socket,json!(["CLOSED",name,"error: live feed unavailable"])).await?;continue;}
                        // Register first: arrivals while history runs remain queued.
                        subscriptions.insert(name.into(),filters.clone());
                        receiver.replace(subscriptions.values().flatten().cloned().collect())?;
                        let history=self.repo.relay_history(&filters).await;
                        drop(_query);
                        match history {
                         Ok(events)=>{
                          tokio::time::timeout(Duration::from_secs(10),async {
                           for event in events{ensure!(self.delivery_allowed(&keys).await?,"read permission revoked");send(socket,json!(["EVENT",name,event])).await?;}
                           send(socket,json!(["EOSE",name])).await?;Ok::<(),anyhow::Error>(())
                          }).await??;

                         }
                         Err(_)=>{
                           subscriptions.remove(name);
                           receiver.replace(subscriptions.values().flatten().cloned().collect())?;
                           send(socket,json!(["CLOSED",name,"error: query budget or database unavailable"])).await?;
                         },
                        }
                       }
                       Some("EVENT") if parts.len()==2=>{
                        let event=match serde_json::from_value::<Event>(parts[1].clone()){Ok(e)=>e,Err(_)=>{send(socket,json!(["NOTICE","invalid: event required"])).await?;continue;}};
                        if !keys.contains(&event.pubkey){send(socket,json!(["OK",event.id,false,"auth-required: authenticate this event's key"])).await?;continue;}
                        let result=self.publish(&event).await;
                        send(socket,json!(["OK",event.id,result.is_ok(),result.err().map(|error|if crate::db::is_database_error(&error){"error: metadata store unavailable"}else if error.is::<tokio::sync::TryAcquireError>(){"rate-limited: event budget exhausted"}else{"restricted: event rejected by admission or storage policy"}).unwrap_or("")])).await?;
                       }
                       _=>send(socket,json!(["NOTICE","invalid: unsupported command"])).await?,
                      }
                     }
                    }
        }
    }
    async fn can_read(&self, keys: &BTreeSet<String>) -> Result<bool> {
        if self.peer_only {
            let keys: Vec<Vec<u8>> = keys
                .iter()
                .map(hex::decode)
                .collect::<std::result::Result<_, _>>()?;
            return Ok(self.repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM approved_peers WHERE service_key=ANY($1) AND allowed)",&[&keys]).await?.get(0));
        }
        match self.read_policy {
            ReadPolicy::Public => Ok(true),
            ReadPolicy::Authenticated => Ok(!keys.is_empty()),
            ReadPolicy::Admitted => {
                let keys: Vec<Vec<u8>> = keys
                    .iter()
                    .map(hex::decode)
                    .collect::<std::result::Result<_, _>>()?;
                Ok(self.repo.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM account_permissions WHERE publisher=ANY($1) AND allowed)",&[&keys]).await?.get(0))
            }
        }
    }
    async fn authenticate(
        &self,
        event: &Event,
        challenge: &str,
        keys: &BTreeSet<String>,
    ) -> Result<()> {
        ensure!(
            keys.contains(&event.pubkey) || keys.len() < 8,
            "authentication key budget"
        );
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            event.kind == 22242 && event.created_at.abs_diff(auth::now()) <= 60,
            "authentication age or kind"
        );
        ensure!(
            event.one("challenge").map_err(anyhow::Error::msg)? == challenge,
            "challenge"
        );
        let provided = url::Url::parse(event.one("relay").map_err(anyhow::Error::msg)?)?;
        ensure!(provided == url::Url::parse(&self.url)?, "relay URL");
        ensure!(
            self.repo
                .authorization_clock_allowed(event.created_at)
                .await?,
            "restore barrier"
        );
        if self.peer_only {
            self.repo
                .approved_peer(
                    hex::decode(&event.pubkey)?
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("peer key"))?,
                )
                .await?;
        }
        Ok(())
    }
    async fn publish(&self, event: &Event) -> Result<()> {
        ensure!(!self.peer_only, "peer event interface is read-only");
        let _slot = self.writes.clone().try_acquire_owned()?;
        ensure!(self.projection_enabled, "projection disabled");
        event.verify().map_err(anyhow::Error::msg)?;
        if matches!(event.kind, 1001 | 1002) {
            self.publications
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("publication unavailable"))?
                .publish(event)
                .await?;
            return Ok(());
        }
        let permit = self
            .repo
            .account_permit(hex::decode(&event.pubkey)?.try_into().unwrap())
            .await?;
        match event.kind {
            0 | 3 | 10002 => {
                ensure!(
                    self.repo.stage_account_metadata(&permit, event).await?
                        != crate::accounts::StageOutcome::Superseded,
                    "superseded"
                );
            }
            1 => {
                self.repo.stage_personal_note(&permit, event).await?;
            }
            7 | 1111 => {
                self.repo.stage_social(&permit, event).await?;
            }
            5 => {
                self.repo.delete_ordinary(&permit, event).await?;
            }
            11999 => {
                let result = self
                    .libraries
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("library unavailable"))?
                    .commit(permit, event.clone(), crate::library::ExpectedHead::Any)
                    .await?;
                ensure!(
                    result != crate::accounts::StageOutcome::Superseded,
                    "superseded"
                );
            }
            _ => anyhow::bail!("unsupported kind"),
        }
        Ok(())
    }
}
