//! Private durable upload scheduling. An accepted schedule is not a published
//! chapter. Source signing/storage and all public catalogue work remain due-only.
use super::{auth::{self, BoundAccountRead, BoundAccountWrite}, PublicApiError, PublicApiResult};
use crate::{domains::{catalog::{CatalogRepository, entity_mutations::{sealed, CatalogMutationActor}, validation::LanguageContract}, uploads::{sessions as core, storage}}, error::{ApiError, ApiResult}, state::AppState};
use manga_node::http_auth::AdmittedHttpCapability;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_postgres::{Row, Transaction};

fn digest(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }
fn payload_hash(payload: &core::Commit) -> PublicApiResult<String> {
    Ok(digest(&serde_json::to_vec(payload).map_err(|_| PublicApiError::bad_request("Invalid commit"))?))
}

/// This is not an HTTP capability, native session, or deserializable authority.
/// Construction is private to the fresh, locked durable-intent execution gate.
pub(crate) struct ScheduledUploadActor {
    account: BoundAccountRead, session: String, token: String, hash: String,
    chapter: String, signer: String, group: Option<String>, parent_manifest: Option<Vec<u8>>,
}
impl sealed::Sealed for ScheduledUploadActor {}
impl CatalogMutationActor for ScheduledUploadActor {
    fn language_contract(&self) -> LanguageContract { LanguageContract::Public }
    fn user_id(&self) -> &str { self.account.user_id() }
    fn username(&self) -> &str { self.account.username() }
    fn is_moderator(&self) -> bool { self.account.is_moderator() }
    fn has_perm(&self, permission: &str) -> bool { self.account.has_perm(permission) }
}
impl ScheduledUploadActor {
    pub(crate) fn parent_manifest(&self) -> Option<&[u8]> { self.parent_manifest.as_deref() }
    pub(crate) fn chapter_id(&self) -> &str { &self.chapter }
    pub(crate) fn source_identity(&self) -> (&str, Option<&str>) { (&self.signer, self.group.as_deref()) }
    pub(crate) async fn session_for(&self, tx: &Transaction<'_>, id: &str, hash: &str) -> ApiResult<Row> {
        if id != self.session || hash != self.hash { return Err(ApiError::forbidden("Scheduled input mismatch")); }
        let row = tx.query_opt("SELECT s.manga_id,s.chapter_id,s.edit_version,s.groups,s.state,s.committed_chapter_id,s.commit_hash,true AS active FROM platform.upload_sessions s JOIN platform.scheduled_uploads i ON i.session_id=s.id WHERE s.id=$1 AND s.owner_id=$2 AND s.state='scheduled' AND i.state='starting' AND i.started_at IS NOT NULL AND i.lease_token=$3 AND i.payload_hash=$4 AND i.session_version=s.version AND i.due_at<=clock_timestamp() FOR UPDATE OF s,i", &[&id,&self.user_id(),&self.token,&hash]).await?
            .ok_or_else(|| ApiError::conflict("Scheduled publication claim is no longer current"))?;
        Ok(row)
    }
}

