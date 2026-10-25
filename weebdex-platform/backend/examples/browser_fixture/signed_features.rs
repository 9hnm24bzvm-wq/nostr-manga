//! Opt-in synthetic signed report/draft/chapter acceptance data. No browser
//! private key, native password, cookie, publisher grant or original write.
use super::{Request, bounded_json, id, loopback_origin, recorded_source, write_json};
use anyhow::{Context, Result, ensure};
use manga_node::{auth, db::Repository, manifest};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
use tokio_postgres::Transaction;

const SCENARIO: &str = "public-signed-features-v1";
const USER: &str = "SgFeatUsr1";
const FOREIGN_USER: &str = "SgFeatUsr2";
const DRAFT: &str = "SgFeatDft1";
const FOREIGN_DRAFT: &str = "SgFeatDft2";
const REASON: &str = "manga_signed_feature_fixture";
const REASON_TEXT: &str = "Synthetic signed feature reason";
const REPORT_BODY: &str = "Synthetic signed feature report details.";
const SAVED_TITLE: &str = "Synthetic signed feature saved chapter";
const REMOVAL_BODY: &str = "Authorized current owner requests instance title removal; no original deletion requested";

fn permissions() -> Value {
    json!(["report.create", "manga.create", "self.chapter.update"])
}
fn guard<'a>(root: &Path, request: &'a Request) -> Result<&'a str> {
    let marker = bounded_json(&root.join("fixture-owned.json"))?;
    ensure!(marker["signed_feature_controls"] == true && marker["synthetic_images"] == true,
        "separate signed feature synthetic fixture opt-in required");
    ensure!(request.ui_origin.is_none() && request.protocol_origin.is_none()
        && request.backend_binary.is_none() && request.seed_input.is_none()
        && request.allowed.is_none() && request.event_id.is_none()
        && request.recipient_origin.is_none() && request.scenario.is_none()
        && request.session_id.is_none(), "only a synthetic public key is accepted");
    let public = request.pubkey.as_deref().context("synthetic browser public key required")?;
    id(public)?;
    let setup = bounded_json(&root.join("protocol-setup.json"))?;
    ensure!(Some(public) != setup["operator_pubkey"].as_str()
        && Some(public) != setup["service_key"].as_str(), "fresh browser identity required");
    Ok(public)
}
async fn alias(tx: &Transaction<'_>, kind: &str, native: &str) -> Result<String> {
    Ok(tx.query_one("SELECT public_id::text FROM platform.public_resource_ids WHERE kind=$1 AND native_id=$2 AND preferred AND principal_type='native'", &[&kind,&native]).await?.get(0))
}
async fn authority(tx: &Transaction<'_>, public: &str) -> Result<()> {
    let valid: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.users u JOIN platform.protocol_identity_bindings b ON b.user_id=u.id WHERE u.id=$1 AND b.publisher=$2 AND b.group_id IS NULL AND u.verified AND NOT u.disabled AND u.profile->>'fixture_scenario'=$3 AND u.roles='[\"user\"]'::jsonb AND u.permissions=$4) AND NOT EXISTS(SELECT 1 FROM platform.sessions WHERE user_id=$1) AND NOT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE user_id=$1)", &[&USER,&public,&SCENARIO,&permissions()]).await?.get(0);
    ensure!(valid, "exact cookie-free non-moderator fixture authority required");
    Ok(())
}
async fn blob(client: &reqwest::Client, origin: &str, hash: &str, limit: usize) -> Result<Vec<u8>> {
    id(hash)?;
    let mut response = client.get(format!("{origin}/blobs/{hash}")).send().await?;
    ensure!(response.status() == reqwest::StatusCode::OK
        && response.content_length().is_none_or(|n| n <= limit as u64), "bounded visible original required");
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(bytes.len() + chunk.len() <= limit, "original observation byte budget");
        bytes.extend_from_slice(&chunk);
    }
    ensure!(auth::hash(&bytes) == hash, "served original fingerprint mismatch");
    Ok(bytes)
}
fn signed_manifest(content: &str, expected: &str) -> Result<()> {
    let pointer: Value = serde_json::from_str(content)?;
    ensure!(pointer.as_object().is_some_and(|object| object.len() == 2)
        && pointer["version"] == 1 && pointer["manifest"] == expected,
        "served manifest must match the signed source pointer");
    Ok(())
}
async fn original(repo: &Repository, root: &Path, event_id: &str) -> Result<Value> {
    // Never open a second storage owner. Read the persisted genuine event and
    // actual original-serving endpoint, rather than trusting cached input files.
    let (saved, _) = recorded_source(root, Some(event_id))?;
    ensure!(saved.kind == 1001, "recorded seed chapter required");
    let row = repo.pool.get().await?.query_one("SELECT raw_event,manifest,state,version FROM events WHERE id=$1", &[&id(event_id)?.as_slice()]).await?;
    let raw: Value = row.get(0);
    let event: auth::Event = serde_json::from_value(raw.clone())?;
    event.verify().map_err(anyhow::Error::msg)?;
    ensure!(raw == serde_json::to_value(saved)? && row.get::<_,String>(2) == "hosted",
        "recorded immutable hosted chapter event required");
    let hash = hex::encode(row.get::<_,Option<Vec<u8>>>(1).context("canonical manifest identity")?);
    signed_manifest(&event.content, &hash)?;
    let setup = bounded_json(&root.join("protocol-setup.json"))?;
    let origin = loopback_origin(setup["protocol_origin"].as_str())?;
    let client = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(1)).timeout(Duration::from_secs(2)).build()?;
    let bytes = blob(&client, &origin, &hash, 1_000_000).await?;
    let chapter = manifest::chapter(&bytes).map_err(anyhow::Error::msg)?;
    ensure!((1..=12).contains(&chapter.pages.len()), "bounded synthetic chapter required");
    let mut total = 0usize;
    let mut pages = Vec::new();
    for page in chapter.pages {
        ensure!(page.size <= 4 * 1024 * 1024, "synthetic page byte bound");
        let bytes = blob(&client, &origin, &page.hash, page.size as usize).await?;
        ensure!(bytes.len() as u64 == page.size, "exact original page bytes required");
        total += bytes.len();
        ensure!(total <= 16 * 1024 * 1024, "synthetic original total byte bound");
        pages.push(json!({"sha256":auth::hash(&bytes),"bytes":bytes.len()}));
    }
    Ok(json!({"source_event_id":event_id,"source_event_sha256":auth::hash(&serde_json::to_vec(&raw)?),
        "source_version":row.get::<_,i64>(3),"manifest_sha256":hash,"pages":pages,"fresh_original_reads":true}))
}
async fn bounded_original(repo: &Repository, root: &Path, event: &str) -> Result<Value> {
    tokio::time::timeout(Duration::from_secs(12), original(repo, root, event)).await?
}

