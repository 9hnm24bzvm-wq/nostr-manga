//! Private native membership mutations. No protocol graph is created or signed.
use crate::{domains::catalog::entity_mutations::CatalogMutationActor,error::{ApiError,ApiResult}};
use serde_json::{Value,json};
use tokio_postgres::Transaction;

#[derive(Clone,Copy,Debug)]
pub(crate) enum Target {Manga,Group,User}
impl Target {
    pub(crate) fn parts(self)->(&'static str,&'static str,&'static str) {
        match self {
            Self::Manga=>("manga","platform.library_manga_follows","manga_id"),
            Self::Group=>("group","platform.library_group_follows","group_id"),
            Self::User=>("user","platform.library_user_follows","target_id"),
        }
    }
}
pub(crate) async fn state_in(tx:&Transaction<'_>,user:&str,target:Target,id:&str)->ApiResult<Value> {
    let (kind,table,column)=target.parts();
    let row=tx.query_opt("SELECT followed,version,platform.catalog_timestamp(updated_at) FROM platform.library_follow_revisions WHERE user_id=$1 AND kind=$2 AND target_id=$3",&[&user,&kind,&id]).await?;
    let actual=tx.query_opt(&format!("SELECT platform.catalog_timestamp(followed_at) FROM {table} WHERE user_id=$1 AND {column}=$2"),&[&user,&id]).await?;
    if row.as_ref().is_some_and(|r|r.get::<_,bool>(0))!=actual.is_some() {return Err(ApiError::unavailable());}
    Ok(json!({"followed":actual.is_some(),"version":row.as_ref().map_or(0,|r|r.get::<_,i32>(1)),"followedAt":actual.map(|r|r.get::<_,String>(0)),"updatedAt":row.map(|r|r.get::<_,String>(2))}))
}
pub(crate) async fn mutate_in(tx:&Transaction<'_>,actor:&impl CatalogMutationActor,target:Target,id:&str,follow:bool,version:Option<i32>,expected_account:Option<&str>)->ApiResult<Value> {
    if version.is_some_and(|v|v<0) {return Err(ApiError::bad_request("Follow version must be nonnegative"));}
    actor.require_permission("self.preferences.update")?;
    let user=tx.query_opt("SELECT verified,disabled,roles,permissions FROM platform.users WHERE id=$1 FOR NO KEY UPDATE",&[&actor.user_id()]).await?.ok_or_else(ApiError::unauthorized)?;
    let roles:Vec<String>=serde_json::from_value(user.get(2)).map_err(|_|ApiError::unavailable())?;
    let permissions:Vec<String>=serde_json::from_value(user.get(3)).map_err(|_|ApiError::unavailable())?;
    if !user.get::<_,bool>(0) || user.get::<_,bool>(1) || roles.iter().any(|r|matches!(r.as_str(),"banned"|"restricted")) || !permissions.iter().any(|p|p=="self.preferences.update") {return Err(ApiError::forbidden("This account cannot change follows"));}
    let account_id=super::account_public_id_in(tx,actor.user_id()).await?;
    if expected_account.is_some_and(|id|id!=account_id) {return Err(ApiError::conflict("Account binding changed; refresh before submitting"));}
    if matches!(target,Target::User) && id==actor.user_id() {return Err(ApiError::bad_request("An account cannot follow itself"));}
    if follow {
        let sql=match target {
            Target::Manga=>"SELECT id FROM platform.catalog_manga WHERE id=$1 AND deleted_at IS NULL AND state='published' AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id) FOR NO KEY UPDATE",
            Target::Group=>"SELECT id FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id) FOR SHARE",
            Target::User=>"SELECT id FROM platform.users WHERE id=$1 AND NOT disabled FOR SHARE NOWAIT",
        };
        tx.query_opt(sql,&[&id]).await?.ok_or_else(||ApiError::not_found("Follow target not found"))?;
    }
    let current=state_in(tx,actor.user_id(),target,id).await?;
    let actual=current["version"].as_i64().ok_or_else(ApiError::unavailable)? as i32;
    if version.is_some_and(|v|v!=actual) {return Err(ApiError::conflict("Follow membership changed; refresh before submitting"));}
    let changed=current["followed"]!=follow;
    let (kind,table,column)=target.parts();
    if changed {
        actual.checked_add(1).ok_or_else(||ApiError::conflict("Follow version exhausted"))?;
        if follow {
            if matches!(target,Target::Manga) {
                // New follows use existing native private/notification defaults;
                // a repeated follow never overwrites an existing reading status.
                super::repository::upsert_follow(tx,actor.user_id(),id,"plan_to_read",None,None).await?;
            } else {
                tx.execute(&format!("INSERT INTO {table}(user_id,{column}) VALUES($1,$2) ON CONFLICT(user_id,{column}) DO NOTHING"),&[&actor.user_id(),&id]).await?;
            }
        } else {
            let removed=tx.execute(&format!("DELETE FROM {table} WHERE user_id=$1 AND {column}=$2"),&[&actor.user_id(),&id]).await?;
            if removed>0 && matches!(target,Target::Manga) {tx.execute("UPDATE platform.catalog_manga_counters SET follows=GREATEST(0,follows-1) WHERE manga_id=$1",&[&id]).await?;}
        }
    }
    let mut result=state_in(tx,actor.user_id(),target,id).await?;
    result["scope"]=json!("instance-account");result["source"]=json!("persisted-native-account-follows");result["accountId"]=json!(account_id);result["kind"]=json!(kind);result["changed"]=json!(changed);result["versionSemantics"]=json!("membership-change-revision");
    Ok(result)
}