// Claims use only the intent lock and commit before this full execution order:
// service SHARE -> account admission SHARE -> binding SHARE/user NO KEY UPDATE
// -> session UPDATE -> intent UPDATE -> parent/chapter -> sorted group/source.
const INTENT_SELECT: &str = "SELECT i.*,i.chapter_uuid::text AS reserved_uuid,i.started_at IS NOT NULL AS started,i.due_at<=clock_timestamp() AS due,i.retry_until>clock_timestamp() AS within_retry,extract(epoch FROM i.retry_until-i.due_at)::bigint AS retry_seconds FROM platform.scheduled_uploads i WHERE i.session_id=$1";
struct Intent {
    session: String, owner: String, author: String, service: String, origin: String,
    admission: i64, hash: String, payload: Value, version: i32,
    signer: String, group: Option<String>, parent_source: Option<String>,
    parent_fingerprint: String, parent_manifest: Option<Vec<u8>>, parent_cover: Option<String>, files_fingerprint: String,
    chapter: String, uuid: String, publish_at: String, state: String,
    started: bool, due: bool, within_retry: bool, retry_seconds:i64,
}
impl Intent {
    fn from_row(r: &Row) -> Self { Self {
        session:r.get("session_id"),owner:r.get("owner_id"),author:r.get("author"),service:r.get("service"),origin:r.get("origin"),admission:r.get("admission_version"),hash:r.get("payload_hash"),payload:r.get("payload"),version:r.get("session_version"),signer:r.get("signer"),group:r.get("signing_group"),parent_source:r.get("parent_source"),parent_fingerprint:r.get("parent_fingerprint"),parent_manifest:r.get("parent_manifest"),parent_cover:r.get("parent_cover_id"),files_fingerprint:r.get("files_fingerprint"),chapter:r.get("chapter_id"),uuid:r.get("reserved_uuid"),publish_at:r.get("publish_at"),state:r.get("state"),started:r.get("started"),due:r.get("due"),within_retry:r.get("within_retry"),retry_seconds:r.get("retry_seconds"),
    } }
    fn receipt(&self) -> Value {
        json!({"id":self.uuid,"type":"chapter","attributes":{"volume":self.payload["draft"]["volume"],"chapter":self.payload["draft"]["chapter"],"title":self.payload["draft"]["title"],"translatedLanguage":self.payload["draft"]["language"],"pages":0,"publishAt":self.publish_at,"readableAt":null},"relationships":[],"nostr":{"publicationStatus":if self.state=="blocked"{"blocked"}else{"scheduled"},"scheduleState":self.state,"scheduleAccepted":true,"retryWindowSeconds":self.retry_seconds,"sourceAccepted":false,"readerAvailable":false,"canCancel":!self.started&&self.state!="cancelled"&&self.state!="complete"}})
    }
}
async fn load_intent(tx: &Transaction<'_>, id: &str, locked: bool) -> PublicApiResult<Option<Intent>> {
    let sql = if locked { format!("{INTENT_SELECT} FOR UPDATE OF i") } else { INTENT_SELECT.into() };
    Ok(tx.query_opt(&sql, &[&id]).await?.as_ref().map(Intent::from_row))
}
async fn files_snapshot(tx: &Transaction<'_>, id: &str) -> PublicApiResult<(String,i64)> {
    // Count every retained reference conservatively, including derivatives. The
    // immutable schedule retains these rows until complete or safe cancellation;
    // prepared command pins cannot make an ambiguous intent disappear from quota.
    let row=tx.query_one("WITH refs AS (SELECT f.id AS file,'original'::text AS variant,o.id,o.digest,o.mime,o.bytes,o.width,o.height FROM platform.upload_files f JOIN platform.media_objects o ON o.id=f.object_id WHERE f.session_id=$1 UNION ALL SELECT f.id,d.name,o.id,o.digest,o.mime,o.bytes,o.width,o.height FROM platform.upload_files f JOIN platform.upload_file_derivatives d ON d.file_id=f.id JOIN platform.media_objects o ON o.id=d.object_id WHERE f.session_id=$1) SELECT COALESCE(jsonb_agg(to_jsonb(refs) ORDER BY file,variant,id),'[]'::jsonb),COALESCE(sum(bytes),0)::bigint FROM refs",&[&id]).await?;
    let value:Value=row.get(0); Ok((digest(value.to_string().as_bytes()),row.get(1)))
}
async fn lock_target(tx: &Transaction<'_>, session: &Row, actor: &impl CatalogMutationActor) -> PublicApiResult<(String,Option<String>,i32)> {
    let manga:String=session.get("manga_id");
    let edit=session.get::<_,Option<String>>("chapter_id");
    // Local edit never writes/upgrades the parent. NO KEY UPDATE still excludes
    // metadata/cover writers, but permits begin_edit's parent FK KEY SHARE after
    // it holds the chapter. FOR UPDATE here would create a parent/chapter cycle.
    let parent_sql=if edit.is_some(){"SELECT protocol_series_id,version FROM platform.catalog_manga WHERE id=$1 FOR NO KEY UPDATE"}else{"SELECT protocol_series_id,version FROM platform.catalog_manga WHERE id=$1 FOR UPDATE"};
    let parent=tx.query_opt(parent_sql,&[&manga]).await?.ok_or_else(PublicApiError::not_found)?;
    if let Some(chapter)=edit {
        CatalogRepository::authorize_chapter_edit(tx,&chapter,actor,session.get::<_,Option<i32>>("edit_version").ok_or_else(PublicApiError::unavailable)?).await?;
    } else { CatalogRepository::ensure_upload_target(tx,&manga,actor,&session.get::<_,Vec<String>>("groups")).await?; }
    // The original path also validates groups. Lock in stable order here before
    // current binding/managed-source fences, never acquire an advisory owner lock.
    tx.query("SELECT id FROM platform.catalog_groups WHERE id=ANY($1) ORDER BY id FOR SHARE",&[&session.get::<_,Vec<String>>("groups")]).await?;
    Ok((manga,parent.get(0),parent.get(1)))
}
async fn source_identity(state:&AppState,tx:&Transaction<'_>,author:&str,groups:&[String]) -> PublicApiResult<(String,Option<String>)> {
    let config=state.config.protocol_publication.as_ref().filter(|c|c.native_node.is_some()).ok_or_else(PublicApiError::unavailable)?;
    let bindings=tx.query("SELECT publisher,group_id FROM platform.protocol_identity_bindings WHERE group_id=ANY($1) ORDER BY group_id,publisher FOR SHARE",&[&groups]).await?;
    if bindings.len()>1 {return Err(PublicApiError::bad_request("Select at most one configured protocol signing group"));}
    if let Some(binding)=bindings.first() {
        let signer:String=binding.get(0);let group:String=binding.get(1);
        let key=hex::decode(&signer).map_err(|_|PublicApiError::unavailable())?;
        let member=hex::decode(author).map_err(|_|PublicApiError::unavailable())?;
        let policy=tx.query_opt("SELECT enabled FROM managed_groups WHERE group_key=$1 FOR SHARE",&[&key]).await?.ok_or_else(||PublicApiError::forbidden("Original signing group is unavailable"))?;
        let role=tx.query_opt("SELECT role FROM group_members WHERE group_key=$1 AND member=$2",&[&key,&member]).await?.map(|r|r.get::<_,String>(0));
        if !policy.get::<_,bool>(0)||!matches!(role.as_deref(),Some("admin"|"publisher")) {return Err(PublicApiError::forbidden("Current original group publication authority is required"));}
        Ok((signer,Some(group)))
    } else {
        if author!=config.expected_publisher {return Err(PublicApiError::forbidden("This account is not bound to the configured original signer"));}
        let key=hex::decode(author).map_err(|_|PublicApiError::unavailable())?;
        let allowed=tx.query_opt("SELECT allowed FROM publisher_permissions WHERE publisher=$1 FOR SHARE",&[&key]).await?.is_some_and(|r|r.get::<_,bool>(0));
        if !allowed {return Err(PublicApiError::forbidden("Current original publisher admission is required"));}
        Ok((author.into(),None))
    }
}
async fn parent_cover(tx:&Transaction<'_>,manga:&str)->PublicApiResult<Option<String>> {
    Ok(tx.query_opt("SELECT c.id FROM platform.catalog_covers c JOIN platform.cover_media cm ON cm.cover_id=c.id AND cm.variant='original' JOIN platform.media_objects o ON o.id=cm.object_id WHERE c.manga_id=$1 AND c.deleted_at IS NULL ORDER BY c.is_main DESC,c.created_at DESC,c.id LIMIT 1",&[&manga]).await?.map(|r|r.get(0)))
}
fn parent_fingerprint(version:i32,manifest:&[u8])->String {digest(&[version.to_be_bytes().as_slice(),manifest].concat())}

