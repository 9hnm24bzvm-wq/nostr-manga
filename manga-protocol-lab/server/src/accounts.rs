//! Local account admission and replaceable metadata. No implicit permission is
//! granted by federation. Library kind 11999 must use verified storage admission.
use crate::read_diagnostics::{Phase, during, phase};
use crate::{
    auth::Event,
    db::{Id, Repository},
};
use anyhow::{Result, ensure};

#[derive(Debug, thiserror::Error)]
#[error("account is not admitted")]
pub struct AccountDenied;

#[derive(Debug, thiserror::Error)]
#[error("metadata changed since review")]
pub struct AccountHeadChanged;

#[derive(Clone)]
pub struct AccountPermit {
    pub(crate) publisher: Id,
    pub(crate) version: i64,
    pub(crate) account_peer: Option<crate::peers::Peer>,
    pub(crate) sync_lease: Option<crate::db::Lease>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum StageOutcome {
    Accepted,
    Current,
    Superseded,
}

impl Repository {
    /// Operator-only control-plane operation. HTTP must authenticate the operator.
    pub async fn set_account_admission(&self, publisher: Id, allowed: bool) -> Result<()> {
        self.pool.get().await?.execute("INSERT INTO account_permissions(publisher,allowed,enrollment_blocked) VALUES($1,$2,NOT $2) ON CONFLICT(publisher) DO UPDATE SET allowed=EXCLUDED.allowed,enrollment_blocked=EXCLUDED.enrollment_blocked,version=account_permissions.version+1", &[&publisher.as_slice(),&allowed]).await?;
        Ok(())
    }
    /// HTTP callers must bind the authenticated request key to this publisher.
    pub async fn account_permit(&self, publisher: Id) -> Result<AccountPermit> {
        let row = {
            let client = during(Phase::PoolCheckout, self.pool.get()).await?;
            during(
                Phase::DbRoundtrip,
                client.query_opt(
                    "SELECT version FROM account_permissions WHERE publisher=$1 AND allowed",
                    &[&publisher.as_slice()],
                ),
            )
            .await?
        };
        let _decode = phase(Phase::DecodeResponse);
        let row = row.ok_or(AccountDenied)?;
        Ok(AccountPermit {
            publisher,
            version: row.get(0),
            account_peer: None,
            sync_lease: None,
        })
    }
    pub async fn stage_account_metadata(
        &self,
        permit: &AccountPermit,
        event: &Event,
    ) -> Result<StageOutcome> {
        self.stage_account_metadata_expected(permit, event, None).await
    }
    /// HTTP proof binds expected_head to the full request body. None retains
    /// legacy relay semantics; Some(None) means the editor observed no head.
    pub async fn stage_account_metadata_expected(
        &self,
        permit: &AccountPermit,
        event: &Event,
        expected_head: Option<Option<Id>>,
    ) -> Result<StageOutcome> {
        ensure!(
            matches!(event.kind, 0 | 3 | 10002),
            "event requires a dedicated workflow"
        );
        ensure!(
            event.content.len() <= 1_048_576
                && event.tags.len() <= 4096
                && event
                    .tags
                    .iter()
                    .all(|t| t.len() <= 32 && t.iter().all(|s| s.len() <= 1024)),
            "metadata budget"
        );
        let raw = serde_json::to_value(event)?;
        ensure!(
            serde_json::to_vec(&raw)?.len() <= 1_048_576,
            "event size budget"
        );
        ensure!(
            event.values("expiration").is_empty(),
            "expiration requires a dedicated lifecycle"
        );
        event.verify().map_err(anyhow::Error::msg)?;
        ensure!(
            event.pubkey == hex::encode(permit.publisher),
            "account author mismatch"
        );
        if event.kind == 0 {
            ensure!(
                serde_json::from_str::<serde_json::Value>(&event.content)?.is_object(),
                "profile must be a JSON object"
            );
        }
        let id = hex::decode(&event.id)?;
        let kind = i32::try_from(event.kind)?;
        let created = i64::try_from(event.created_at)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // This author-scoped lock serializes all local head changes with revocation.
        let _admission = tx
            .query_one(
                "SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR UPDATE",
                &[&permit.publisher.as_slice()],
            )
            .await?;
        check_account_authority_tx(&tx, permit).await?;
        let old=tx.query_opt("SELECT winner_at,winner_id,event_id FROM account_heads WHERE publisher=$1 AND kind=$2", &[&permit.publisher.as_slice(),&kind]).await?;
        if let Some(ref old) = old {
            let clock: i64 = old.get(0);
            let winner: Vec<u8> = old.get(1);
            let current: Option<Vec<u8>> = old.get(2);
            if current.as_ref() == Some(&id) {
                return Ok(StageOutcome::Current);
            }
            if let Some(expected) = expected_head.as_ref() {
                if current.as_deref() != expected.as_ref().map(|id| id.as_slice()) { return Err(AccountHeadChanged.into()); }
            }
            if created < clock || (created == clock && id >= winner) {
                return Ok(StageOutcome::Superseded);
            }
        }
        if old.is_none() && expected_head.as_ref().is_some_and(|expected| expected.is_some()) { return Err(AccountHeadChanged.into()); }
        crate::ordinary_deletion::admit_event_tx(&tx, event).await?;
        // A collision with a differently managed lifecycle is never resurrected.
        tx.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state) VALUES($1,$2,$3,$4,$5,'hosted')", &[&id,&permit.publisher.as_slice(),&kind,&created,&raw]).await?;
        tx.execute(
            "INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'import',$2)",
            &[&id, &raw],
        )
        .await?;
        if let Some(old) = old
            && let Some(old_id) = old.get::<_, Option<Vec<u8>>>(2)
        {
            let changed=tx.execute("UPDATE events SET state='withdrawing',version=version+1 WHERE id=$1 AND state='hosted'", &[&old_id]).await?;
            ensure!(changed == 1, "account head lifecycle mismatch");
            tx.execute("INSERT INTO backend_outbox(event_id,action,payload,event_version) SELECT id,'delete',raw_event,version FROM events WHERE id=$1", &[&old_id]).await?;
        }
        tx.execute("INSERT INTO account_heads(publisher,kind,winner_at,winner_id,event_id) VALUES($1,$2,$3,$4,$4) ON CONFLICT(publisher,kind) DO UPDATE SET winner_at=EXCLUDED.winner_at,winner_id=EXCLUDED.winner_id,event_id=EXCLUDED.event_id", &[&permit.publisher.as_slice(),&kind,&created,&id]).await?;
        check_account_lease_tx(&tx, permit).await?;
        tx.commit().await?;
        Ok(StageOutcome::Accepted)
    }
}

