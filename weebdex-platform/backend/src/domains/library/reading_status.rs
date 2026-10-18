//! Actual native follow status and its independently persisted change revision.
//! No source event, membership change or device-local preference is synthesized.
use crate::{contracts::library::ReadingStatus,domains::catalog::entity_mutations::CatalogMutationActor,error::{ApiError,ApiResult}};
use serde_json::{Value,json};
use tokio_postgres::Transaction;

pub(crate) async fn state_in(tx:&Transaction<'_>,user:&str,manga:&str,lock:bool)->ApiResult<Value> {
    let sql=if lock {"SELECT reading_status FROM platform.library_manga_follows WHERE user_id=$1 AND manga_id=$2 FOR UPDATE"} else {"SELECT reading_status FROM platform.library_manga_follows WHERE user_id=$1 AND manga_id=$2"};
    let follow=tx.query_opt(sql,&[&user,&manga]).await?;
    let revision=tx.query_opt("SELECT status,followed,version,to_char(updated_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') FROM platform.library_reading_status_revisions WHERE user_id=$1 AND manga_id=$2",&[&user,&manga]).await?;
    let status=follow.as_ref().map(|r|r.get::<_,String>(0));
    if revision.as_ref().is_some_and(|r|r.get::<_,bool>(1))!=follow.is_some()
        || revision.as_ref().and_then(|r|r.get::<_,Option<String>>(0))!=status {
        return Err(ApiError::unavailable());
    }
    Ok(json!({"status":status,"followed":follow.is_some(),"version":revision.as_ref().map_or(0,|r|r.get::<_,i32>(2)),"updatedAt":revision.map(|r|r.get::<_,String>(3))}))
}
pub(crate) async fn mutate_in(tx:&Transaction<'_>,actor:&impl CatalogMutationActor,manga:&str,status:ReadingStatus,version:i32)->ApiResult<Value> {
    actor.require_permission("self.preferences.update")?;
    if version<0 {return Err(ApiError::bad_request("Status version must be nonnegative"));}
    // The public bound-account helper already locks/rechecks this user and its
    // binding. This core also requires the genuine actor, never a public UUID.
    let current=state_in(tx,actor.user_id(),manga,true).await?;
    if current["version"]!=version {return Err(ApiError::conflict("Reading status changed; refresh before submitting"));}
    if current["followed"]!=true {return Err(ApiError::conflict("Follow this manga with the instance account before editing reading status"));}
    let changed=current["status"]!=status.as_str();
    if changed {
        version.checked_add(1).ok_or_else(||ApiError::conflict("Status version exhausted"))?;
        // Only status and its actual source update time change. Existing
        // private/notification/follow time and counters are untouched.
        tx.execute("UPDATE platform.library_manga_follows SET reading_status=$3,updated_at=clock_timestamp() WHERE user_id=$1 AND manga_id=$2",&[&actor.user_id(),&manga,&status.as_str()]).await?;
    }
    let mut result=state_in(tx,actor.user_id(),manga,false).await?;
    result["changed"]=json!(changed);Ok(result)
}