/// Return None for omitted/past timestamps, retaining immediate commit semantics.
/// A future receipt is deliberately not a published chapter success response.
pub(super) async fn admit(
    state:&AppState,tx:&Transaction<'_>,actor:&BoundAccountWrite,cap:&AdmittedHttpCapability,
    id:&str,publish_at:Option<&str>,payload:&core::Commit,
)->PublicApiResult<Option<Value>> {
    let author=hex::encode(cap.principal());let service=hex::encode(cap.service());
    admit_inner(state,tx,actor,&author,&service,id,publish_at,payload).await
}
/// Native scheduling captures only already provisioned publication authority.
/// It does not manufacture an HTTP capability, bind an account, or grant source
/// rights. The durable worker subsequently rechecks exactly the same captures.
pub(super) async fn admit_native(
    state:&AppState,tx:&Transaction<'_>,actor:&super::native_contributions::NativeActor,
    id:&str,publish_at:Option<&str>,payload:&core::Commit,
)->PublicApiResult<Option<Value>> {
    let Some(time)=publish_at else{return Ok(None)};
    if !super::publish_at::valid_publish_at(time){return Err(PublicApiError::bad_request("publishAt must be a real UTC timestamp"));}
    let future:bool=tx.query_one("SELECT ($1::text::timestamp AT TIME ZONE 'UTC')>clock_timestamp()",&[&time]).await?.get(0);
    if !future{return Ok(None)}
    let config=state.config.protocol_publication.as_ref().filter(|c|c.native_node.is_some()).ok_or_else(||PublicApiError::forbidden("Scheduled publication is not configured on this site"))?;
    let bindings=tx.query("SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1 AND revoked_at IS NULL ORDER BY publisher FOR SHARE",&[&actor.user_id()]).await?;
    if bindings.len()!=1{return Err(PublicApiError::forbidden("Your account has no unique site-configured publication role; ask the site team"));}
    let author:String=bindings[0].get(0);let service=&config.storage_holder;
    let author_bytes=hex::decode(&author).map_err(|_|PublicApiError::unavailable())?;
    let version:i64=tx.query_opt("SELECT version FROM account_permissions WHERE publisher=$1 AND allowed FOR SHARE",&[&author_bytes]).await?.ok_or_else(||PublicApiError::forbidden("Your configured publication role is not currently enabled"))?.get(0);
    auth::scheduled_account(tx,service,&author,version,actor.user_id()).await?;
    admit_inner(state,tx,actor,&author,service,id,publish_at,payload).await
}
async fn admit_inner(
    state:&AppState,tx:&Transaction<'_>,actor:&impl CatalogMutationActor,author:&str,service:&str,
    id:&str,publish_at:Option<&str>,payload:&core::Commit,
)->PublicApiResult<Option<Value>> {
    let Some(publish_at)=publish_at else{return Ok(None)};
    if !super::publish_at::valid_publish_at(publish_at) {return Err(PublicApiError::bad_request("publishAt must be a real YYYY-MM-DDTHH:MM:SS UTC timestamp"));}
    let hash=payload_hash(payload)?;
    // Session lock always precedes the intent lock, including idempotent replay.
    let session=tx.query_opt("SELECT * FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 FOR UPDATE",&[&id,&actor.user_id()]).await?.ok_or_else(PublicApiError::not_found)?;
    if let Some(intent)=load_intent(tx,id,true).await? {
        if intent.hash!=hash||intent.publish_at!=publish_at {return Err(PublicApiError::conflict("Scheduled input is immutable; cancel before publication starts to create a new intent"));}
        if intent.state=="cancelled" {return Err(PublicApiError::conflict("This schedule was cancelled"));}
        if intent.state=="complete" {return Ok(None)}
        return Ok(Some(intent.receipt()));
    }
    let horizon=i64::from(state.config.scheduled_upload_horizon_seconds);
    let timing=tx.query_one("SELECT ($1::text::timestamp AT TIME ZONE 'UTC')>clock_timestamp(),($1::text::timestamp AT TIME ZONE 'UTC')<=clock_timestamp()+$2::bigint*interval '1 second'",&[&publish_at,&horizon]).await?;
    if !timing.get::<_,bool>(0) {return Ok(None)}
    if !timing.get::<_,bool>(1) {return Err(PublicApiError::bad_request("publishAt exceeds this instance's configured scheduling horizon"));}
    actor.require_permission("self.chapter.update")?;
    core::validate_draft_for(&payload.draft,LanguageContract::Public)?;
    core::open_session(tx,id,actor.user_id()).await?;core::require_mutable_session(tx,id).await?;
    let rows=core::file_rows(tx,id).await?;
    core::validate_order(&payload.page_order,&rows.iter().map(|r|r.get::<_,String>(0)).collect::<Vec<_>>())?;
    let (manga,parent_source,parent_version)=lock_target(tx,&session,actor).await?;
    let is_edit=session.get::<_,Option<String>>("chapter_id").is_some();
    if !is_edit && parent_source.is_none() && tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE command_id=$1)",&[&format!("manga:{manga}")]).await?.get::<_,bool>(0) {
        return Err(PublicApiError::conflict("Reconcile the existing original parent publication command before scheduling"));
    }
    let (signer,group)=if is_edit {
        (state.config.protocol_publication.as_ref().ok_or_else(PublicApiError::unavailable)?.expected_publisher.clone(),None)
    } else {source_identity(state,tx,&author,&session.get::<_,Vec<String>>("groups")).await?};
    let manifest=crate::protocol::publication::scheduled_parent_manifest(state,tx,&manga).await?;
    let parent_fp=parent_fingerprint(parent_version,&manifest);
    let cover=parent_cover(tx,&manga).await?;
    let (files_fp,mut bytes)=files_snapshot(tx,id).await?;
    if !is_edit {
        let mut unique=std::collections::BTreeMap::new();
        for file in &rows {
            if !matches!(file.get::<_,String>(2).as_str(),"jpg"|"png"|"webp") {return Err(PublicApiError::bad_request("Scheduled original format is unsupported"));}
            unique.insert(file.get::<_,String>(1),file.get::<_,i64>(3));
        }
        if unique.values().sum::<i64>()>256*1024*1024 {return Err(PublicApiError::bad_request("Configured original publication supports at most 256 MiB of unique original bytes"));}
        if parent_source.is_none() {
            let cover=manga_node::series_metadata::parse(&manifest).map_err(|_|PublicApiError::unavailable())?.cover;
            let cover_bytes=cover.map(|c|c.size).unwrap_or(0);
            bytes=bytes.checked_add(i64::try_from(cover_bytes).map_err(|_|PublicApiError::unavailable())?).ok_or_else(PublicApiError::unavailable)?;
        }
    }
    let budget=tx.query_one("SELECT count(*)::bigint,COALESCE(sum(retained_bytes),0)::bigint FROM platform.scheduled_uploads WHERE owner_id=$1 AND state NOT IN ('complete','cancelled')",&[&actor.user_id()]).await?;
    if budget.get::<_,i64>(0)>=i64::from(state.config.scheduled_upload_maximum)||budget.get::<_,i64>(1).checked_add(bytes).is_none_or(|n|n>state.config.scheduled_upload_bytes) {return Err(PublicApiError::conflict("This account's scheduling queue or retained originals-plus-derivatives budget is full; blocked and uncertain schedules remain counted"));}
    let chapter=session.get::<_,Option<String>>("chapter_id").unwrap_or_else(storage::short_id);
    if !is_edit && tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_chapters WHERE id=$1) OR EXISTS(SELECT 1 FROM platform.scheduled_uploads WHERE chapter_id=$1)",&[&chapter]).await?.get::<_,bool>(0) {return Err(PublicApiError::conflict("Reserved chapter identity collided; retry scheduling"));}
    let uuid:String=tx.query_one("SELECT COALESCE((SELECT public_id FROM platform.public_resource_ids WHERE kind='chapter' AND native_id=$1 AND preferred),platform.public_native_uuid(namespace,'chapter',$1))::text FROM platform.catalogue_namespace WHERE singleton",&[&chapter]).await?.get(0);
    let author_bytes=hex::decode(author).map_err(|_|PublicApiError::unavailable())?;
    let admission:i64=tx.query_one("SELECT version FROM account_permissions WHERE publisher=$1",&[&author_bytes]).await?.get(0);
    let version=session.get::<_,i32>("version").checked_add(1).ok_or_else(PublicApiError::unavailable)?;
    let value=serde_json::to_value(payload).map_err(|_|PublicApiError::bad_request("Invalid commit"))?;
    tx.execute("INSERT INTO platform.scheduled_uploads(session_id,owner_id,publish_at,due_at,retry_until,payload,payload_hash,session_version,author,service,origin,admission_version,signer,signing_group,parent_source,parent_fingerprint,parent_manifest,files_fingerprint,chapter_id,chapter_uuid,retained_bytes,parent_cover_id) VALUES($1,$2,$3,$3::text::timestamp AT TIME ZONE 'UTC',($3::text::timestamp AT TIME ZONE 'UTC')+$21::bigint*interval '1 second',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18::text::uuid,$19,$20)",&[&id,&actor.user_id(),&publish_at,&value,&hash,&version,&author,&service,&state.config.public_origin,&admission,&signer,&group,&parent_source,&parent_fp,&manifest,&files_fp,&chapter,&uuid,&bytes,&cover,&i64::from(state.config.scheduled_upload_retry_seconds)]).await?;
    tx.execute("UPDATE platform.upload_sessions SET state='scheduled',version=$2,updated_at=clock_timestamp(),resume_draft=$3,resume_page_order=$4 WHERE id=$1",&[&id,&version,&value["draft"],&payload.page_order]).await?;
    Ok(Some(load_intent(tx,id,false).await?.ok_or_else(PublicApiError::unavailable)?.receipt()))
}