pub(super) async fn account(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let public = guard(root, request)?;
    repo.verify_schema().await?;
    let path = root.join("signed-features.json");
    if path.exists() {
        let descriptor = bounded_json(&path)?;
        ensure!(descriptor["fixture_only"] == true && descriptor["scenario"] == SCENARIO
            && descriptor["pubkey"] == public, "fixture browser actor cannot be rebound");
        let mut db = repo.pool.get().await?;
        let tx = db.transaction().await?;
        authority(&tx, public).await?;
        tx.commit().await?;
        repo.account_permit(id(public)?).await?;
        return Ok(descriptor);
    }
    let setup = bounded_json(&root.join("protocol-setup.json"))?;
    let seed = bounded_json(&root.join("protocol-seed.json"))?;
    let series = seed["series"]["event_id"].as_str().context("canonical seed series")?;
    let event = seed["chapters"].as_array().context("seed chapters")?.iter()
        .find(|item| item["chapter"] == 1).context("canonical first chapter")?["event_id"]
        .as_str().context("canonical chapter identity")?;
    let original = bounded_original(repo, root, event).await?;
    let mut db = repo.pool.get().await?;
    let tx = db.transaction().await?;
    let conflict: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.users WHERE id=ANY($1)) OR EXISTS(SELECT 1 FROM platform.catalog_manga WHERE id=ANY($2)) OR EXISTS(SELECT 1 FROM platform.report_reasons WHERE id=$3) OR EXISTS(SELECT 1 FROM platform.protocol_identity_bindings WHERE publisher=$4 OR user_id=ANY($1))", &[&vec![USER,FOREIGN_USER],&vec![DRAFT,FOREIGN_DRAFT],&REASON,&public]).await?.get(0);
    ensure!(!conflict, "fresh fixed signed feature identities required");
    let manga: String = tx.query_one("SELECT id FROM platform.catalog_manga WHERE protocol_series_id=$1 AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)", &[&series]).await?.get(0);
    let chapter = tx.query_one("SELECT id,title,version FROM platform.catalog_chapters WHERE protocol_event_id=$1 AND manga_id=$2 AND deleted_at IS NULL AND NOT is_unavailable AND platform.protocol_source_is_visible(protocol_event_id) FOR UPDATE", &[&event,&manga]).await?;
    let chapter_id: String = chapter.get(0);
    for (user, username, email, grants) in [
        (USER,"SyntheticFeatureOwner","signed-feature-owner@example.invalid",permissions()),
        (FOREIGN_USER,"SyntheticFeatureOther","signed-feature-other@example.invalid",json!([])),
    ] {
        tx.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions,profile) VALUES($1,$2,$3,'not-a-login-password-hash',TRUE,'[\"user\"]',$4,jsonb_build_object('fixture_scenario',$5::text))", &[&user,&username,&email,&grants,&SCENARIO]).await?;
    }
    tx.execute("INSERT INTO platform.protocol_identity_bindings(publisher,user_id) VALUES($1,$2)", &[&public,&USER]).await?;
    for (draft, owner, name, title) in [
        (DRAFT,USER,"SyntheticFeatureOwner","Synthetic signed feature own draft"),
        (FOREIGN_DRAFT,FOREIGN_USER,"SyntheticFeatureOther","Synthetic signed feature foreign draft"),
    ] {
        tx.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,metadata) VALUES($1,$2,$3,$4,'en','ongoing','safe','draft',jsonb_build_object('fixture_scenario',$5::text,'description','Synthetic signed feature draft description.'))", &[&draft,&owner,&name,&title,&SCENARIO]).await?;
        tx.execute("INSERT INTO platform.catalog_manga_counters(manga_id) VALUES($1)", &[&draft]).await?;
    }
    tx.execute("INSERT INTO platform.report_reasons(id,category,reason,details_required,enabled) VALUES($1,'manga',$2,TRUE,TRUE)", &[&REASON,&REASON_TEXT]).await?;
    // Only local ownership changes. Do not alter canonical event, manifest,
    // originals, chapter metadata/version, or any upload/group permission.
    ensure!(tx.execute("UPDATE platform.catalog_chapters SET uploader_id=$2 WHERE id=$1 AND protocol_event_id=$3", &[&chapter_id,&USER,&event]).await? == 1, "exact fixture chapter ownership required");
    authority(&tx, public).await?;
    let namespace: String = tx.query_one("SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton", &[]).await?.get(0);
    let reason_version: i32 = tx.query_one("SELECT version FROM platform.report_reasons WHERE id=$1", &[&REASON]).await?.get(0);
    let descriptor = json!({"fixture_only":true,"scenario":SCENARIO,"pubkey":public,
        "user":{"id":alias(&tx,"user",USER).await?,"native_id":USER},
        "draft":{"id":alias(&tx,"manga",DRAFT).await?,"native_id":DRAFT},
        "foreign_draft":{"id":alias(&tx,"manga",FOREIGN_DRAFT).await?,"native_id":FOREIGN_DRAFT},
        "manga":{"id":alias(&tx,"manga",&manga).await?,"native_id":manga},
        "chapter":{"id":alias(&tx,"chapter",&chapter_id).await?,"native_id":chapter_id,"initial_title":chapter.get::<_,Option<String>>(1),"initial_version":chapter.get::<_,i32>(2)},
        "reason":{"id":alias(&tx,"report_reason",REASON).await?,"native_id":REASON,"version":reason_version,"text":REASON_TEXT},
        "catalog_namespace":namespace,"service_key":setup["service_key"],"original":original,
        "allowed":true,"roles":["user"],"native_permissions":permissions(),"no_native_sessions":true,
        "browser_private_key_received":false,"secret_received":false,"browser_operator_binding":false,
        "browser_publisher_admission_granted":false,"report_body":REPORT_BODY,"chapter_saved_title":SAVED_TITLE});
    tx.commit().await?;
    repo.set_account_admission(id(public)?, true).await?;
    repo.account_permit(id(public)?).await?;
    write_json(&path, &descriptor)?;
    Ok(descriptor)
}

