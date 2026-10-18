use super::{preference_mutations as core,repository};
use crate::{contracts::library::PreferencesPatch,domains::auth,error::ApiResult,state::AppState};
use axum::{extract::State,http::HeaderMap,Json};
use serde_json::Value;

pub async fn update(State(state):State<AppState>,headers:HeaderMap,Json(p):Json<PreferencesPatch>)->ApiResult<Json<Value>> {
    let actor=auth::require_principal(&state,&headers).await?;
    auth::validate_mutation(&state,&headers)?;
    actor.require_permission("self.preferences.update")?;
    let (version,patch)=core::patch(p)?;
    let mut db=state.db.client().await?;let tx=db.transaction().await?;
    core::update_in(&tx,&actor,version,patch,core::Mode::NativeLegacy,None).await?;
    tx.commit().await?;
    let projection=repository::session_projection(&state.db,&actor.user_id).await?;
    Ok(Json(projection["preferences"].clone()))
}