async fn expected_chapter_manifest(tx:&Transaction<'_>,intent:&Intent,parent:&str)->PublicApiResult<Vec<u8>> {
    let payload:core::Commit=serde_json::from_value(intent.payload.clone()).map_err(|_|PublicApiError::unavailable())?;
    let files=core::file_rows(tx,&intent.session).await?;
    let mut pages=Vec::new();
    for id in &payload.page_order {
        let row=files.iter().find(|r|r.get::<_,String>(0)==*id).ok_or_else(||PublicApiError::conflict("Scheduled original set changed"))?;
        let extension:String=row.get(2);
        let mime=match extension.as_str(){"jpg"=>"image/jpeg","png"=>"image/png","webp"=>"image/webp",_=>return Err(PublicApiError::bad_request("Scheduled original format is unsupported"))};
        pages.push(manga_node::manifest::Page{hash:row.get(1),size:row.get::<_,i64>(3) as u64,mime:mime.into(),width:row.get::<_,i32>(5) as u32,height:row.get::<_,i32>(6) as u32});
    }
    manga_node::platform_bridge::encode_chapter(&manga_node::manifest::Manifest{series:parent.into(),language:payload.draft.language,chapter:payload.draft.chapter,volume:payload.draft.volume,title:payload.draft.title,pages}).map_err(|_|PublicApiError::bad_request("Scheduled chapter cannot be represented by the original protocol"))
}
fn source_id(text:&str)->PublicApiResult<[u8;32]> {hex::decode(text).ok().and_then(|v|v.try_into().ok()).ok_or_else(PublicApiError::unavailable)}
/// Both the immutable command and a currently healthy, verified original receipt
/// are required. A prepared row alone is never permission to replay uncertain IO.
async fn verified_command(
    state:&AppState,tx:&Transaction<'_>,intent:&Intent,command:&str,kind:&str,
    catalog:&str,manifest:&[u8],parent:Option<&str>,
)->PublicApiResult<Option<String>> {
    let Some(row)=tx.query_opt("SELECT user_id,group_id,resource_kind,catalog_id,event_id,source_manifest,native_commit_hash,native_commit_payload,scheduled_session_id,created_at FROM platform.protocol_publication_commands WHERE command_id=$1",&[&command]).await? else{return Ok(None)};
    if row.get::<_,String>(0)!=intent.owner||row.get::<_,Option<String>>(1)!=intent.group||row.get::<_,String>(2)!=kind||row.get::<_,String>(3)!=catalog||row.get::<_,Option<Vec<u8>>>(5).as_deref()!=Some(manifest)||row.get::<_,Option<String>>(8).as_deref()!=Some(intent.session.as_str()) {
        return Err(PublicApiError::conflict("Scheduled original command provenance or immutable input does not match"));
    }
    if kind=="chapter" && (row.get::<_,Option<String>>(6).as_deref()!=Some(intent.hash.as_str())||row.get::<_,Option<Value>>(7).as_ref()!=Some(&intent.payload)) {return Err(PublicApiError::conflict("Scheduled original chapter payload changed"));}
    let event_id=row.get::<_,Option<String>>(4).ok_or_else(||PublicApiError::conflict("Original publication outcome is uncertain; no automatic replay or purge"))?;
    let id=source_id(&event_id)?;
    let event=if let Some(entry)=state.protocol.entry(id).await? {Some(entry.event)} else if kind=="manga" {
        manga_node::platform_bridge::pending_series_receipt(tx,id).await.map_err(|_|PublicApiError::unavailable())?
    } else {
        manga_node::platform_bridge::pending_chapter_receipt(tx,id,source_id(parent.ok_or_else(PublicApiError::unavailable)?)?).await.map_err(|_|PublicApiError::unavailable())?
    }.ok_or_else(||PublicApiError::conflict("Original publication outcome is uncertain; no automatic replay or purge"))?;
    event.verify().map_err(|_|PublicApiError::conflict("Original publication receipt failed verification"))?;
    let content:Value=serde_json::from_str(&event.content).map_err(|_|PublicApiError::unavailable())?;
    if event.id!=event_id||event.pubkey!=intent.signer||event.kind!=if kind=="manga"{1002}else{1001}||event.created_at!=row.get::<_,i64>(9) as u64||content["manifest"].as_str()!=Some(digest(manifest).as_str()) {return Err(PublicApiError::conflict("Original publication receipt does not match the scheduled immutable command"));}
    Ok(Some(event_id))
}
async fn gate(state:&AppState,tx:&Transaction<'_>,id:&str,token:&str,execution:bool)->PublicApiResult<(Intent,ScheduledUploadActor)> {
    let initial=load_intent(tx,id,false).await?.ok_or_else(PublicApiError::not_found)?;
    // This first nonlocking read grants no authority. All immutable fields and
    // claim ownership are reread under the established full lock order below.
    let account=auth::scheduled_account(tx,&initial.service,&initial.author,initial.admission,&initial.owner).await?;
    account.require_permission("self.chapter.update")?;
    let session=tx.query_opt("SELECT * FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 FOR UPDATE",&[&id,&account.user_id()]).await?.ok_or_else(PublicApiError::not_found)?;
    let intent=load_intent(tx,id,true).await?.ok_or_else(PublicApiError::not_found)?;
    let claim:bool=tx.query_one("SELECT lease_token=$2 AND lease_until>clock_timestamp() FROM platform.scheduled_uploads WHERE session_id=$1",&[&id,&token]).await?.get::<_,Option<bool>>(0).unwrap_or(false);
    if !claim||!intent.due||!matches!(intent.state.as_str(),"scheduled"|"starting")||session.get::<_,String>("state")!="scheduled"||session.get::<_,i32>("version")!=intent.version||(execution&&!intent.started) {return Err(PublicApiError::conflict("Scheduled publication claim is no longer current"));}
    if intent.origin!=state.config.public_origin||state.config.protocol_publication.as_ref().is_none_or(|c|c.storage_holder!=intent.service||c.native_node.is_none()) {return Err(PublicApiError::forbidden("Scheduled publication service configuration changed"));}
    let actor=ScheduledUploadActor{account,session:id.into(),token:token.into(),hash:intent.hash.clone(),chapter:intent.chapter.clone(),signer:intent.signer.clone(),group:intent.group.clone(),parent_manifest:intent.parent_manifest.clone()};
    let payload:core::Commit=serde_json::from_value(intent.payload.clone()).map_err(|_|PublicApiError::unavailable())?;
    if payload_hash(&payload)?!=intent.hash {return Err(PublicApiError::conflict("Scheduled payload digest changed"));}
    core::validate_draft_for(&payload.draft,LanguageContract::Public)?;
    let (manga,parent_source,parent_version)=lock_target(tx,&session,&actor).await?;
    let is_edit=session.get::<_,Option<String>>("chapter_id").is_some();
    let (signer,group)=if is_edit {
        (state.config.protocol_publication.as_ref().ok_or_else(PublicApiError::unavailable)?.expected_publisher.clone(),None)
    } else {source_identity(state,tx,&intent.author,&session.get::<_,Vec<String>>("groups")).await?};
    if signer!=intent.signer||group!=intent.group {return Err(PublicApiError::forbidden("Scheduled original signing identity changed"));}
    if files_snapshot(tx,id).await?.0!=intent.files_fingerprint {return Err(PublicApiError::conflict("Scheduled immutable originals or derivatives changed"));}
    let current_manifest=crate::protocol::publication::scheduled_parent_manifest(state,tx,&manga).await?;
    if parent_cover(tx,&manga).await?!=intent.parent_cover||parent_fingerprint(parent_version,&current_manifest)!=intent.parent_fingerprint||intent.parent_manifest.as_deref()!=Some(current_manifest.as_slice()) {return Err(PublicApiError::conflict("Scheduled parent version or immutable metadata changed"));}
    if is_edit {
        if !intent.within_retry||parent_source!=intent.parent_source {return Err(PublicApiError::conflict("Scheduled edit deadline or parent identity changed"));}
        return Ok((intent,actor));
    }
    let mut source=intent.parent_source.clone();
    let mut accepted_chapter=None;
    if intent.started {
        if source.is_none() {
            source=verified_command(state,tx,&intent,&format!("manga:{manga}"),"manga",&manga,intent.parent_manifest.as_deref().ok_or_else(PublicApiError::unavailable)?,None).await?;
        }
        if parent_source!=intent.parent_source && (source.is_none()||parent_source!=source) {return Err(PublicApiError::conflict("Unrelated original parent association changed"));}
        // Inspect the upload command even when the parent is already accepted.
        let has_chapter:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE command_id=$1)",&[&format!("upload:{id}")]).await?.get(0);
        if has_chapter {
            let parent=source.as_deref().ok_or_else(||PublicApiError::conflict("Original publication outcome is uncertain; parent receipt is unavailable"))?;
            let manifest=expected_chapter_manifest(tx,&intent,parent).await?;
            accepted_chapter=verified_command(state,tx,&intent,&format!("upload:{id}"),"chapter",&intent.chapter,&manifest,Some(parent)).await?;
        }
        if !intent.within_retry && (intent.parent_source.is_some() || source.is_none()) && !has_chapter {return Err(PublicApiError::conflict("Scheduled publication deadline passed before original acceptance"));}
    } else {
        if !intent.within_retry {return Err(PublicApiError::conflict("Scheduled publication deadline passed"));}
        if parent_source!=intent.parent_source {return Err(PublicApiError::conflict("Scheduled original parent changed"));}
        // No command may be silently adopted before this schedule's start.
        let existing:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE command_id=$1 OR ($2::bool AND command_id=$3))",&[&format!("upload:{id}"),&intent.parent_source.is_none(),&format!("manga:{manga}")]).await?.get(0);
        if existing {return Err(PublicApiError::conflict("Another original publication command now owns this input"));}
    }
    if let Some(existing)=tx.query_opt("SELECT protocol_event_id FROM platform.catalog_chapters WHERE id=$1",&[&intent.chapter]).await? {
        if accepted_chapter.is_none()||existing.get::<_,Option<String>>(0)!=accepted_chapter {return Err(PublicApiError::conflict("Reserved scheduled chapter identity is already occupied"));}
    }
    Ok((intent,actor))
}

