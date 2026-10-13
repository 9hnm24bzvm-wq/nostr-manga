//! Only platform.* account rows are changed here. No protocol Repository or signing key is used.
use crate::{
    db::PgStore,
    error::{ApiError, ApiResult},
};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

pub async fn session_projection(db: &PgStore, user: &str) -> ApiResult<Value> {
    let row=db.client().await?.query_opt("SELECT jsonb_build_object('follows',COALESCE((SELECT jsonb_object_agg(f.manga_id,jsonb_build_array(f.reading_status,CASE WHEN f.notification THEN 1 ELSE 0 END)) FROM platform.library_manga_follows f JOIN platform.catalog_manga m ON m.id=f.manga_id WHERE f.user_id=u.id AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)),'{}'::jsonb),'preferences',u.preferences || jsonb_build_object('uploaders',COALESCE((SELECT jsonb_agg(platform.library_user_json(x.id) ORDER BY x.username,x.id) FROM platform.users x WHERE NOT x.disabled AND x.id IN (SELECT jsonb_array_elements_text(u.preferences->'uploaders'))),'[]'::jsonb),'groups',COALESCE((SELECT jsonb_agg(platform.catalog_group_json(g.id) ORDER BY g.name,g.id) FROM platform.catalog_groups g WHERE g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) AND g.id IN (SELECT jsonb_array_elements_text(u.preferences->'groups'))),'[]'::jsonb)),'groups',COALESCE((SELECT jsonb_agg(platform.catalog_group_json(g.id) ORDER BY g.name,g.id) FROM platform.catalog_group_members gm JOIN platform.catalog_groups g ON g.id=gm.group_id WHERE gm.user_id=u.id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)),'[]'::jsonb)) FROM platform.users u WHERE u.id=$1 AND NOT u.disabled",&[&user]).await?.ok_or_else(ApiError::unauthorized)?;
    Ok(row.get(0))
}
pub async fn lock_user(tx: &Transaction<'_>, user: &str) -> ApiResult<()> {
    if tx
        .query_opt(
            "SELECT id FROM platform.users WHERE id=$1 AND NOT disabled FOR UPDATE",
            &[&user],
        )
        .await?
        .is_none()
    {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}
pub async fn published_manga(tx: &Transaction<'_>, id: &str) -> ApiResult<()> {
    if tx.query_opt("SELECT id FROM platform.catalog_manga WHERE id=$1 AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR KEY SHARE",&[&id]).await?.is_none(){return Err(ApiError::not_found("Published manga not found"));}
    Ok(())
}
pub async fn validate_manga_ids(tx: &Transaction<'_>, ids: &[String]) -> ApiResult<()> {
    let count:i64=tx.query_one("SELECT count(*) FROM platform.catalog_manga WHERE id=ANY($1) AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)",&[&ids]).await?.get(0);
    if count as usize != ids.len() {
        return Err(ApiError::bad_request(
            "List contains an unknown or unpublished manga",
        ));
    }
    Ok(())
}
pub async fn upsert_follow(
    tx: &Transaction<'_>,
    user: &str,
    manga: &str,
    status: &str,
    notification: Option<bool>,
    private: Option<bool>,
) -> ApiResult<()> {
    let added=tx.execute("INSERT INTO platform.library_manga_follows(user_id,manga_id,reading_status,notification,is_private) VALUES($1,$2,$3,COALESCE($4,TRUE),COALESCE($5,TRUE)) ON CONFLICT(user_id,manga_id) DO NOTHING",&[&user,&manga,&status,&notification,&private]).await?;
    if added == 0 {
        tx.execute("UPDATE platform.library_manga_follows SET reading_status=$3,notification=COALESCE($4,notification),is_private=COALESCE($5,is_private),updated_at=clock_timestamp() WHERE user_id=$1 AND manga_id=$2",&[&user,&manga,&status,&notification,&private]).await?;
    } else {
        tx.execute("INSERT INTO platform.catalog_manga_counters(manga_id,follows) VALUES($1,1) ON CONFLICT(manga_id) DO UPDATE SET follows=platform.catalog_manga_counters.follows+1",&[&manga]).await?;
    }
    Ok(())
}
pub async fn nested_history(db: &PgStore, user: &str, ids: &[String]) -> ApiResult<Value> {
    let rows=db.client().await?.query("SELECT h.manga_id,h.chapter_id FROM platform.library_chapter_history h JOIN platform.catalog_chapters c ON c.id=h.chapter_id JOIN platform.catalog_manga m ON m.id=h.manga_id WHERE h.user_id=$1 AND (cardinality($2::TEXT[])=0 OR h.manga_id=ANY($2)) AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) ORDER BY h.manga_id,h.chapter_id LIMIT 10001",&[&user,&ids]).await?;
    if rows.len() > 10_000 {
        return Err(ApiError::bad_request(
            "History selection exceeds 10000 chapters; select fewer manga",
        ));
    }
    let mut out = serde_json::Map::new();
    for id in ids {
        out.insert(id.clone(), json!({}));
    }
    for row in rows {
        let manga: String = row.get(0);
        let chapter: String = row.get(1);
        out.entry(manga)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("history object")
            .insert(chapter, Value::Null);
    }
    Ok(Value::Object(out))
}
const LIST_PROJECTION: &str = r#"jsonb_build_object('id',l.id,'name',l.name,'description',l.description,'visibility',l.visibility,'version',l.version,'created_at',platform.catalog_timestamp(l.created_at),'updated_at',platform.catalog_timestamp(l.updated_at),'relationships',jsonb_build_object('owner',platform.library_user_json(l.user_id),'manga',COALESCE((SELECT jsonb_agg(platform.catalog_manga_json(m.id) ORDER BY lm.position,m.id) FROM platform.library_list_members lm JOIN platform.catalog_manga m ON m.id=lm.manga_id WHERE lm.list_id=l.id AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)),'[]'::jsonb)))"#;
pub async fn list_json(tx: &Transaction<'_>, id: &str) -> ApiResult<Value> {
    Ok(tx
        .query_one(
            &format!("SELECT {LIST_PROJECTION} FROM platform.library_lists l WHERE l.id=$1"),
            &[&id],
        )
        .await?
        .get(0))
}
/// Visibility, owner admission and contents share one PostgreSQL statement
/// snapshot. A public-to-private transition cannot expose newer private contents
/// after an earlier authorization lookup.
pub async fn authorized_list_json(
    db: &impl tokio_postgres::GenericClient,
    id: &str,
    reader: Option<&str>,
) -> ApiResult<Value> {
    db.query_opt(&format!("SELECT {LIST_PROJECTION} FROM platform.library_lists l WHERE l.id=$1 AND (l.visibility<>'private' OR l.user_id=$2)"), &[&id,&reader]).await?
        .map(|row| row.get(0)).ok_or_else(|| ApiError::not_found("List not found"))
}
pub async fn replace_members(tx: &Transaction<'_>, id: &str, ids: &[String]) -> ApiResult<()> {
    validate_manga_ids(tx, ids).await?;
    tx.execute(
        "DELETE FROM platform.library_list_members WHERE list_id=$1",
        &[&id],
    )
    .await?;
    for (position, manga) in ids.iter().enumerate() {
        let position = position as i32;
        tx.execute(
            "INSERT INTO platform.library_list_members(list_id,manga_id,position) VALUES($1,$2,$3)",
            &[&id, &manga, &position],
        )
        .await?;
    }
    Ok(())
}