/// Authority fence shared by account-owned storage workflows. Always lock this
/// before blob rows; an imported author never receives this capability implicitly.
pub(crate) async fn check_account_tx(
    tx: &tokio_postgres::Transaction<'_>,
    permit: &AccountPermit,
) -> Result<()> {
    let row = tx
        .query_opt(
            "SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR SHARE",
            &[&permit.publisher.as_slice()],
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("account not admitted"))?;
    ensure!(
        row.get::<_, bool>(0) && row.get::<_, i64>(1) == permit.version,
        "account permission changed"
    );
    Ok(())
}

impl Repository {
    /// Peer authority permits guarded signed-account import; never personal login.
    pub(crate) async fn peer_account_permit(
        &self,
        publisher: Id,
        peer: crate::peers::Peer,
    ) -> Result<AccountPermit> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        tx.execute("INSERT INTO account_permissions(publisher,allowed,enrollment_blocked) VALUES($1,FALSE,FALSE) ON CONFLICT DO NOTHING",&[&publisher.as_slice()]).await?;
        let version = tx
            .query_one(
                "SELECT version FROM account_permissions WHERE publisher=$1",
                &[&publisher.as_slice()],
            )
            .await?
            .get(0);
        let permit = AccountPermit {
            publisher,
            version,
            account_peer: Some(peer),
            sync_lease: None,
        };
        check_account_authority_tx(&tx, &permit).await?;
        tx.commit().await?;
        Ok(permit)
    }
    pub(crate) async fn verify_account_storage_permit(&self, permit: &AccountPermit) -> Result<()> {
        if let Some(lease) = &permit.sync_lease {
            ensure!(
                self.lease_current(lease).await?,
                "synchronization lease changed"
            );
        }
        // Advisory preflight only. Final storage/root transactions take the
        // authority locks again after I/O, including peer epoch fencing.
        if let Some(peer) = &permit.account_peer {
            let current = self.approved_peer(peer.key).await?;
            ensure!(
                current.version == peer.version && current.endpoint == peer.endpoint,
                "peer account authority changed"
            );
        } else {
            let current = self.account_permit(permit.publisher).await?;
            ensure!(
                current.version == permit.version,
                "account permission changed"
            );
        }
        Ok(())
    }
}
pub(crate) async fn check_account_lease_tx(
    tx: &tokio_postgres::Transaction<'_>,
    permit: &AccountPermit,
) -> Result<()> {
    if let Some(lease) = &permit.sync_lease {
        crate::db::check_lease_tx(tx, lease).await?;
    }
    Ok(())
}
pub(crate) async fn check_account_authority_tx(
    tx: &tokio_postgres::Transaction<'_>,
    permit: &AccountPermit,
) -> Result<()> {
    check_account_lease_tx(tx, permit).await?;
    if let Some(peer) = &permit.account_peer {
        tx.query_one(
            "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR SHARE",
            &[&permit.publisher.as_slice()],
        )
        .await?;
        let row=tx.query_opt("SELECT allowed,version,endpoint FROM approved_peers WHERE service_key=$1 FOR SHARE",&[&peer.key.as_slice()]).await?.ok_or_else(||anyhow::anyhow!("peer missing"))?;
        ensure!(
            row.get::<_, bool>(0)
                && row.get::<_, i64>(1) == peer.version
                && row.get::<_, String>(2) == peer.endpoint,
            "peer library authority changed"
        );
        Ok(())
    } else {
        check_account_tx(tx, permit).await
    }
}