async fn claim(state:&AppState)->PublicApiResult<Option<(String,String)>> {
    let token=format!("{}{}",storage::short_id(),storage::short_id());
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let row=tx.query_opt("WITH candidate AS (SELECT session_id FROM platform.scheduled_uploads WHERE due_at<=clock_timestamp() AND next_attempt_at<=clock_timestamp() AND state IN ('scheduled','starting') AND (lease_until IS NULL OR lease_until<=clock_timestamp()) ORDER BY due_at,session_id LIMIT 1 FOR UPDATE SKIP LOCKED) UPDATE platform.scheduled_uploads i SET lease_token=$1,lease_until=clock_timestamp()+interval '5 minutes',generation=generation+1,updated_at=clock_timestamp() FROM candidate c WHERE i.session_id=c.session_id RETURNING i.session_id",&[&token]).await?;
    tx.commit().await?;Ok(row.map(|r|(r.get(0),token)))
}
async fn process_claim(state:&AppState,id:&str,token:&str)->PublicApiResult<()> {
    // Durable one-way start boundary commits before any alias/source command IO.
    {
        let mut client=state.db.client().await?;let tx=client.transaction().await?;
        let (_intent,_actor)=gate(state,&tx,id,token,false).await?;
        tx.execute("UPDATE platform.scheduled_uploads SET state='starting',started_at=COALESCE(started_at,clock_timestamp()),updated_at=clock_timestamp() WHERE session_id=$1 AND lease_token=$2",&[&id,&token]).await?;
        tx.commit().await?;
    }
    // Due-only alias registration is independently committed. Never retain its
    // advisory/row lock while original command preparation independently uses it.
    {
        let mut client=state.db.client().await?;let tx=client.transaction().await?;
        let row=tx.query_opt("SELECT chapter_id,chapter_uuid::text FROM platform.scheduled_uploads WHERE session_id=$1 AND state='starting' AND lease_token=$2 AND due_at<=clock_timestamp()",&[&id,&token]).await?.ok_or_else(||PublicApiError::conflict("Scheduled claim changed"))?;
        let chapter:String=row.get(0);let uuid:String=row.get(1);
        let actual:String=tx.query_one("SELECT platform.public_register_id('chapter',$1,$2::text::uuid,'native')::text",&[&chapter,&uuid]).await?.get(0);
        if actual!=uuid {return Err(PublicApiError::conflict("Reserved scheduled chapter identity collided"));}
        tx.commit().await?;
    }
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let (intent,actor)=gate(state,&tx,id,token,true).await?;
    let payload:core::Commit=serde_json::from_value(intent.payload).map_err(|_|PublicApiError::unavailable())?;
    core::commit_scheduled_in(state,&tx,&actor,id,&payload).await?;
    let changed=tx.execute("UPDATE platform.scheduled_uploads SET state='complete',lease_token=NULL,lease_until=NULL,failure_code=NULL,updated_at=clock_timestamp() WHERE session_id=$1 AND lease_token=$2 AND state='starting'",&[&id,&token]).await?;
    if changed!=1 {return Err(PublicApiError::conflict("Scheduled completion lost its claim"));}
    tx.commit().await?;core::cleanup(state).await;Ok(())
}
async fn block_claim(state:&AppState,id:&str,token:&str,error:&PublicApiError)->PublicApiResult<()> {
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    // No authority locks are acquired after these rows. This is only private
    // failure bookkeeping, not publication, cancellation or quota release.
    tx.query_opt("SELECT id FROM platform.upload_sessions WHERE id=$1 FOR UPDATE",&[&id]).await?;
    let Some(intent)=tx.query_opt("SELECT i.started_at IS NOT NULL,i.retry_attempts,i.retry_until>clock_timestamp(),s.manga_id FROM platform.scheduled_uploads i JOIN platform.upload_sessions s ON s.id=i.session_id WHERE i.session_id=$1 AND i.lease_token=$2 AND i.state IN ('scheduled','starting') FOR UPDATE OF i",&[&id,&token]).await? else {tx.commit().await?;return Ok(())};
    let attempts:i32=intent.get(1);
    let manga:String=intent.get(3);
    // Command insertion commits before original signing/storage. Only a complete
    // negative lookup of BOTH relevant commands proves that no source attempt has
    // begun. Never classify a prepared/accepted/unknown external outcome as retryable.
    let commands:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE command_id=$1 OR command_id=$2 OR scheduled_session_id=$3)",&[&format!("upload:{id}"),&format!("manga:{manga}"),&id]).await?.get(0);
    let transient=error.status==axum::http::StatusCode::SERVICE_UNAVAILABLE&&!commands&&attempts<5&&intent.get::<_,bool>(2);
    if transient {
        let delay=2_i64.pow(attempts as u32).min(60);
        tx.execute("UPDATE platform.scheduled_uploads SET state=CASE WHEN started_at IS NULL THEN 'scheduled' ELSE 'starting' END,retry_attempts=retry_attempts+1,next_attempt_at=clock_timestamp()+$3::bigint*interval '1 second',failure_code='transient_retry',lease_token=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE session_id=$1 AND lease_token=$2",&[&id,&token,&delay]).await?;
    } else {
        let code=if commands&&error.status==axum::http::StatusCode::SERVICE_UNAVAILABLE||error.detail.contains("uncertain"){"publication_outcome_uncertain"}else{"authority_or_input_unavailable"};
        tx.execute("UPDATE platform.scheduled_uploads SET state='blocked',failure_code=$3,lease_token=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE session_id=$1 AND lease_token=$2",&[&id,&token,&code]).await?;
    }
    tx.commit().await?;Ok(())
}
/// One bounded maintenance turn. Database clock and transactional claims, not a
/// process timer or persisted HTTP authorization, decide whether execution is due.
pub async fn run_due(state:&AppState)->PublicApiResult<bool> {
    let Some((id,token))=claim(state).await? else{return Ok(false)};
    if let Err(error)=process_claim(state,&id,&token).await {block_claim(state,&id,&token,&error).await?;return Err(error)}
    Ok(true)
}

