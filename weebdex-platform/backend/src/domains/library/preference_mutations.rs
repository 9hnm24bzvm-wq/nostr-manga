//! Transaction-owned preference validation and known-field merging. Genuine
//! native sessions and explicitly bound public write capabilities share it.
use super::validation as v;
use crate::{contracts::library::PreferencesPatch,domains::catalog::entity_mutations::CatalogMutationActor,error::{ApiError,ApiResult}};
use serde_json::{Value,json};
use std::collections::BTreeSet;
use tokio_postgres::Transaction;

#[derive(Clone,Copy,PartialEq,Eq)]
pub(crate) enum Mode {NativeLegacy,PublicPreserving}
pub(crate) fn patch(p:PreferencesPatch)->ApiResult<(i32,Value)> {
    if p.version<1 {return Err(ApiError::bad_request("A positive preferences version is required"));}
    let mut patch=json!({});
    if let Some(n)=p.items_per_page {
        let value=n.parse::<u32>().map_err(|_|ApiError::bad_request("items_per_page must be a decimal count"))?;
        if !(1..=100).contains(&value) {return Err(ApiError::bad_request("items_per_page must be between 1 and 100"));}
        patch["items_per_page"]=json!(value.to_string());
    }
    for (key,values) in [("languages",p.languages),("translated_languages",p.translated_languages)] {
        if let Some(values)=values {
            let values=v::ids(&values,100)?;
            for language in &values {crate::domains::catalog::validation::language(language)?;}
            patch[key]=json!(values);
        }
    }
    for (key,value) in [("show_safe",p.show_safe),("show_suggestive",p.show_suggestive),("show_erotic",p.show_erotic),("show_hentai",p.show_hentai),("data_saver",p.data_saver)] {
        if let Some(value)=value {patch[key]=json!(value);}
    }
    for (key,values) in [("tags",p.tags),("groups",p.groups),("uploaders",p.uploaders)] {
        if let Some(values)=values {patch[key]=json!(v::ids(&values,100)?);}
    }
    Ok((p.version,patch))
}
pub(crate) async fn update_in(tx:&Transaction<'_>,actor:&impl CatalogMutationActor,version:i32,mut patch:Value,mode:Mode,expected_updated_at:Option<&str>)->ApiResult<bool> {
    actor.require_permission("self.preferences.update")?;
    let user=tx.query_opt("SELECT preferences,platform.catalog_timestamp(updated_at),verified,disabled,roles,permissions FROM platform.users WHERE id=$1 FOR NO KEY UPDATE",&[&actor.user_id()]).await?.ok_or_else(ApiError::unauthorized)?;
    let roles:Vec<String>=serde_json::from_value(user.get(4)).map_err(|_|ApiError::unavailable())?;
    let permissions:Vec<String>=serde_json::from_value(user.get(5)).map_err(|_|ApiError::unavailable())?;
    if user.get::<_,bool>(3) || (mode==Mode::PublicPreserving && !user.get::<_,bool>(2)) || roles.iter().any(|r|matches!(r.as_str(),"banned"|"restricted")) || !permissions.iter().any(|p|p=="self.preferences.update") {return Err(ApiError::forbidden("This account cannot change preferences"));}
    let current:Value=user.get(0);
    if expected_updated_at.is_some_and(|expected|expected!=user.get::<_,&str>(1)) {return Err(ApiError::conflict("Account settings timestamp changed; refresh before saving"));}
    let actual=current.get("version").and_then(Value::as_i64).filter(|v|(1..=i64::from(i32::MAX)).contains(v)).ok_or_else(ApiError::unavailable)?;
    if actual!=i64::from(version) {return Err(ApiError::conflict("Preferences changed; reload before saving"));}
    for (key,table,visibility) in [
        ("tags","platform.catalog_tags r","TRUE"),
        ("groups","platform.catalog_groups r","r.deleted_at IS NULL AND platform.protocol_group_is_visible(r.id)"),
        ("uploaders","platform.users r","NOT r.disabled"),
    ] {
        let Some(values)=patch.get(key) else{continue};
        let mut ids:Vec<String>=serde_json::from_value(values.clone()).map_err(|_|ApiError::bad_request("Invalid preference identifiers"))?;
        let visible=tx.query(&format!("SELECT r.id FROM {table} WHERE r.id=ANY($1::text[]) AND {visibility}"),&[&ids]).await?.into_iter().map(|r|r.get::<_,String>(0)).collect::<BTreeSet<_>>();
        if visible.len()!=ids.len() {return Err(ApiError::bad_request(format!("Unknown {key} preference")));}
        if mode==Mode::PublicPreserving {
            if let Some(stored)=current.get(key) {
                let old:Vec<String>=serde_json::from_value(stored.clone()).map_err(|_|ApiError::unavailable())?;
                if old.len()>100 || old.iter().any(|s|s.is_empty() || s.len()>128 || s.contains('\0')) {return Err(ApiError::unavailable());}
                let visible_old=tx.query(&format!("SELECT r.id FROM {table} WHERE r.id=ANY($1::text[]) AND {visibility}"),&[&old]).await?.into_iter().map(|r|r.get::<_,String>(0)).collect::<BTreeSet<_>>();
                for value in old {if !visible_old.contains(&value) && !ids.contains(&value) {ids.push(value);}}
                if ids.len()>100 {return Err(ApiError::bad_request(format!("{key} plus preserved unavailable filters exceed 100 entries")));}
                patch[key]=json!(ids);
            }
        }
    }
    let fields=patch.as_object().ok_or_else(ApiError::unavailable)?;
    let changed=!fields.is_empty() && (mode==Mode::NativeLegacy || fields.iter().any(|(key,value)|current.get(key)!=Some(value)));
    if changed {
        patch["version"]=json!(version.checked_add(1).ok_or_else(||ApiError::conflict("Preference version limit reached"))?);
        tx.execute("UPDATE platform.users SET preferences=preferences || $2,updated_at=clock_timestamp() WHERE id=$1",&[&actor.user_id(),&patch]).await?;
    }
    Ok(changed)
}
