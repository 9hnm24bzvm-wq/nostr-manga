//! The sole writer of platform catalogue metadata. Protocol source identities are immutable.
use super::validation as v;
use crate::{
    db::PgStore,
    domains::catalog::entity_mutations::CatalogMutationActor,
    error::{ApiError, ApiResult},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_postgres::Transaction;

#[derive(Clone)]
pub struct CatalogRepository {
    pub(super) db: PgStore,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogPage {
    pub name: String,
    pub dimensions: [i32; 2],
}
#[derive(Clone, Debug)]
pub struct ChapterPublication {
    pub id: String,
    pub manga_id: String,
    pub uploader_id: String,
    pub uploader_name: String,
    pub title: Option<String>,
    pub volume: Option<String>,
    pub chapter: Option<String>,
    pub language: String,
    pub node: String,
    pub pages: Vec<CatalogPage>,
    pub optimized_pages: Vec<CatalogPage>,
    pub groups: Vec<String>,
    pub source_id: Option<String>,
    pub protocol_event_id: Option<String>,
}
#[derive(Clone, Debug)]
pub struct CoverPublication {
    pub id: String,
    pub manga_id: String,
    pub uploader_id: String,
    pub uploader_name: String,
    pub ext: String,
    pub main: bool,
    pub dimensions: [i32; 2],
    pub volume: String,
    pub language: String,
    pub description: Option<String>,
}
impl CatalogRepository {
    pub fn new(db: PgStore) -> Self {
        Self { db }
    }
    pub async fn get_manga(&self, id: &str) -> ApiResult<Value> {
        v::id(id)?;
        self.db.client().await?.query_opt("SELECT platform.catalog_manga_json(id) FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&id]).await?.map(|r|r.get(0)).ok_or_else(||ApiError::not_found("Manga not found"))
    }
    pub async fn get_chapter(&self, id: &str) -> ApiResult<Value> {
        v::id(id)?;
        self.db.client().await?.query_opt("SELECT platform.catalog_chapter_json(c.id) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) AND platform.protocol_source_is_visible(c.protocol_event_id)",&[&id]).await?.map(|r|r.get(0)).ok_or_else(||ApiError::not_found("Chapter not found"))
    }
    pub async fn get_cover(&self, id: &str) -> ApiResult<Value> {
        v::id(id)?;
        self.db.client().await?.query_opt("SELECT platform.catalog_cover_json(c.id) FROM platform.catalog_covers c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) AND platform.protocol_source_is_visible(c.protocol_event_id)",&[&id]).await?.map(|r|r.get(0)).ok_or_else(||ApiError::not_found("Cover not found"))
    }
    pub async fn ensure_upload_target(
        tx: &Transaction<'_>,
        manga_id: &str,
        actor: &impl CatalogMutationActor,
        groups: &[String],
    ) -> ApiResult<()> {
        actor.require_permission("chapter.create")?;
        if tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR SHARE",&[&manga_id]).await?.is_none(){return Err(ApiError::not_found("Published manga not found"));}
        validate_groups(tx, groups).await?;
        // Group attribution is permitted for chapter creators, as the source upload UI allows all groups.
        Ok(())
    }
    pub async fn authorize_chapter_edit(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
        version: i32,
    ) -> ApiResult<Value> {
        let row=tx.query_opt("SELECT uploader_id,version,protocol_event_id FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=manga_id AND parent.deleted_at IS NULL AND parent.state='published' AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id)) FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Chapter not found"))?;
        chapter_authority(tx, id, actor, &row.get::<_, String>(0), "update", false).await?;
        if row.get::<_, i32>(1) != version {
            return Err(ApiError::conflict("Chapter changed; reload before editing"));
        }
        if row.get::<_, Option<String>>(2).is_some() {
            return Err(ApiError::conflict(
                "Protocol source pages cannot be replaced",
            ));
        }
        Ok(tx
            .query_one("SELECT platform.catalog_chapter_json($1)", &[&id])
            .await?
            .get(0))
    }
    pub async fn replace_chapter_pages(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
        expected_version: i32,
        patch: &Value,
        node: &str,
        pages: &[CatalogPage],
    ) -> ApiResult<Value> {
        Self::authorize_chapter_edit(tx, id, actor, expected_version).await?;
        if pages.is_empty()
            || pages.len() > 500
            || pages.iter().any(|p| {
                p.name.is_empty()
                    || p.name.len() > 200
                    || p.name.contains(['/', '\\'])
                    || p.dimensions.iter().any(|v| *v <= 0 || *v > 100_000)
            })
        {
            return Err(ApiError::bad_request("Invalid chapter pages"));
        }
        let mut changes = patch.clone();
        v::object(&changes)?;
        changes
            .as_object_mut()
            .unwrap()
            .insert("version".to_owned(), json!(expected_version));
        Self::update_chapter(tx, id, actor, &changes).await?;
        let metadata = json!({"node":node,"data":pages});
        tx.execute("UPDATE platform.catalog_chapters SET metadata=(metadata - 'data_optimized') || $2 WHERE id=$1",&[&id,&metadata]).await?;
        Ok(tx
            .query_one("SELECT platform.catalog_chapter_json($1)", &[&id])
            .await?
            .get(0))
    }
    pub async fn publish_chapter(
        tx: &Transaction<'_>,
        record: &ChapterPublication,
    ) -> ApiResult<Value> {
        Self::insert_chapter(tx, record, false).await
    }
    /// Private integration capability, never exposed by an unsigned upload.
    /// Canonical public visibility is still required by every read projection.
    pub(super) async fn publish_chapter_after_verified_acceptance(
        tx: &Transaction<'_>,
        record: &ChapterPublication,
        receipt: &crate::protocol::publication::PublicationReceipt,
    ) -> ApiResult<Value> {
        receipt
            .event
            .verify()
            .map_err(|_| ApiError::bad_request("Invalid original signature"))?;
        if receipt.event.kind != 1001
            || record.protocol_event_id.as_deref() != Some(receipt.event.id.as_str())
        {
            return Err(ApiError::bad_request(
                "Verified chapter publication identity mismatch",
            ));
        }
        let mut local = record.clone();
        local.protocol_event_id = None;
        Self::insert_chapter(tx, &local, true).await
    }
    async fn insert_chapter(
        tx: &Transaction<'_>,
        record: &ChapterPublication,
        verified_source_accepted: bool,
    ) -> ApiResult<Value> {
        v::id(&record.id)?;
        v::id(&record.manga_id)?;
        v::language(&record.language)?;
        if record.pages.is_empty() || record.pages.len() > 500 || record.groups.len() > 100 {
            return Err(ApiError::bad_request("Invalid chapter page or group count"));
        }
        let origin = url::Url::parse(&record.node)
            .map_err(|_| ApiError::bad_request("Invalid media node"))?;
        if !matches!(origin.scheme(), "http" | "https")
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err(ApiError::bad_request("Invalid media node"));
        }
        for page in record.pages.iter().chain(record.optimized_pages.iter()) {
            if page.name.is_empty()
                || page.name.len() > 200
                || page.name.contains(['/', '\\'])
                || page.name == "."
                || page.name == ".."
                || page.dimensions.iter().any(|n| *n <= 0 || *n > 100_000)
            {
                return Err(ApiError::bad_request("Invalid chapter page"));
            }
        }
        if tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND ($2 OR (protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id))) FOR UPDATE",&[&record.manga_id,&verified_source_accepted]).await?.is_none(){return Err(ApiError::not_found("Published manga not found"));}
        if record.protocol_event_id.is_some() {
            return Err(ApiError::conflict(
                "Protocol chapter association requires the verified import service",
            ));
        }
        validate_groups(tx, &record.groups).await?;
        let mut data = json!({"node":record.node,"data":record.pages,"publication_scope":"instance-local","publication_status":"local"});
        if !record.optimized_pages.is_empty() {
            data["data_optimized"] = json!(record.optimized_pages);
        }
        tx.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,volume,chapter,language,metadata,source_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",&[&record.id,&record.manga_id,&record.uploader_id,&record.uploader_name,&record.title,&record.volume,&record.chapter,&record.language,&data,&record.source_id]).await?;
        for group in &record.groups {
            tx.execute(
                "INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES($1,$2)",
                &[&record.id, group],
            )
            .await?;
        }
        create_thread(
            tx,
            "chapter",
            &record.id,
            &record.uploader_id,
            &record.uploader_name,
            record.title.as_deref().unwrap_or("Chapter discussion"),
        )
        .await?;
        tx.execute(
            "INSERT INTO platform.catalog_chapter_counters(chapter_id) VALUES($1)",
            &[&record.id],
        )
        .await?;
        audit(
            tx,
            &record.uploader_id,
            "chapter",
            &record.id,
            "publish",
            None,
            Some(1),
        )
        .await?;
        let value: Option<Value> = tx
            .query_one("SELECT platform.catalog_chapter_json($1)", &[&record.id])
            .await?
            .get(0);
        match value {
            Some(value) => Ok(value),
            None if verified_source_accepted => Ok(
                json!({"id":record.id,"manga_id":record.manga_id,"version":1,"is_unavailable":true,"publication_status":"pending_projection"}),
            ),
            None => Err(ApiError::unavailable()),
        }
    }
    pub async fn update_chapter(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
        patch: &Value,
    ) -> ApiResult<Value> {
        v::allowed(
            patch,
            &[
                "title", "volume", "chapter", "language", "groups", "version",
            ],
        )?;
        let version = v::version(patch)?;
        let row=tx.query_opt("SELECT uploader_id,version,protocol_event_id FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=manga_id AND parent.deleted_at IS NULL AND parent.state='published' AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id)) FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Chapter not found"))?;
        chapter_authority(tx, id, actor, &row.get::<_, String>(0), "update", false).await?;
        if row.get::<_, i32>(1) != version {
            return Err(ApiError::conflict("Chapter changed; reload before editing"));
        }
        let protocol_overlay = row.get::<_, Option<String>>(2).is_some();
        let title = v::string(patch, "title", 500, false)?;
        let volume = v::string(patch, "volume", 32, false)?;
        let chapter = v::string(patch, "chapter", 32, false)?;
        let language = v::string(patch, "language", 12, false)?;
        if let Some(lang) = &language {
            v::language(lang)?;
        }
        if let Some(groups) = v::ids(patch, "groups", false)? {
            validate_groups(tx, &groups).await?;
            tx.execute(
                "DELETE FROM platform.catalog_chapter_groups WHERE chapter_id=$1",
                &[&id],
            )
            .await?;
            for group in groups {
                tx.execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id) VALUES($1,$2)",&[&id,&group]).await?;
            }
        }
        // These fields are an instance editorial overlay. The signed event, source
        // metadata and originals remain immutable; page replacement is denied above.
        let mut overlay = patch.clone();
        overlay.as_object_mut().unwrap().remove("version");
        overlay["scope"] = json!("instance-local");
        overlay["updated_by"] = json!(actor.user_id());
        tx.execute("UPDATE platform.catalog_chapters SET title=COALESCE($2,title),volume=COALESCE($3,volume),chapter=COALESCE($4,chapter),language=COALESCE($5,language),metadata=CASE WHEN protocol_event_id IS NOT NULL THEN metadata || jsonb_build_object('instance_overlay',COALESCE(metadata->'instance_overlay','{}'::jsonb) || $6::jsonb) ELSE metadata END,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&title,&volume,&chapter,&language,&overlay]).await?;
        audit(
            tx,
            &actor.user_id(),
            "chapter",
            id,
            if protocol_overlay {
                "overlay_update"
            } else {
                "update"
            },
            Some(version),
            Some(version + 1),
        )
        .await?;
        Ok(tx
            .query_one("SELECT platform.catalog_chapter_json($1)", &[&id])
            .await?
            .get(0))
    }
    /// Read-only preflight for an independently durable original deletion request.
    /// The native delete rechecks the same authority under its ordinary row lock.
    pub(crate) async fn check_chapter_delete_authority(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
    ) -> ApiResult<Option<String>> {
        v::id(id)?;
        let row = tx.query_opt(
            "SELECT c.uploader_id,c.protocol_event_id FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",
            &[&id],
        ).await?.ok_or_else(|| ApiError::not_found("Chapter not found"))?;
        chapter_authority(tx, id, actor, &row.get::<_, String>(0), "remove", true).await?;
        Ok(row.get(1))
    }
    pub async fn delete_chapter(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
    ) -> ApiResult<()> {
        let row=tx.query_opt("SELECT uploader_id,version FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=manga_id AND parent.deleted_at IS NULL AND parent.state='published' AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id)) FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Chapter not found"))?;
        chapter_authority(tx, id, actor, &row.get::<_, String>(0), "remove", true).await?;
        tx.execute("UPDATE platform.catalog_chapters SET deleted_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE id=$1",&[&id]).await?;
        let version = row.get::<_, i32>(1);
        audit(
            tx,
            &actor.user_id(),
            "chapter",
            id,
            "remove",
            Some(version),
            Some(version + 1),
        )
        .await?;
        Ok(())
    }
    pub async fn claim_chapter(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
    ) -> ApiResult<Value> {
        actor.require_permission("self.chapter.update")?;
        let row=tx.query_opt("SELECT uploader_id,uploader_name,version FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=manga_id AND parent.deleted_at IS NULL AND parent.state='published' AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id)) FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Chapter not found"))?;
        if row.get::<_, String>(0) == actor.user_id() || row.get::<_, String>(1) == "system" {
            return Err(ApiError::forbidden("Chapter cannot be claimed"));
        }
        if !group_authority(tx, id, &actor.user_id(), true).await? {
            return Err(ApiError::forbidden(
                "A chapter group leader must claim this chapter",
            ));
        }
        tx.execute("UPDATE platform.catalog_chapters SET uploader_id=$2,uploader_name=$3,version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&actor.user_id(),&actor.username()]).await?;
        let version = row.get::<_, i32>(2);
        audit(
            tx,
            &actor.user_id(),
            "chapter",
            id,
            "claim",
            Some(version),
            Some(version + 1),
        )
        .await?;
        Ok(tx
            .query_one("SELECT platform.catalog_chapter_json($1)", &[&id])
            .await?
            .get(0))
    }
    pub async fn publish_cover(
        tx: &Transaction<'_>,
        record: &CoverPublication,
    ) -> ApiResult<Value> {
        v::id(&record.id)?;
        v::language(&record.language)?;
        if !["jpg", "jpeg", "png", "webp", "avif"].contains(&record.ext.as_str())
            || record.dimensions.iter().any(|v| *v <= 0 || *v > 100_000)
            || record.volume.len() > 32
            || record
                .description
                .as_ref()
                .is_some_and(|s| s.len() > 50_000)
        {
            return Err(ApiError::bad_request("Invalid cover metadata"));
        }
        if tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",&[&record.manga_id]).await?.is_none(){return Err(ApiError::not_found("Manga not found"));}
        require_mutable_manga(tx, &record.manga_id).await?;
        if record.main {
            tx.execute(
                "UPDATE platform.catalog_covers SET is_main=FALSE WHERE manga_id=$1",
                &[&record.manga_id],
            )
            .await?;
        }
        tx.execute("INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,volume,language,description,is_main) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",&[&record.id,&record.manga_id,&record.uploader_id,&record.uploader_name,&record.ext,&&record.dimensions[..],&record.volume,&record.language,&record.description,&record.main]).await?;
        audit(
            tx,
            &record.uploader_id,
            "cover",
            &record.id,
            "create",
            None,
            Some(1),
        )
        .await?;
        Ok(tx
            .query_one("SELECT platform.catalog_cover_json($1)", &[&record.id])
            .await?
            .get(0))
    }
    pub async fn update_cover(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
        patch: &Value,
    ) -> ApiResult<Value> {
        v::allowed(
            patch,
            &["volume", "description", "language", "main", "version"],
        )?;
        let version = v::version(patch)?;
        let parent: String = tx
            .query_opt(
                "SELECT manga_id FROM platform.catalog_covers WHERE id=$1 AND deleted_at IS NULL",
                &[&id],
            )
            .await?
            .ok_or_else(|| ApiError::not_found("Cover not found"))?
            .get(0);
        // Every cover writer locks its parent first, matching publication and
        // main-cover changes, rather than taking those locks in opposite orders.
        require_manga_edit(tx, &parent, actor, "manga.cover.edit").await?;
        let row=tx.query_opt("SELECT manga_id,version FROM platform.catalog_covers WHERE id=$1 AND manga_id=$2 AND deleted_at IS NULL FOR UPDATE",&[&id,&parent]).await?.ok_or_else(||ApiError::not_found("Cover not found"))?;
        if row.get::<_, i32>(1) != version {
            return Err(ApiError::conflict("Cover changed; reload before editing"));
        }
        let volume = v::string(patch, "volume", 32, false)?;
        let description = v::string(patch, "description", 50_000, false)?;
        let language = v::string(patch, "language", 12, false)?;
        if let Some(lang) = &language {
            v::language(lang)?;
        }
        let main = v::boolean(patch, "main")?;
        if main == Some(true) {
            tx.execute(
                "UPDATE platform.catalog_covers SET is_main=FALSE WHERE manga_id=$1",
                &[&row.get::<_, String>(0)],
            )
            .await?;
        }
        tx.execute("UPDATE platform.catalog_covers SET is_main=COALESCE($5,is_main),volume=COALESCE($2,volume),description=COALESCE($3,description),language=COALESCE($4,language),version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&id,&volume,&description,&language,&main]).await?;
        audit(
            tx,
            &actor.user_id(),
            "cover",
            id,
            "update",
            Some(version),
            Some(version + 1),
        )
        .await?;
        Ok(tx
            .query_one("SELECT platform.catalog_cover_json($1)", &[&id])
            .await?
            .get(0))
    }
    pub async fn delete_cover(
        tx: &Transaction<'_>,
        id: &str,
        actor: &impl CatalogMutationActor,
    ) -> ApiResult<()> {
        if !actor.is_moderator() {
            return Err(ApiError::forbidden(
                "Staff cover deletion permission required",
            ));
        }
        actor.require_permission("cover.delete")?;
        let parent: String = tx
            .query_opt(
                "SELECT manga_id FROM platform.catalog_covers WHERE id=$1 AND deleted_at IS NULL",
                &[&id],
            )
            .await?
            .ok_or_else(|| ApiError::not_found("Cover not found"))?
            .get(0);
        if tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL FOR UPDATE", &[&parent]).await?.is_none() {
            return Err(ApiError::not_found("Manga not found"));
        }
        require_mutable_manga(tx, &parent).await?;
        let changed=tx.execute("UPDATE platform.catalog_covers SET deleted_at=clock_timestamp(),updated_at=clock_timestamp(),version=version+1 WHERE id=$1 AND deleted_at IS NULL",&[&id]).await?;
        if changed == 0 {
            return Err(ApiError::not_found("Cover not found"));
        }
        audit(tx, &actor.user_id(), "cover", id, "remove", None, None).await?;
        Ok(())
    }
}
pub async fn require_manga_edit(
    tx: &Transaction<'_>,
    id: &str,
    actor: &impl CatalogMutationActor,
    permission: &str,
) -> ApiResult<()> {
    let row=tx.query_opt("SELECT owner_id,locked,state FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("Manga not found"))?;
    if actor.is_moderator() && actor.has_perm(permission) {
        return require_mutable_manga(tx, id).await;
    }
    if row.get::<_, bool>(1) {
        return Err(ApiError::forbidden("Manga is locked"));
    }
    if row.get::<_, String>(2) == "published" {
        actor.require_permission(permission)?;
    } else if row.get::<_, String>(0) == actor.user_id() {
        actor.require_permission("manga.create")?;
    } else {
        return Err(ApiError::forbidden("Only the draft owner can edit it"));
    }
    require_mutable_manga(tx, id).await
}
/// The caller holds the native title row lock. A separately committed source
/// intent may survive native rollback; changing that title would make its exact
/// retry and version-checked recovery impossible. Materialized editorial
/// overlays remain ordinary instance-local writes.
pub(crate) async fn require_mutable_manga(tx: &Transaction<'_>, id: &str) -> ApiResult<()> {
    let pending: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.protocol_publication_commands WHERE command_id=$1 AND resource_kind='manga' AND catalog_id=$2 AND state IN ('prepared','source_accepted'))", &[&format!("manga:{id}"), &id]).await?.get(0);
    if pending {
        return Err(ApiError::conflict("This title has an immutable pending protocol publication; retry its original publish or reconcile the source outcome before editing"));
    }
    Ok(())
}
async fn group_authority(
    tx: &Transaction<'_>,
    id: &str,
    user: &str,
    leader_only: bool,
) -> ApiResult<bool> {
    // Lock eligibility through this native mutation; soft deletion and membership
    // edits must not race a delegation already being exercised in the transaction.
    Ok(tx.query_opt("SELECT g.id FROM platform.catalog_chapter_groups cg JOIN platform.catalog_group_members gm ON gm.group_id=cg.group_id JOIN platform.catalog_groups g ON g.id=gm.group_id WHERE cg.chapter_id=$1 AND gm.user_id=$2 AND (gm.is_leader OR (NOT $3 AND gm.is_officer)) AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) ORDER BY g.id LIMIT 1 FOR SHARE OF g,gm",&[&id,&user,&leader_only]).await?.is_some())
}
async fn chapter_authority(
    tx: &Transaction<'_>,
    id: &str,
    actor: &impl CatalogMutationActor,
    owner: &str,
    action: &str,
    leader_only: bool,
) -> ApiResult<()> {
    if actor.is_moderator() && actor.has_perm(&format!("chapter.{action}")) {
        return Ok(());
    }
    actor.require_permission(&format!("self.chapter.{action}"))?;
    if owner == actor.user_id() || group_authority(tx, id, &actor.user_id(), leader_only).await? {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "Chapter ownership or group authority required",
        ))
    }
}
pub async fn validate_groups(tx: &Transaction<'_>, groups: &[String]) -> ApiResult<()> {
    for group in groups {
        v::id(group)?;
    }
    let rows = tx
        .query(
            "SELECT id FROM platform.catalog_groups WHERE id=ANY($1) AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) FOR SHARE",
            &[&groups],
        )
        .await?;
    let unique: std::collections::BTreeSet<_> = groups.iter().collect();
    if rows.len() != unique.len() {
        return Err(ApiError::bad_request("Unknown group"));
    }
    Ok(())
}
pub async fn create_thread(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    user: &str,
    name: &str,
    subject: &str,
) -> ApiResult<()> {
    let thread = v::new_id("thread");
    tx.execute("INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject) VALUES($1,$2,$3,$4,$5,$6)",&[&thread,&kind,&id,&user,&name,&subject]).await?;
    Ok(())
}
pub async fn audit(
    tx: &Transaction<'_>,
    actor: &str,
    kind: &str,
    id: &str,
    operation: &str,
    old: Option<i32>,
    new: Option<i32>,
) -> ApiResult<()> {
    tx.execute("INSERT INTO platform.catalog_audit(actor_id,entity_type,entity_id,operation,old_version,new_version) VALUES($1,$2,$3,$4,$5,$6)",&[&actor,&kind,&id,&operation,&old,&new]).await?;
    Ok(())
}