pub(super) async fn cancel_in(tx:&Transaction<'_>,actor:&impl CatalogMutationActor,id:&str)->PublicApiResult<bool> {
    actor.require_permission("self.chapter.update")?;
    let session=tx.query_opt("SELECT state FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 FOR UPDATE",&[&id,&actor.user_id()]).await?.ok_or_else(PublicApiError::not_found)?;
    let Some(intent)=load_intent(tx,id,true).await? else{return Ok(false)};
    if intent.state=="cancelled" {return Ok(true)}
    let command:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE upload_session_id=$1 OR scheduled_session_id=$1)",&[&id]).await?.get(0);
    if intent.started||command||session.get::<_,String>(0)!="scheduled" {return Err(PublicApiError::conflict("Publication has started or its outcome is uncertain; cancellation cannot release these originals"));}
    // A pre-start claim confers no irrevocable ownership. Clearing its token under
    // this same session->intent order makes the worker's gate fail atomically.
    tx.execute("UPDATE platform.scheduled_uploads SET state='cancelled',lease_token=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE session_id=$1",&[&id]).await?;
    tx.execute("UPDATE platform.upload_sessions SET state='deleted',version=version+1,updated_at=clock_timestamp() WHERE id=$1 AND state='scheduled'",&[&id]).await?;
    tx.execute("DELETE FROM platform.upload_files WHERE session_id=$1",&[&id]).await?;
    core::queue_unused(tx).await?;Ok(true)
}
/// The owner can distinguish verified source acceptance from reader availability.
/// Historical command states are not promoted to success without original proof.
pub(super) async fn owner_receipt(state:&AppState,tx:&Transaction<'_>,id:&str,owner:&str)->PublicApiResult<Option<Value>> {
    let owned=tx.query_opt("SELECT manga_id,chapter_id FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2",&[&id,&owner]).await?;
    let Some(session)=owned else{return Err(PublicApiError::not_found())};
    let Some(intent)=load_intent(tx,id,false).await? else{return Ok(None)};
    let mut receipt=intent.receipt();
    let reason:Option<String>=tx.query_one("SELECT failure_code FROM platform.scheduled_uploads WHERE session_id=$1",&[&id]).await?.get(0);
    receipt["nostr"]["blockedReason"]=json!(reason);
    receipt["nostr"]["canRetry"]=json!(intent.state=="blocked");
    if intent.state=="complete" {
        let native=core::owner_snapshot_in(state,tx,id,owner,false).await?;
        receipt["nostr"]["publicationStatus"]=json!("completed_unavailable");
        receipt["nostr"]["sourceAccepted"]=Value::Null;
        if native["receipt"].is_object() {
            if let Some(chapter)=super::uploads::optional_chapter_resource(tx,&native["receipt"]).await? {
                receipt["attributes"]=chapter["attributes"].clone();
                receipt["relationships"]=chapter["relationships"].clone();
                let readable=chapter["attributes"]["isUnavailable"]!=true;
                receipt["nostr"]["readerAvailable"]=json!(readable);
                receipt["nostr"]["sourceAccepted"]=json!(chapter["attributes"]["nostr"]["eventId"].is_string());
                receipt["nostr"]["publicationStatus"]=json!(if readable {"readable"}else{"source_accepted"});
            }
        }
        return Ok(Some(receipt));
    }
    if intent.started && session.get::<_,Option<String>>("chapter_id").is_none() {
        let has_commands:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE scheduled_session_id=$1)",&[&id]).await?.get(0);
        if has_commands {receipt["nostr"]["sourceAccepted"]=Value::Null;receipt["nostr"]["sourceState"]=json!("acceptance_unverified");}
        let manga:String=session.get(0);
        let parent=if let Some(source)=&intent.parent_source {Some(source.clone())} else {
            match intent.parent_manifest.as_deref() {
                Some(manifest)=>verified_command(state,tx,&intent,&format!("manga:{manga}"),"manga",&manga,manifest,None).await.ok().flatten(),None=>None,
            }
        };
        if let Some(parent)=parent {
            if intent.parent_source.is_none() {receipt["nostr"]["sourceState"]=json!("parent_source_accepted");}
            if let Ok(manifest)=expected_chapter_manifest(tx,&intent,&parent).await {
                if verified_command(state,tx,&intent,&format!("upload:{id}"),"chapter",&intent.chapter,&manifest,Some(&parent)).await.ok().flatten().is_some() {
                    receipt["nostr"]["sourceAccepted"]=json!(true);
                    receipt["nostr"]["sourceState"]=json!("chapter_source_accepted");
                    receipt["nostr"]["publicationStatus"]=json!("source_accepted");
                }
            }
        }
    }
    Ok(Some(receipt))
}

