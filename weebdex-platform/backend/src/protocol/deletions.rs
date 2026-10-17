//! Native explicit delete -> actual original signed request -> moderator review.
//! A native soft hide does not approve source deletion or remove peer copies.
use crate::{
    domains::catalog::{CatalogRepository, entity_mutations::CatalogMutationActor},
    error::{ApiError, ApiResult},
    state::AppState,
};
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct NativeDeletionReceipt {
    pub source_event_id: String,
    pub request_id: String,
    pub source_status: &'static str,
    pub instance_effect: &'static str,
}
fn id(text: &str) -> ApiResult<[u8; 32]> {
    if !manga_node::auth::valid_hash(text) {
        return Err(ApiError::unavailable());
    }
    hex::decode(text)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(ApiError::unavailable)
}
/// Runs before the native write transaction. Native deletion rechecks authority
/// under its normal row lock after this independently durable source request.
pub(crate) async fn request_native_chapter_deletion(
    state: &AppState,
    actor: &impl CatalogMutationActor,
    chapter: &str,
) -> ApiResult<Option<NativeDeletionReceipt>> {
    let source = {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        let source = CatalogRepository::check_chapter_delete_authority(&tx, chapter, actor).await?;
        tx.commit().await?;
        source
    };
    let Some(target) = source else {
        return Ok(None);
    };
    let runtime = state.protocol.publication_runtime().await?;
    let mut source_client = state.db.client().await?;
    let source_tx = source_client.transaction().await?;
    // Serialize this whole durable native action through original submission,
    // including the uncertain group-signing response window, not just insertion.
    let locked:bool=source_tx.query_one("SELECT pg_try_advisory_xact_lock(hashtextextended('platform-delete:'||$1::text||':'||$2::text,0))", &[&chapter,&actor.user_id()]).await?.get(0);
    if !locked {
        return Err(ApiError::busy());
    }
    // The original facade owns visibility/signature/generation validation and
    // holds a source row lock across independent intent and request persistence.
    let (source, generation) =
        manga_node::platform_bridge::lock_deletion_source(&*source_tx, id(&target)?)
            .await
            .map_err(|_| ApiError::unavailable())?
            .ok_or_else(|| ApiError::not_found("Original publication is unavailable"))?;
    let publisher = source.pubkey.clone();
    let member = state
        .db
        .client()
        .await?
        .query_opt(
            "SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1",
            &[&actor.user_id()],
        )
        .await?
        .map(|r| r.get::<_, String>(0))
        .ok_or_else(|| {
            ApiError::new(axum::http::StatusCode::FORBIDDEN,"source_deletion_not_authorized","This account has no configured source deletion identity")
        })?;
    let group=state.db.client().await?.query_opt("SELECT group_id FROM platform.protocol_identity_bindings WHERE publisher=$1 AND group_id IS NOT NULL", &[&publisher]).await?.map(|r|r.get::<_,String>(0));
    if group.is_none() && (member != publisher || runtime.publisher_identity() != publisher) {
        return Err(ApiError::new(axum::http::StatusCode::FORBIDDEN,"source_deletion_not_authorized",
            "This account cannot sign deletion requests for the original publisher",
        ));
    }
    runtime
        .authorize_bound_deletion(
            group.as_ref().map(|_| id(&publisher)).transpose()?,
            id(&member)?,
        )
        .await
        .map_err(|_| ApiError::new(axum::http::StatusCode::FORBIDDEN,"source_deletion_not_authorized","Current original deletion authority is unavailable"))?;
    let (intent_uuid, mut prepared) = {
        let mut client = state.db.client().await?;
        let tx = client.transaction().await?;
        tx.execute("INSERT INTO platform.protocol_deletion_intents(chapter_id,actor_user_id,source_event_id,source_generation,authority_publisher,member_publisher) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING", &[&chapter,&actor.user_id(),&target,&generation,&publisher,&member]).await?;
        let row=tx.query_one("SELECT source_event_id,authority_publisher,member_publisher,intent_uuid::text,request_event FROM platform.protocol_deletion_intents WHERE chapter_id=$1 AND actor_user_id=$2 AND source_generation=$3 FOR UPDATE", &[&chapter,&actor.user_id(),&generation]).await?;
        if row.get::<_, String>(0) != target
            || row.get::<_, String>(1) != publisher
            || row.get::<_, String>(2) != member
        {
            return Err(ApiError::conflict(
                "Source deletion authority changed for this current prepared action",
            ));
        }
        let prepared: Option<manga_node::auth::Event> = row
            .get::<_, Option<serde_json::Value>>(4)
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| ApiError::unavailable())?;
        let uuid: String = row.get(3);
        tx.commit().await?;
        (uuid, prepared)
    };
    let reason = deletion_intent_reason(&intent_uuid, generation);
    if let Some(event) = &prepared {
        validate_intent_event(event, &publisher, &target, &reason)?;
    }
    if group.is_none() && prepared.is_none() {
        let event = runtime
            .prepare_bound_operator_deletion(&target, &reason)
            .map_err(|_| ApiError::unavailable())?;
        let raw = serde_json::to_value(&event).map_err(|_| ApiError::unavailable())?;
        // Commit the exact operator body before submission, independent of the
        // eventual native soft-hide transaction and its uncertain response.
        state.db.client().await?.execute("UPDATE platform.protocol_deletion_intents SET request_event=COALESCE(request_event,$4) WHERE chapter_id=$1 AND actor_user_id=$2 AND source_generation=$3", &[&chapter,&actor.user_id(),&generation,&raw]).await?;
        prepared=state.db.client().await?.query_one("SELECT request_event FROM platform.protocol_deletion_intents WHERE chapter_id=$1 AND actor_user_id=$2 AND source_generation=$3", &[&chapter,&actor.user_id(),&generation]).await?.get::<_,Option<serde_json::Value>>(0)
            .map(serde_json::from_value).transpose().map_err(|_|ApiError::unavailable())?;
    }
    let event = if group.is_some() {
        // Recovery searches only this durable UUID/generation content and one
        // exact target in the original group audit, never arbitrary history.
        let event = runtime
            .request_bound_group_deletion(id(&publisher)?, id(&member)?, &target, &reason)
            .await
            .map_err(|_| {
                ApiError::forbidden("Original managed group remove authority is unavailable")
            })?;
        if prepared.as_ref().is_some_and(|old| old.id != event.id) {
            return Err(ApiError::conflict("Original group deletion intent changed"));
        }
        event
    } else {
        let event = prepared.ok_or_else(ApiError::unavailable)?;
        validate_intent_event(&event, &publisher, &target, &reason)?;
        runtime
            .submit_bound_operator_deletion(&event)
            .await
            .map_err(|_| {
                ApiError::forbidden("Original publisher deletion authority is unavailable")
            })?;
        event
    };
    validate_intent_event(&event, &publisher, &target, &reason)?;
    let raw = serde_json::to_value(&event).map_err(|_| ApiError::unavailable())?;
    state.db.client().await?.execute("UPDATE platform.protocol_deletion_intents SET request_event=$4,state='requested',updated_at=clock_timestamp() WHERE chapter_id=$1 AND actor_user_id=$2 AND source_generation=$3", &[&chapter,&actor.user_id(),&generation,&raw]).await?;
    source_tx.commit().await?;
    Ok(Some(NativeDeletionReceipt {
        source_event_id: target,
        request_id: event.id,
        source_status: "moderator_review_requested",
        instance_effect: "instance_soft_hide",
    }))
}