// APPEND/merge into original accounts.rs. Preserve the base's existing account
// admission APIs. This is recovered from visible source and needs fresh tests.
use serde_json::{Value, json};
use std::sync::Arc;
pub struct PublicAccountProfile {
    pub publisher: Id,
    pub known: bool,
    pub event: Option<Arc<Event>>,
    pub metadata: Value,
}
impl PublicAccountProfile {
    pub fn display_name(&self) -> Option<&str> {
        ["display_name", "name"].into_iter().find_map(|key| {
            self.metadata
                .get(key)
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty() && name.len() <= 512)
        })
    }
}
impl Repository {
    pub async fn public_account_profile(&self, publisher: Id) -> Result<PublicAccountProfile> {
        let (known, head): (bool, Option<Vec<u8>>) = {
            let row=self.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM events WHERE publisher=$1 AND state='hosted'),(SELECT event_id FROM account_heads WHERE publisher=$1 AND kind=0)",&[&publisher.as_slice()]).await?;
            (row.get(0), row.get(1))
        };
        let event = if let Some(head) = &head {
            let id: Id = head.as_slice().try_into()?;
            self.visible_event(id).await?
        } else {
            None
        };
        let metadata = if let Some(event) = &event {
            ensure!(
                event.kind == 0 && event.pubkey == hex::encode(publisher),
                "stored profile identity mismatch"
            );
            ensure!(
                event.content.len() <= 1048576,
                "profile metadata byte budget"
            );
            let metadata: Value = serde_json::from_str(&event.content)?;
            ensure!(metadata.is_object(), "profile metadata object required");
            metadata
        } else {
            json!({})
        };
        let current: Option<Vec<u8>> = self
            .pool
            .get()
            .await?
            .query_one(
                "SELECT (SELECT event_id FROM account_heads WHERE publisher=$1 AND kind=0)",
                &[&publisher.as_slice()],
            )
            .await?
            .get(0);
        ensure!(current == head, "public profile head changed");
        if let Some(event) = &event {
            ensure!(
                self.visible_event(hex::decode(&event.id)?.try_into().unwrap())
                    .await?
                    .is_some(),
                "public profile visibility changed"
            );
        }
        Ok(PublicAccountProfile {
            publisher,
            known,
            event,
            metadata,
        })
    }
}
#[cfg(test)]
mod public_profile_tests {
    use super::*;
    #[test]
    fn display_names_are_bounded_public_metadata_only() {
        let mut p = PublicAccountProfile {
            publisher: [1; 32],
            known: true,
            event: None,
            metadata: json!({"display_name":"Public display","name":"secondary"}),
        };
        assert_eq!(p.display_name(), Some("Public display"));
        p.metadata = json!({"display_name":"","name":"secondary"});
        assert_eq!(p.display_name(), Some("secondary"));
        for m in [
            json!({}),
            json!({"display_name":false,"name":123}),
            json!({"display_name":"a".repeat(513),"name":null}),
        ] {
            p.metadata = m;
            assert!(p.display_name().is_none());
        }
    }
}