async fn captured_account(tx:&Transaction<'_>,actual:&str,expected:Option<&Value>)->PublicApiResult<()> {
    let expected=expected.and_then(Value::as_str).ok_or_else(||PublicApiError::bad_request("These schedule operations require the signed expectedAccountId query"))?;
    let expected=super::PublicUuid::parse(expected)?;
    if super::uploads::resolve(tx,super::ResourceKind::User,&expected).await?!=actual {
        return Err(PublicApiError::conflict("Upload account binding changed; reload the intended account"));
    }
    Ok(())
}
fn require_get(request:&axum::extract::Request)->PublicApiResult<()> {
    if request.method()!=axum::http::Method::GET {return Err(PublicApiError::new(axum::http::StatusCode::METHOD_NOT_ALLOWED,"Method Not Allowed","Use a signed GET request"));}
    Ok(())
}
pub fn router()->axum::Router<AppState> {
    axum::Router::new()
        .route("/nostr/uploads/schedules",axum::routing::get(list))
        .route("/nostr/uploads/{session}/schedule",axum::routing::get(get))
        .route("/nostr/uploads/{session}/schedule/retry",axum::routing::post(retry))
        .layer(axum::middleware::map_response(super::upload_extensions::private_response))
}
async fn list(
    axum::extract::State(state):axum::extract::State<AppState>,axum::extract::OriginalUri(uri):axum::extract::OriginalUri,
    headers:axum::http::HeaderMap,request:axum::extract::Request,
)->PublicApiResult<axum::Json<Value>> {
    require_get(&request)?;
    let (cap,expected)=super::uploads::authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=auth::bound_account_read(&tx,&cap).await?;
    captured_account(&tx,actor.user_id(),expected.as_ref()).await?;
    let rows=tx.query("SELECT session_id FROM platform.scheduled_uploads WHERE owner_id=$1 AND state NOT IN ('complete','cancelled') ORDER BY due_at,session_id LIMIT 1000",&[&actor.user_id()]).await?;
    let mut data=Vec::new();
    for row in rows {
        let id:String=row.get(0);
        let session=super::uploads::preferred(&tx,super::ResourceKind::UploadSession,&id).await?;
        let receipt=owner_receipt(&state,&tx,&id,actor.user_id()).await?.ok_or_else(PublicApiError::unavailable)?;
        data.push(json!({"uploadSessionId":session,"receipt":receipt}));
    }
    tx.commit().await?;Ok(axum::Json(json!({"result":"ok","data":data,"limits":{"maximumOutstanding":state.config.scheduled_upload_maximum,"maximumRetainedBytes":state.config.scheduled_upload_bytes,"maximumHorizonSeconds":state.config.scheduled_upload_horizon_seconds,"retryWindowSeconds":state.config.scheduled_upload_retry_seconds,"timezone":"UTC"}})))
}
async fn get(
    axum::extract::State(state):axum::extract::State<AppState>,axum::extract::Path(session):axum::extract::Path<String>,axum::extract::OriginalUri(uri):axum::extract::OriginalUri,
    headers:axum::http::HeaderMap,request:axum::extract::Request,
)->PublicApiResult<axum::Json<Value>> {
    require_get(&request)?;
    let session=super::PublicUuid::parse(&session)?;
    let (cap,expected)=super::uploads::authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=auth::bound_account_read(&tx,&cap).await?;
    captured_account(&tx,actor.user_id(),expected.as_ref()).await?;
    let id=super::uploads::resolve(&tx,super::ResourceKind::UploadSession,&session).await?;
    let receipt=owner_receipt(&state,&tx,&id,actor.user_id()).await?.ok_or_else(PublicApiError::not_found)?;
    tx.commit().await?;Ok(axum::Json(json!({"result":"ok","uploadSessionId":session,"receipt":receipt})))
}
async fn retry(
    axum::extract::State(state):axum::extract::State<AppState>,axum::extract::Path(session):axum::extract::Path<String>,axum::extract::OriginalUri(uri):axum::extract::OriginalUri,
    headers:axum::http::HeaderMap,request:axum::extract::Request,
)->PublicApiResult<(axum::http::StatusCode,axum::Json<Value>)> {
    let session=super::PublicUuid::parse(&session)?;
    let (cap,expected)=super::uploads::authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=auth::bound_account_write(&tx,&cap).await?;
    captured_account(&tx,actor.user_id(),expected.as_ref()).await?;
    actor.require_permission("self.chapter.update")?;
    let id=super::uploads::resolve(&tx,super::ResourceKind::UploadSession,&session).await?;
    tx.query_opt("SELECT id FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 AND state='scheduled' FOR UPDATE",&[&id,&actor.user_id()]).await?.ok_or_else(PublicApiError::not_found)?;
    let intent=load_intent(&tx,&id,true).await?.ok_or_else(PublicApiError::not_found)?;
    if intent.state!="blocked" {return Err(PublicApiError::conflict("Only a blocked immutable schedule can request rechecking"));}
    // Requeue is not permission to replay. The same fresh gate must still prove
    // prepared outcomes before any publication call. Unknown stays blocked.
    tx.execute("UPDATE platform.scheduled_uploads SET state=CASE WHEN started_at IS NULL THEN 'scheduled' ELSE 'starting' END,lease_token=NULL,lease_until=NULL,failure_code=NULL,retry_attempts=0,next_attempt_at=clock_timestamp(),updated_at=clock_timestamp() WHERE session_id=$1",&[&id]).await?;
    let receipt=owner_receipt(&state,&tx,&id,actor.user_id()).await?.ok_or_else(PublicApiError::unavailable)?;
    tx.commit().await?;Ok((axum::http::StatusCode::ACCEPTED,axum::Json(receipt)))
}