fn deletion_intent_reason(uuid: &str, generation: i64) -> String {
    format!(
        "Native chapter deletion intent {uuid} for original generation {generation}; authorized uploader requested moderator review"
    )
}
fn validate_intent_event(
    event: &manga_node::auth::Event,
    publisher: &str,
    target: &str,
    reason: &str,
) -> ApiResult<()> {
    event.verify().map_err(|_| ApiError::unavailable())?;
    if event.kind != 5
        || event.pubkey != publisher
        || event.values("e") != vec![target]
        || event.content != reason
    {
        return Err(ApiError::unavailable());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_native_deletion_intent_never_replays_other_generations_or_multiple_targets() {
        let secret = "01".repeat(32);
        let target = "ab".repeat(32);
        let reason = deletion_intent_reason("00000000-0000-4000-8000-000000000001", 1);
        let event = manga_node::auth::sign(
            &secret,
            5,
            vec![vec!["e".into(), target.clone()]],
            reason.clone(),
        )
        .unwrap();
        assert!(validate_intent_event(&event, &event.pubkey, &target, &reason).is_ok());
        assert!(
            validate_intent_event(
                &event,
                &event.pubkey,
                &target,
                &deletion_intent_reason("00000000-0000-4000-8000-000000000001", 3)
            )
            .is_err()
        );
        assert!(
            validate_intent_event(
                &event,
                &event.pubkey,
                &target,
                &deletion_intent_reason("00000000-0000-4000-8000-000000000002", 1)
            )
            .is_err()
        );
        let multiple = manga_node::auth::sign(
            &secret,
            5,
            vec![
                vec!["e".into(), target.clone()],
                vec!["e".into(), "cd".repeat(32)],
            ],
            reason.clone(),
        )
        .unwrap();
        assert!(validate_intent_event(&multiple, &multiple.pubkey, &target, &reason).is_err());
    }
}
