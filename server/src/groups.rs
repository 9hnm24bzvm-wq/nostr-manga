//! Instance-custodied ordinary Nostr group keys. Purpose-specific operations bind
//! member authorization; a recovered key does not recreate membership authority.
use crate::{
    auth::{self, Event},
    db::{Id, Repository},
    publication::Publications,
    upload::UploadStore,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::json;
use std::{fs, io::Write, path::PathBuf, sync::Arc};
use zeroize::Zeroizing;
fn id(s: &str) -> Result<Id> {
    ensure!(auth::valid_hash(s), "invalid group/member identifier");
    Ok(hex::decode(s)?.try_into().unwrap())
}
fn allowed(role: &str, action: &str) -> bool {
    role == "admin"
        || (action == "publish" && role == "publisher")
        || (action == "remove" && role == "remove")
}
pub(crate) async fn check_group_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    group: Id,
    member: Id,
    version: i64,
    action: &str,
) -> Result<()> {
    let row = tx
        .query_opt(
            "SELECT enabled,version FROM managed_groups WHERE group_key=$1 FOR SHARE",
            &[&group.as_slice()],
        )
        .await?
        .context("group is not managed here")?;
    ensure!(
        row.get::<_, bool>(0) && row.get::<_, i64>(1) == version,
        "group policy changed or disabled"
    );
    let role: Option<String> = tx
        .query_opt(
            "SELECT role FROM group_members WHERE group_key=$1 AND member=$2",
            &[&group.as_slice(), &member.as_slice()],
        )
        .await?
        .map(|r| r.get(0));
    ensure!(
        role.as_deref().is_some_and(|role| allowed(role, action)),
        "member role does not permit operation"
    );
    Ok(())
}
struct KeyStore {
    root: PathBuf,
}
impl KeyStore {
    fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        Ok(Self { root })
    }
    fn path(&self, group: Id) -> PathBuf {
        self.root.join(format!("{}.key", hex::encode(group)))
    }
    fn read(&self, group: Id) -> Result<Zeroizing<String>> {
        let path = self.path(group);
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.file_type().is_file() && metadata.len() <= 128,
            "invalid group key file"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o077 == 0,
                "group key file must not be group/world accessible"
            );
        }
        let secret = Zeroizing::new(fs::read_to_string(path)?);
        let secret = Zeroizing::new(secret.trim().to_owned());
        let event = auth::sign(&secret, 1, vec![], String::new()).map_err(anyhow::Error::msg)?;
        ensure!(
            id(&event.pubkey)? == group,
            "group key reference identity mismatch"
        );
        Ok(secret)
    }
    fn create(&self) -> Result<Id> {
        let secret = loop {
            let bytes: [u8; 32] = rand::random();
            if secp256k1::SecretKey::from_slice(&bytes).is_ok() {
                break Zeroizing::new(hex::encode(bytes));
            }
        };
        let key = id(&auth::sign(&secret, 1, vec![], String::new())
            .map_err(anyhow::Error::msg)?
            .pubkey)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(self.path(key))?;
        file.write_all(secret.as_bytes())?;
        file.sync_all()?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(key)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupPublication {
    pub kind: u32,
    pub manifest: String,
    pub version: u64,
    /// Persist with the command before sending; retries then keep the Nostr ID.
    #[serde(default)]
    pub created_at: Option<u64>,
}
pub struct Groups {
    repo: Repository,
    keys: Arc<KeyStore>,
    storage: Arc<UploadStore>,
    publications: Arc<Publications>,
}
impl Groups {
    pub fn open(
        repo: Repository,
        path: PathBuf,
        storage: Arc<UploadStore>,
        publications: Arc<Publications>,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            repo,
            keys: Arc::new(KeyStore::open(path)?),
            storage,
            publications,
        }))
    }
    pub async fn create(&self, administrator: Id) -> Result<Id> {
        secp256k1::XOnlyPublicKey::from_slice(&administrator)?;
        let keys = self.keys.clone();
        let group = crate::node::spawn_blocking_owned_scope(move || keys.create()).await??;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        tx.execute(
            "INSERT INTO managed_groups(group_key,key_ref) VALUES($1,$2)",
            &[&group.as_slice(), &hex::encode(group)],
        )
        .await?;
        tx.execute(
            "INSERT INTO group_members(group_key,member,role) VALUES($1,$2,'admin')",
            &[&group.as_slice(), &administrator.as_slice()],
        )
        .await?;
        crate::mangadex_compat::register(
            &tx,
            "group",
            &hex::encode(group),
            json!({"nostrPubkey":hex::encode(group)}),
        )
        .await?;
        tx.commit().await?;
        Ok(group)
    }
    pub async fn register_recovered_key(&self, group: Id) -> Result<()> {
        let keys = self.keys.clone();
        crate::node::spawn_blocking_owned_scope(move || keys.read(group)).await??;
        self.repo.pool.get().await?.execute("INSERT INTO managed_groups(group_key,key_ref,enabled) VALUES($1,$2,FALSE) ON CONFLICT(group_key) DO NOTHING", &[&group.as_slice(),&hex::encode(group)]).await?;
        Ok(())
    }
    pub async fn bootstrap_recovered(&self, group: Id, administrator: Id) -> Result<()> {
        secp256k1::XOnlyPublicKey::from_slice(&administrator)?;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        let enabled: bool = tx
            .query_one(
                "SELECT enabled FROM managed_groups WHERE group_key=$1 FOR UPDATE",
                &[&group.as_slice()],
            )
            .await?
            .get(0);
        let members: i64 = tx
            .query_one(
                "SELECT COUNT(*) FROM group_members WHERE group_key=$1",
                &[&group.as_slice()],
            )
            .await?
            .get(0);
        ensure!(
            !enabled && members == 0,
            "bootstrap only applies to a disabled key-only recovery without members"
        );
        tx.execute(
            "INSERT INTO group_members(group_key,member,role) VALUES($1,$2,'admin')",
            &[&group.as_slice(), &administrator.as_slice()],
        )
        .await?;
        tx.execute(
            "UPDATE managed_groups SET enabled=TRUE,version=version+1 WHERE group_key=$1",
            &[&group.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn set_enabled(&self, group: Id, enabled: bool) -> Result<()> {
        let changed = self
            .repo
            .pool
            .get()
            .await?
            .execute(
                "UPDATE managed_groups SET enabled=$2,version=version+1 WHERE group_key=$1",
                &[&group.as_slice(), &enabled],
            )
            .await?;
        ensure!(changed == 1, "group not managed here");
        Ok(())
    }
    pub async fn authorize(&self, group: Id, member: Id, action: &str) -> Result<i64> {
        let row=self.repo.pool.get().await?.query_opt("SELECT g.version,m.role FROM managed_groups g JOIN group_members m ON m.group_key=g.group_key WHERE g.group_key=$1 AND g.enabled AND m.member=$2", &[&group.as_slice(),&member.as_slice()]).await?.context("member is not admitted")?;
        ensure!(
            allowed(&row.get::<_, String>(1), action),
            "member role does not permit operation"
        );
        Ok(row.get(0))
    }
    pub async fn set_member(
        &self,
        group: Id,
        actor: Id,
        version: i64,
        member: Id,
        role: Option<String>,
    ) -> Result<()> {
        secp256k1::XOnlyPublicKey::from_slice(&member)?;
        ensure!(
            role.as_deref()
                .is_none_or(|v| matches!(v, "admin" | "publisher" | "remove")),
            "invalid member role"
        );
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        // All role mutations take the group row first; a commit's shared lock prevents
        // policy changing between its final authorization check and publication.
        tx.query_one(
            "SELECT version FROM managed_groups WHERE group_key=$1 FOR UPDATE",
            &[&group.as_slice()],
        )
        .await?;
        check_group_tx(&tx, group, actor, version, "admin").await?;
        let old: Option<String> = tx
            .query_opt(
                "SELECT role FROM group_members WHERE group_key=$1 AND member=$2",
                &[&group.as_slice(), &member.as_slice()],
            )
            .await?
            .map(|r| r.get(0));
        if old.as_deref() == Some("admin") && role.as_deref() != Some("admin") {
            let admins: i64 = tx
                .query_one(
                    "SELECT COUNT(*) FROM group_members WHERE group_key=$1 AND role='admin'",
                    &[&group.as_slice()],
                )
                .await?
                .get(0);
            ensure!(admins > 1, "cannot remove the last group administrator");
        }
        if let Some(role) = role {
            tx.execute("INSERT INTO group_members(group_key,member,role) VALUES($1,$2,$3) ON CONFLICT(group_key,member) DO UPDATE SET role=EXCLUDED.role", &[&group.as_slice(),&member.as_slice(),&role]).await?;
        } else {
            tx.execute(
                "DELETE FROM group_members WHERE group_key=$1 AND member=$2",
                &[&group.as_slice(), &member.as_slice()],
            )
            .await?;
        }
        tx.execute(
            "UPDATE managed_groups SET version=version+1 WHERE group_key=$1",
            &[&group.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn members(&self, group: Id, actor: Id, version: i64) -> Result<serde_json::Value> {
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_group_tx(&tx, group, actor, version, "admin").await?;
        let rows=tx.query("SELECT member,role FROM group_members WHERE group_key=$1 ORDER BY member LIMIT 1001", &[&group.as_slice()]).await?;
        ensure!(rows.len() <= 1000, "member listing budget");
        let members:Vec<_>=rows.iter().map(|row|json!({"member":hex::encode(row.get::<_,Vec<u8>>(0)),"role":row.get::<_,String>(1)})).collect();
        tx.commit().await?;
        Ok(json!({"group":hex::encode(group),"members":members}))
    }
    pub async fn upload(
        &self,
        group: Id,
        member: Id,
        version: i64,
        hash: Id,
        bytes: &[u8],
    ) -> Result<()> {
        let intent = self.storage.put(hash, bytes).await?;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_group_tx(&tx, group, member, version, "publish").await?;
        let blob = tx
            .query_opt(
                "SELECT owner,state,generation FROM blob_intents WHERE hash=$1 FOR SHARE",
                &[&hash.as_slice()],
            )
            .await?
            .context("group blob disappeared")?;
        ensure!(
            blob.get::<_, Vec<u8>>(0) == self.storage.owner()
                && blob.get::<_, String>(1) == "available"
                && blob.get::<_, Vec<u8>>(2) == intent.key.generation,
            "group blob unavailable"
        );
        ensure!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM blob_reclaims WHERE hash=$1)",
                &[&hash.as_slice()]
            )
            .await?
            .get::<_, bool>(0),
            "group blob reclamation pending"
        );
        tx.execute(
            "INSERT INTO group_blob_grants(group_key,hash,generation,lease_until) SELECT $1,hash,generation,staging_until FROM blob_intents WHERE hash=$2 ON CONFLICT(group_key,hash) DO UPDATE SET generation=EXCLUDED.generation,lease_until=EXCLUDED.lease_until",
            &[&group.as_slice(), &hash.as_slice()],
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn delete_upload(&self, group: Id, member: Id, version: i64, hash: Id) -> Result<()> {
        self.storage
            .delete_staged(
                crate::staged_deletion::DeleteAuthority::Group {
                    group,
                    member,
                    version,
                },
                hash,
            )
            .await
    }
    /// Admin-only public kind0 metadata. This is not an arbitrary signing API.
    pub async fn profile(
        &self,
        group: Id,
        member: Id,
        version: i64,
        metadata: serde_json::Value,
        expected: Option<Id>,
    ) -> Result<Event> {
        ensure!(metadata.is_object(), "profile metadata must be an object");
        let content = serde_json::to_string(&metadata)?;
        ensure!(content.len() <= 16384, "profile metadata byte budget");
        let keys = self.keys.clone();
        let secret = crate::node::spawn_blocking_owned_scope(move || keys.read(group)).await??;
        let mut c = self.repo.pool.get().await?;
        let tx = c.transaction().await?;
        check_group_tx(&tx, group, member, version, "profile").await?;
        // A profile does not grant the group key independent account posting rights.
        tx.execute("INSERT INTO account_permissions(publisher,allowed,enrollment_blocked) VALUES($1,FALSE,FALSE) ON CONFLICT DO NOTHING",&[&group.as_slice()]).await?;
        tx.query_one(
            "SELECT publisher FROM account_permissions WHERE publisher=$1 FOR UPDATE",
            &[&group.as_slice()],
        )
        .await?;
        let old = tx
            .query_opt(
                "SELECT winner_at,event_id FROM account_heads WHERE publisher=$1 AND kind=0",
                &[&group.as_slice()],
            )
            .await?;
        let old_id = old
            .as_ref()
            .and_then(|row| row.get::<_, Option<Vec<u8>>>(1));
        if expected.is_some_and(|id| old_id.as_deref() != Some(id.as_slice())) {
            return Err(ProfileChanged.into());
        }
        let now = auth::now();
        let previous = old.as_ref().map(|row| row.get::<_, i64>(0)).unwrap_or(0);
        let created = now.max(
            u64::try_from(previous)?
                .checked_add(1)
                .context("profile clock overflow")?,
        );
        if created > now.saturating_add(60) {
            return Err(ProfileClockAhead.into());
        }
        let event =
            auth::sign_at(&secret, 0, vec![], content, created).map_err(anyhow::Error::msg)?;
        crate::ordinary_deletion::admit_event_tx(&tx, &event).await?;
        let key = id(&event.id)?;
        let raw = serde_json::to_value(&event)?;
        tx.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state) VALUES($1,$2,0,$3,$4,'hosted')",&[&key.as_slice(),&group.as_slice(),&(created as i64),&raw]).await?;
        tx.execute(
            "INSERT INTO backend_outbox(event_id,action,payload) VALUES($1,'import',$2)",
            &[&key.as_slice(), &raw],
        )
        .await?;
        if let Some(old) = old_id {
            ensure!(tx.execute("UPDATE events SET state='withdrawing',version=version+1 WHERE id=$1 AND state='hosted'",&[&old]).await?==1,"profile head lifecycle changed");
            tx.execute("INSERT INTO backend_outbox(event_id,action,payload,event_version) SELECT id,'delete',raw_event,version FROM events WHERE id=$1",&[&old]).await?;
        }
        tx.execute("INSERT INTO account_heads(publisher,kind,winner_at,winner_id,event_id) VALUES($1,0,$2,$3,$3) ON CONFLICT(publisher,kind) DO UPDATE SET winner_at=EXCLUDED.winner_at,winner_id=EXCLUDED.winner_id,event_id=EXCLUDED.event_id",&[&group.as_slice(),&(created as i64),&key.as_slice()]).await?;
        record_action_tx(&tx, group, member, version, "profile", key).await?;
        tx.commit().await?;
        Ok(event)
    }
    pub async fn publish(
        &self,
        group: Id,
        member: Id,
        version: i64,
        request: GroupPublication,
    ) -> Result<Event> {
        ensure!(
            matches!(request.kind, 1001 | 1002) && request.version == 1,
            "group API signs only supported publication kinds"
        );
        id(&request.manifest)?;
        let keys = self.keys.clone();
        let secret = crate::node::spawn_blocking_owned_scope(move || keys.read(group)).await??;
        let created_at = request.created_at.unwrap_or_else(auth::now);
        ensure!(
            created_at <= auth::now().saturating_add(60),
            "group publication timestamp too far ahead"
        );
        let event = auth::sign_at(
            &secret,
            request.kind,
            vec![],
            json!({"manifest":request.manifest,"version":1}).to_string(),
            created_at,
        )
        .map_err(anyhow::Error::msg)?;
        event.verify().map_err(anyhow::Error::msg)?;
        let prepared = self
            .publications
            .prepare_for_group(&event, group, member, version)
            .await?;
        self.publications.commit(prepared).await?;
        Ok(event)
    }
    pub async fn deletion_request(
        &self,
        group: Id,
        member: Id,
        version: i64,
        targets: Vec<String>,
        reason: String,
    ) -> Result<Event> {
        ensure!(
            !targets.is_empty() && targets.len() <= 100 && reason.len() <= 4096,
            "deletion request budget"
        );
        for target in &targets {
            id(target)?;
        }
        let keys = self.keys.clone();
        let secret = crate::node::spawn_blocking_owned_scope(move || keys.read(group)).await??;
        let event = auth::sign(
            &secret,
            5,
            targets
                .iter()
                .map(|id| vec!["e".into(), id.clone()])
                .collect(),
            reason,
        )
        .map_err(anyhow::Error::msg)?;
        let mut client = self.repo.pool.get().await?;
        let tx = client.transaction().await?;
        check_group_tx(&tx, group, member, version, "remove").await?;
        let request = id(&event.id)?;
        let inserted=tx.execute("INSERT INTO deletion_requests(id,author,raw_event) VALUES($1,$2,$3) ON CONFLICT(id) DO NOTHING", &[&request.as_slice(),&group.as_slice(),&serde_json::to_value(&event)?]).await?;
        if inserted == 1 {
            record_action_tx(&tx, group, member, version, "deletion_request", request).await?;
            for target in targets {
                tx.execute("INSERT INTO deletion_targets(request_id,event_id) VALUES($1,$2) ON CONFLICT DO NOTHING", &[&request.as_slice(),&id(&target)?.as_slice()]).await?;
            }
        }
        tx.commit().await?;
        Ok(event)
    }
    pub fn max_blob_bytes(&self) -> u64 {
        self.storage.max_blob_bytes
    }
    pub fn upload_slots(&self) -> Arc<tokio::sync::Semaphore> {
        self.storage.slots.clone()
    }
    pub fn publication_slots(&self) -> Arc<tokio::sync::Semaphore> {
        self.publications.slots.clone()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("group audit persistence unavailable")]
pub struct AuditUnavailable;

/// Local first-commit attribution. Never projected or exported as a public event.
pub(crate) async fn record_action_tx(
    tx: &deadpool_postgres::Transaction<'_>,
    group: Id,
    member: Id,
    version: i64,
    action: &str,
    event: Id,
) -> Result<()> {
    tx.execute("INSERT INTO group_action_audit(group_key,member,action,event_id,permission_version) VALUES($1,$2,$3,$4,$5) ON CONFLICT(group_key,action,event_id) DO NOTHING", &[&group.as_slice(), &member.as_slice(), &action, &event.as_slice(), &version]).await.context(AuditUnavailable)?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
#[error("managed group profile changed")]
pub struct ProfileChanged;
#[derive(Debug, thiserror::Error)]
#[error("managed group profile clock is too far ahead")]
pub struct ProfileClockAhead;