pub fn native_router()->axum::Router<AppState> {
 axum::Router::new().route("/account/uploads/schedules",axum::routing::get(native_list)).route("/account/uploads/{session}/schedule",axum::routing::get(native_get)).route("/account/uploads/{session}/schedule/retry",axum::routing::post(native_retry))
}
async fn native_list(
    axum::extract::State(state):axum::extract::State<AppState>,axum::extract::OriginalUri(uri):axum::extract::OriginalUri,
    headers:axum::http::HeaderMap,request:axum::extract::Request,
)->PublicApiResult<axum::Json<Value>> {
    require_get(&request)?;
    let (cap,expected)=super::native_uploads::authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=super::native_uploads::authority(&state,&tx,&cap,false).await?;
    captured_account(&tx,actor.user_id(),expected.as_ref()).await?;
    let rows=tx.query("SELECT session_id FROM platform.scheduled_uploads WHERE owner_id=$1 AND state NOT IN ('complete','cancelled') ORDER BY due_at,session_id LIMIT 1000",&[&actor.user_id()]).await?;
    let mut data=Vec::new();
    for row in rows {
        let id:String=row.get(0);
        let session=super::uploads::preferred(&tx,super::ResourceKind::UploadSession,&id).await?;
        let receipt=owner_receipt(&state,&tx,&id,actor.user_id()).await?.ok_or_else(PublicApiError::unavailable)?;
        data.push(json!({"uploadSessionId":session,"receipt":receipt}));
    }
    let account=super::uploads::preferred(&tx,super::ResourceKind::User,actor.user_id()).await?;tx.commit().await?;Ok(axum::Json(json!({"result":"ok","nostr":{"scope":"instance-account","authentication":"native-session","accountId":account},"data":data,"limits":{"maximumOutstanding":state.config.scheduled_upload_maximum,"maximumRetainedBytes":state.config.scheduled_upload_bytes,"maximumHorizonSeconds":state.config.scheduled_upload_horizon_seconds,"retryWindowSeconds":state.config.scheduled_upload_retry_seconds,"timezone":"UTC"}})))
}
async fn native_get(
    axum::extract::State(state):axum::extract::State<AppState>,axum::extract::Path(session):axum::extract::Path<String>,axum::extract::OriginalUri(uri):axum::extract::OriginalUri,
    headers:axum::http::HeaderMap,request:axum::extract::Request,
)->PublicApiResult<axum::Json<Value>> {
    require_get(&request)?;
    let session=super::PublicUuid::parse(&session)?;
    let (cap,expected)=super::native_uploads::authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=super::native_uploads::authority(&state,&tx,&cap,false).await?;
    captured_account(&tx,actor.user_id(),expected.as_ref()).await?;
    let id=super::uploads::resolve(&tx,super::ResourceKind::UploadSession,&session).await?;
    let receipt=owner_receipt(&state,&tx,&id,actor.user_id()).await?.ok_or_else(PublicApiError::not_found)?;
    let account=super::uploads::preferred(&tx,super::ResourceKind::User,actor.user_id()).await?;tx.commit().await?;Ok(axum::Json(json!({"result":"ok","uploadSessionId":session,"receipt":receipt,"nostr":{"scope":"instance-account","authentication":"native-session","accountId":account}})))
}
async fn native_retry(
    axum::extract::State(state):axum::extract::State<AppState>,axum::extract::Path(session):axum::extract::Path<String>,axum::extract::OriginalUri(uri):axum::extract::OriginalUri,
    headers:axum::http::HeaderMap,request:axum::extract::Request,
)->PublicApiResult<(axum::http::StatusCode,axum::Json<Value>)> {
    let session=super::PublicUuid::parse(&session)?;
    let (cap,expected)=super::native_uploads::authorize_account_query(&state,&uri,&headers,request).await?;
    let mut client=state.db.client().await?;let tx=client.transaction().await?;
    let actor=super::native_uploads::authority(&state,&tx,&cap,true).await?;
    captured_account(&tx,actor.user_id(),expected.as_ref()).await?;
    actor.require_permission("self.chapter.update")?;
    let id=super::uploads::resolve(&tx,super::ResourceKind::UploadSession,&session).await?;
    tx.query_opt("SELECT id FROM platform.upload_sessions WHERE id=$1 AND owner_id=$2 AND state='scheduled' FOR UPDATE",&[&id,&actor.user_id()]).await?.ok_or_else(PublicApiError::not_found)?;
    let intent=load_intent(&tx,&id,true).await?.ok_or_else(PublicApiError::not_found)?;
    if intent.state!="blocked" {return Err(PublicApiError::conflict("Only a blocked immutable schedule can request rechecking"));}
    // Requeue is not permission to replay. The same fresh gate must still prove
    // prepared outcomes before any publication call. Unknown stays blocked.
    tx.execute("UPDATE platform.scheduled_uploads SET state=CASE WHEN started_at IS NULL THEN 'scheduled' ELSE 'starting' END,lease_token=NULL,lease_until=NULL,failure_code=NULL,retry_attempts=0,next_attempt_at=clock_timestamp(),updated_at=clock_timestamp() WHERE session_id=$1",&[&id]).await?;
    let mut receipt=owner_receipt(&state,&tx,&id,actor.user_id()).await?.ok_or_else(PublicApiError::unavailable)?;
    super::native_account_profile_settings::provenance(&mut receipt,super::uploads::preferred(&tx,super::ResourceKind::User,actor.user_id()).await?.as_str());
    tx.commit().await?;Ok((axum::http::StatusCode::ACCEPTED,axum::Json(receipt)))
}
