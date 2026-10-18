//! Native profile validation, locked CAS and audit shared by genuine sessions
//! and request-bound account writes. No protocol metadata or key is changed.
use super::validation as v;
use crate::{contracts::library::ProfilePatch,domains::catalog::entity_mutations::CatalogMutationActor,error::{ApiError,ApiResult}};
use serde_json::{Value,json};
use tokio_postgres::{GenericClient,Transaction};

pub(crate) async fn update_in(tx:&Transaction<'_>,actor:&impl CatalogMutationActor,id:&str,p:&ProfilePatch)->ApiResult<()> {
    let own=id==actor.user_id();
    let permission=if own {"self.user.update"} else {"user.update"};
    actor.require_permission(permission)?;
    let patch=v::profile(p)?;
    if patch.as_object().expect("profile object").is_empty() {return Err(ApiError::bad_request("No profile changes supplied"));}
    let next=p.version.checked_add(1).ok_or_else(||ApiError::conflict("Profile version limit reached"))?;
    // The current native actor row is locked even for native session callers.
    // Stale session claims cannot survive a concurrent grant/role withdrawal.
    let user=tx.query_opt("SELECT disabled,verified,roles,permissions FROM platform.users WHERE id=$1 FOR NO KEY UPDATE",&[&actor.user_id()]).await?.ok_or_else(ApiError::unauthorized)?;
    let roles:Vec<String>=serde_json::from_value(user.get(2)).map_err(|_|ApiError::unavailable())?;
    let permissions:Vec<String>=serde_json::from_value(user.get(3)).map_err(|_|ApiError::unavailable())?;
    if user.get::<_,bool>(0) || roles.iter().any(|r|matches!(r.as_str(),"banned"|"restricted")) || !permissions.iter().any(|p|p==permission)
        || (!own && (!user.get::<_,bool>(1) || !actor.is_moderator() || !roles.iter().any(|r|matches!(r.as_str(),"staff"|"sysop")))) {
        return Err(ApiError::forbidden("This account cannot edit that profile"));
    }
    let row=tx.query_opt("SELECT version FROM platform.users WHERE id=$1 AND NOT disabled FOR NO KEY UPDATE",&[&id]).await?.ok_or_else(||ApiError::not_found("User not found"))?;
    if row.get::<_,i32>(0)!=p.version {return Err(ApiError::conflict("Profile changed; reload before editing"));}
    tx.execute("UPDATE platform.users SET profile=(profile || $2),version=$3,updated_at=clock_timestamp() WHERE id=$1",&[&id,&patch,&next]).await?;
    for (key,value) in patch.as_object().expect("profile object") {
        if value.as_str()==Some("") {tx.execute("UPDATE platform.users SET profile=profile-$2 WHERE id=$1",&[&id,&key]).await?;}
    }
    tx.execute("INSERT INTO platform.catalog_audit(actor_id,entity_type,entity_id,operation,old_version,new_version) VALUES($1,'user',$2,'update',$3,$4)",&[&actor.user_id(),&id,&p.version,&next]).await?;
    Ok(())
}

/// Only the four fields already public in the native profile. Never select the
/// full JSON object, email, preferences, grants, or private extension keys.
pub(crate) async fn read_in(db:&impl GenericClient,id:&str)->ApiResult<Value> {
    let row=db.query_opt("SELECT username,CASE WHEN octet_length(roles::text)<=32768 THEN roles ELSE NULL END,version,CASE WHEN octet_length(fields::text)<=196608 THEN fields ELSE NULL END FROM platform.users CROSS JOIN LATERAL (SELECT jsonb_strip_nulls(jsonb_build_object('description',profile->'description','discord',profile->'discord','twitter',profile->'twitter','website',profile->'website')) AS fields) public_profile WHERE id=$1 AND NOT disabled",&[&id]).await?.ok_or_else(||ApiError::not_found("User not found"))?;
    let roles:Vec<String>=serde_json::from_value(row.get::<_,Option<Value>>(1).ok_or_else(ApiError::unavailable)?).map_err(|_|ApiError::unavailable())?;
    if roles.len()>128 || roles.iter().any(|r|r.is_empty() || r.len()>128) {return Err(ApiError::unavailable());}
    let version:i32=row.get(2);if version<1 {return Err(ApiError::unavailable());}
    let fields:Value=row.get::<_,Option<Value>>(3).ok_or_else(ApiError::unavailable)?;
    let p:ProfilePatch=serde_json::from_value(json!({"version":version,"description":fields.get("description"),"discord":fields.get("discord"),"twitter":fields.get("twitter"),"website":fields.get("website")})).map_err(|_|ApiError::unavailable())?;
    // Fail closed on legacy malformed public values rather than broadening the
    // native URL contract or forwarding private/unknown metadata.
    let mut attributes=v::profile(&p).map_err(|_|ApiError::unavailable())?;
    attributes["username"]=json!(row.get::<_,String>(0));attributes["roles"]=json!(roles);attributes["version"]=json!(version);
    Ok(attributes)
}
