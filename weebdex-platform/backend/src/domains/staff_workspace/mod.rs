//! Native staff workspace, intentionally separate from original-service operators.
mod instance;
mod operations;
mod profile;
mod reports;
mod history;
mod corrections;
pub use instance::InstanceInformation;
use crate::{AppState, ApiResult, domains::auth};
use axum::{Json, Router, extract::State, http::HeaderMap, routing::{get,post}};
use serde_json::{Value,json};
pub fn router()->Router<AppState>{
    Router::new()
        .route("/instance-information",get(instance::information))
        .route("/staff/context",get(context))
        .route("/staff/operations",get(operations::read))
        .route("/staff/users/{id}/profile",get(profile::read).put(profile::write))
        .route("/staff/reports",get(reports::list))
        .route("/staff/reviewers",get(reports::reviewers))
        .route("/staff/reports/{id}/review",get(reports::preview))
        .route("/staff/reports/{id}/decision",post(reports::decide))
        .route("/staff/reports/{id}/work",post(reports::work))
        .route("/staff/receipts/{id}",get(reports::receipt_read))
        .route("/staff/history",get(history::list))
        .route("/staff/reports/{id}/correction",get(corrections::preview).post(corrections::correct))
        .route("/staff/reports/{id}/reopen",post(corrections::reopen))
        .route("/my/reports/{id}/history",get(history::personal))
        .route("/my/reports/{id}/appeal",post(corrections::appeal))
}
async fn context(State(s):State<AppState>,h:HeaderMap)->ApiResult<Json<Value>>{
    let mut db=s.db.client().await?;let tx=db.transaction().await?;
    let a=auth::require_native_account_principal_in(&s,&h,&tx,false).await?;
    let result=json!({"scope":"native-staff-context","actorId":a.user_id,"username":a.username,"roles":a.roles,"permissions":a.permissions,"verified":a.verified,"moderator":a.is_moderator(),"staff":a.is_staff(),"sourceOperatorAuthority":false});
    tx.commit().await?;Ok(Json(result))
}