pub(super) async fn snapshot(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let public = guard(root, request)?;
    let fixture = bounded_json(&root.join("signed-features.json"))?;
    ensure!(fixture["fixture_only"] == true && fixture["scenario"] == SCENARIO && fixture["pubkey"] == public,
        "exact recorded signed feature public actor required");
    repo.account_permit(id(public)?).await?;
    let mut db = repo.pool.get().await?;
    let tx = db.transaction().await?;
    authority(&tx, public).await?;
    let manga = fixture["manga"]["native_id"].as_str().context("recorded manga")?;
    let rows = tx.query("SELECT r.id,r.entity_type,r.entity_id,r.reason_id,r.body,r.status,r.target_version,r.deleted_at IS NOT NULL FROM platform.reports r WHERE r.submitter_id=$1 ORDER BY r.created_at,r.id LIMIT 4", &[&USER]).await?;
    ensure!(rows.len() <= 2, "bounded signed feature report observations required");
    let mut reports = Vec::new();
    let mut removals = Vec::new();
    for row in rows {
        let reason: String = row.get(3);
        let removal = reason == "manga_removal_request";
        let target = if removal {DRAFT} else {manga};
        ensure!((removal || reason == REASON) && row.get::<_,String>(1) == "manga"
            && row.get::<_,String>(2) == target && row.get::<_,String>(4) == if removal {REMOVAL_BODY} else {REPORT_BODY}
            && row.get::<_,String>(5) == "waiting" && !row.get::<_,bool>(7), "only exact synthetic report rows may leave fixture");
        let native: String = row.get(0);
        let value = json!({"id":alias(&tx,"report",&native).await?,"native_id":native,
            "submitter_id":fixture["user"]["id"],"entity_type":"manga","entity_id":alias(&tx,"manga",target).await?,
            "reason_id":alias(&tx,"report_reason",&reason).await?,"body":row.get::<_,String>(4),
            "status":"waiting","target_version":row.get::<_,Option<i32>>(6)});
        if removal {removals.push(value);} else {reports.push(value);}
    }
    ensure!(reports.len() <= 1 && removals.len() <= 1, "one report and one removal request bound");
    let mut draft_results = Vec::new();
    for (key, native, owner) in [("draft",DRAFT,USER),("foreign_draft",FOREIGN_DRAFT,FOREIGN_USER)] {
        let row = tx.query_one("SELECT state,version,deleted_at IS NOT NULL,owner_id,metadata->>'fixture_scenario' FROM platform.catalog_manga WHERE id=$1", &[&native]).await?;
        ensure!(row.get::<_,String>(3) == owner && row.get::<_,Option<String>>(4).as_deref() == Some(SCENARIO), "fixed draft owner/scenario required");
        let state: String = row.get(0);
        ensure!(["draft","submitted"].contains(&state.as_str()), "bounded draft state");
        draft_results.push(json!({"id":fixture[key]["id"],"native_id":native,"state":state,
            "version":row.get::<_,i32>(1),"deleted":row.get::<_,bool>(2),"owner_id":alias(&tx,"user",owner).await?}));
    }
    draft_results[0]["removal_request"] = json!({"count":removals.len(),"records":removals});
    let native = fixture["chapter"]["native_id"].as_str().context("recorded chapter")?;
    let chapter = tx.query_one("SELECT title,version,uploader_id,protocol_event_id,deleted_at IS NOT NULL,is_unavailable FROM platform.catalog_chapters WHERE id=$1 AND manga_id=$2", &[&native,&manga]).await?;
    let title: Option<String> = chapter.get(0);
    ensure!(serde_json::to_value(&title)? == fixture["chapter"]["initial_title"] || title.as_deref() == Some(SAVED_TITLE), "only synthetic initial/saved chapter title may leave fixture");
    ensure!(chapter.get::<_,String>(2) == USER, "fixed chapter owner required");
    let event: String = chapter.get::<_,Option<String>>(3).context("canonical chapter source")?;
    tx.commit().await?;
    drop(db);
    let mut current = bounded_original(repo, root, &event).await?;
    let unchanged = current == fixture["original"];
    current["unchanged"] = json!(unchanged);
    Ok(json!({"fixture_only":true,"scenario":SCENARIO,"pubkey":public,
        "report":{"count":reports.len(),"records":reports},"total_report_count":reports.len()+removals.len(),
        "draft":draft_results[0],"foreign_draft":draft_results[1],
        "chapter":{"id":fixture["chapter"]["id"],"native_id":native,"title":title,"version":chapter.get::<_,i32>(1),
            "uploader_id":fixture["user"]["id"],"deleted":chapter.get::<_,bool>(4),"is_unavailable":chapter.get::<_,bool>(5)},
        "original":current,"browser_private_key_received":false,"secret_received":false}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(root: &Path, public: &str) -> Request {
        serde_json::from_value(json!({"root":root,"pubkey":public})).unwrap()
    }
    #[test]
    fn public_key_guard_requires_separate_synthetic_opt_in_and_closed_request() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let public = "ab".repeat(32);
        write_json(&root.join("protocol-setup.json"), &json!({"operator_pubkey":"cd".repeat(32),"service_key":"ef".repeat(32)})).unwrap();
        for marker in [json!({}),json!({"signed_feature_controls":true}),json!({"signed_feature_controls":false,"synthetic_images":true}),json!({"signed_feature_controls":true,"synthetic_images":false})] {
            std::fs::write(root.join("fixture-owned.json"),serde_json::to_vec(&marker).unwrap()).unwrap();
            assert!(guard(root,&request(root,&public)).is_err());
        }
        std::fs::write(root.join("fixture-owned.json"),serde_json::to_vec(&json!({"signed_feature_controls":true,"synthetic_images":true})).unwrap()).unwrap();
        assert_eq!(guard(root,&request(root,&public)).unwrap(),public);
        for key in ["cd".repeat(32),"ef".repeat(32),"ab".repeat(31),"not-a-public-key".into()] {
            assert!(guard(root,&request(root,&key)).is_err());
        }
        for extra in [json!({"allowed":true}),json!({"session_id":"anything"}),json!({"scenario":SCENARIO}),json!({"event_id":"ab".repeat(32)}),json!({"ui_origin":"http://127.0.0.1:9999"})] {
            let mut value = json!({"root":root,"pubkey":public});
            value.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            let req: Request = serde_json::from_value(value).unwrap();
            assert!(guard(root,&req).is_err());
        }
        for field in ["private_key","password","cookie","permissions","user_id","chapter_id"] {
            let mut value=json!({"root":root,"pubkey":public});
            value[field]=json!("never-accepted");
            assert!(serde_json::from_value::<Request>(value).is_err());
        }
    }
    #[test]
    fn original_manifest_is_bound_to_the_signed_pointer() {
        let hash = "ab".repeat(32);
        assert!(signed_manifest(&json!({"version":1,"manifest":hash}).to_string(), &hash).is_ok());
        for value in [json!({"version":2,"manifest":hash}), json!({"version":1,"manifest":"cd".repeat(32)}),
            json!({"version":1,"manifest":hash,"extra":true}), json!({"version":1})] {
            assert!(signed_manifest(&value.to_string(), &hash).is_err());
        }
    }
    #[test]
    fn native_permissions_are_exact_and_do_not_grant_upload_or_moderation() {
        assert_eq!(permissions(),json!(["report.create","manga.create","self.chapter.update"]));
        assert!(!permissions().as_array().unwrap().iter().any(|p|p=="chapter.create"||p=="report.update"||p=="manga.delete"));
    }
}
