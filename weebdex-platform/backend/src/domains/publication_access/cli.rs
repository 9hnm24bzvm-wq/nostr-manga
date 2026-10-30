//! Offline reviewed administration only; no HTTP endpoint and no startup caller.
use super::*;
use sha2::{Digest, Sha256};
fn text(value: &str, max: usize, label: &str) -> ApiResult<()> {
    if value.trim().is_empty()
        || value.chars().count() > max
        || value.chars().any(|c| c.is_control())
    {
        return Err(ApiError::bad_request(format!(
            "Give a bounded {label} without credentials or control characters"
        )));
    }
    Ok(())
}
fn review_id(id: &str) -> ApiResult<()> {
    if id.len() != 36 || crate::public_api::PublicUuid::parse(id).is_err() {
        return Err(ApiError::bad_request(
            "Select the exact publication access review ID",
        ));
    }
    Ok(())
}
fn digest(v: &Value) -> ApiResult<String> {
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(v).map_err(|_| ApiError::unavailable())?,
    )))
}
fn request(op: &str, user: &str, publisher: &str, label: &str, reason: &str) -> ApiResult<()> {
    user_id(user)?;
    key(publisher)?;
    text(label, 100, "operator reference")?;
    text(reason, 500, "reason")?;
    if !matches!(op, "grant" | "revoke") {
        return Err(ApiError::bad_request("Choose grant or revoke"));
    }
    Ok(())
}
async fn account_lock(tx: &Transaction<'_>, user: &str) -> ApiResult<()> {
    if tx
        .query_opt(
            "SELECT id FROM platform.users WHERE id=$1 FOR UPDATE NOWAIT",
            &[&user],
        )
        .await
        .map_err(lock_error)?
        .is_none()
    {
        return Err(ApiError::not_found("Native account not found"));
    }
    tx.query("SELECT user_id FROM platform.native_publication_delegations WHERE user_id=$1 FOR UPDATE NOWAIT",&[&user]).await.map_err(lock_error)?;
    Ok(())
}
async fn unresolved(tx: &Transaction<'_>, user: &str) -> ApiResult<Value> {
    let publication=tx.query_one("SELECT count(*) FILTER(WHERE state='prepared'),count(*) FILTER(WHERE state='source_accepted') FROM platform.protocol_publication_commands WHERE user_id=$1 AND state<>'materialized'",&[&user]).await.map_err(lock_error)?;
    let prepared: i64 = publication.get(0);
    let accepted: i64 = publication.get(1);
    let withdrawals:i64=tx.query_one("SELECT count(*) FROM platform.protocol_deletion_intents WHERE actor_user_id=$1 AND state='prepared'",&[&user]).await.map_err(lock_error)?.get(0);
    let rows=tx.query("SELECT state,count(*) FROM platform.scheduled_uploads WHERE owner_id=$1 AND state NOT IN('complete','cancelled') GROUP BY state ORDER BY state",&[&user]).await.map_err(lock_error)?;
    let mut schedules = serde_json::Map::new();
    let mut schedule_count = 0i64;
    for row in rows {
        let count: i64 = row.get(1);
        schedule_count += count;
        schedules.insert(row.get::<_, String>(0), json!(count));
    }
    Ok(
        json!({"present":prepared>0||accepted>0||withdrawals>0||schedule_count>0,"preparedPublicationCommands":prepared,"sourceAcceptedPublicationCommands":accepted,"preparedSourceWithdrawalIntents":withdrawals,"schedulesByState":schedules,"warning":"Revocation only contains dedicated native publish-now authority. Prepared commands remain uncertain; source-accepted events may be irreversible. No command, schedule, accepted event or source withdrawal is cancelled, retracted or deleted."}),
    )
}
async fn snapshot(
    state: &AppState,
    tx: &Transaction<'_>,
    op: &str,
    user: &str,
    publisher: &str,
    label: &str,
    reason: &str,
) -> ApiResult<Value> {
    let row=tx.query_opt("SELECT id,username,verified,disabled,roles ?| ARRAY['banned','restricted'],authority_version,version,account_lifecycle_id::text FROM platform.users WHERE id=$1",&[&user]).await?.ok_or_else(||ApiError::not_found("Native account not found"))?;
    let before = delegation_in(tx, user, false).await?;
    let configured = state
        .config
        .protocol_publication
        .as_ref()
        .map(|v| v.expected_publisher.clone());
    let admission = publisher_admission_in(tx, publisher, true).await?;
    let legacy=tx.query("SELECT publisher FROM platform.protocol_identity_bindings WHERE user_id=$1 AND revoked_at IS NULL ORDER BY publisher LIMIT 2 FOR SHARE NOWAIT",&[&user]).await.map_err(lock_error)?;
    if op == "grant" {
        if !row.get::<_, bool>(2) || row.get::<_, bool>(3) || row.get::<_, bool>(4) {
            return Err(ApiError::forbidden(
                "Grant requires an enabled verified unrestricted native account",
            ));
        }
        if configured.as_deref() != Some(publisher) || state.config.protocol_read_only {
            return Err(ApiError::forbidden(
                "Grant requires the exact already-configured original operator publisher",
            ));
        }
        if !admission.is_some_and(|(allowed, _)| allowed) {
            return Err(ApiError::forbidden(
                "The configured publisher must already be admitted by the original service; this CLI never admits it",
            ));
        }
        if !legacy.is_empty() {
            return Err(ApiError::conflict(
                "An active legacy signer binding already exists for this account. This publication-only workflow cannot replace, merge or broaden it.",
            ));
        }
        if before
            .as_ref()
            .is_some_and(|d| !d.revoked && d.publisher != publisher)
        {
            return Err(ApiError::conflict(
                "Revoke the existing different publisher delegation explicitly before reviewing a replacement",
            ));
        }
    } else if before.as_ref().is_some_and(|d| d.publisher != publisher) {
        return Err(ApiError::conflict(
            "The selected public publisher does not match this account's stored delegation",
        ));
    }
    let unresolved = unresolved(tx, user).await?;
    if op == "grant" && unresolved["present"] == true {
        return Err(ApiError::conflict(
            "Unresolved original publication, withdrawal or scheduled work blocks grant/regrant. Old captures cannot adopt new authority; accepted source events are irreversible. Reviewed containment-only revoke remains available, but does not cancel or unpublish anything.",
        ));
    }
    Ok(
        json!({"scope":"native-publication-access-review","operation":op,"operatorLabel":label,"reason":reason,"nativeAccount":{"id":row.get::<_,String>(0),"username":row.get::<_,String>(1),"verified":row.get::<_,bool>(2),"disabled":row.get::<_,bool>(3),"restricted":row.get::<_,bool>(4),"authorityVersion":row.get::<_,i32>(5),"version":row.get::<_,i32>(6),"lifecycleId":row.get::<_,String>(7)},"requestedPublisher":publisher,"configuredPublisher":configured,"originalPublisherAdmission":admission.map(|(allowed,version)|json!({"allowed":allowed,"version":version})),"currentDelegation":before,"legacyBindingPresent":!legacy.is_empty(),"unresolvedWork":unresolved,"capabilities":{"nativeCookiePublishNow":op=="grant","manifestKinds":[1001,1002],"scheduledPublication":false,"groupPublication":false,"sourceWithdrawal":false,"signerAuthenticatesAsNativeUser":false,"nativeRolesOrGrantsChanged":false,"originalAdmissionChanged":false,"portableIdentityLinkChanged":false},"effect":if op=="grant"{"Allow only this native account's current ordinary browser session to publish immediately through the configured original publisher, subject to current native permissions and original admission epoch. No signing key, native role, signer login, group, schedule or withdrawal authority is granted. Runtime key/storage health is not attested by this read-only review."}else{"Revoke only this native account's dedicated publish-now delegation. This does not retract accepted source events, cancel native commands, alter legacy signed identity bindings or revoke original publisher admission."}}),
    )
}
/// Persists only a bounded reviewed intent; no delegation is changed. One
/// unsettled review per target prevents a new no-op review clearing uncertainty.
pub async fn review(
    state: &AppState,
    op: &str,
    user: &str,
    publisher: &str,
    label: &str,
    reason: &str,
) -> ApiResult<Value> {
    request(op, user, publisher, label, reason)?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    serialize(&tx).await?;
    account_lock(&tx, user).await?;
    if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.publication_access_reviews r WHERE user_id=$1 AND NOT EXISTS(SELECT 1 FROM platform.publication_access_receipts p WHERE p.review_id=r.id))",&[&user]).await?.get::<_,bool>(0){return Err(ApiError::conflict("This account has an unsettled access review. Use publication-access pending, then check or explicitly cancel that exact review before creating another."));}
    let count:i64=tx.query_one("SELECT count(*) FROM platform.publication_access_reviews WHERE operator_label=$1 AND created_at>clock_timestamp()-interval '1 hour'",&[&label]).await?.get(0);
    if count >= 100 {
        return Err(ApiError::busy());
    }
    let summary = snapshot(state, &tx, op, user, publisher, label, reason).await?;
    let clock = tx
        .query_one(
            "SELECT gen_random_uuid()::text,(clock_timestamp()+interval '10 minutes')::text",
            &[],
        )
        .await?;
    let id: String = clock.get(0);
    let expires: String = clock.get(1);
    let reviewed = json!({"reviewId":id,"expiresAt":expires,"summary":summary});
    let hash = digest(&reviewed)?;
    tx.execute("INSERT INTO platform.publication_access_reviews(id,operator_label,user_id,operation,digest,review,expires_at) VALUES($1::text::uuid,$2,$3,$4,$5,$6,$7::text::timestamptz)",&[&id,&label,&user,&op,&hash,&reviewed,&expires]).await?;
    tx.commit().await?;
    let mut out = reviewed;
    out["digest"] = json!(hash);
    Ok(out)
}
async fn stored(tx: &Transaction<'_>, id: &str, label: &str, hash: &str) -> ApiResult<Value> {
    review_id(id)?;
    text(label, 100, "operator reference")?;
    key(hash)?;
    let r=tx.query_opt("SELECT operator_label,digest,review FROM platform.publication_access_reviews WHERE id=$1::text::uuid",&[&id]).await?.ok_or_else(||ApiError::not_found("Publication access review not found; absence is not proof an earlier command did not apply"))?;
    if r.get::<_, String>(0) != label || r.get::<_, String>(1) != hash {
        return Err(ApiError::forbidden(
            "Review operator reference or digest does not match",
        ));
    }
    Ok(r.get(2))
}
pub async fn inspect(state: &AppState, id: &str, label: &str, hash: &str) -> ApiResult<Value> {
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    bounded(&tx).await?;
    let mut result = stored(&tx, id, label, hash).await?;
    result["digest"] = json!(hash);
    tx.commit().await?;
    Ok(result)
}
pub async fn pending(state: &AppState, user: &str) -> ApiResult<Value> {
    user_id(user)?;
    let db = state.db.client().await?;
    let rows=db.query("SELECT id::text,operator_label,operation,digest,expires_at::text FROM platform.publication_access_reviews r WHERE user_id=$1 AND NOT EXISTS(SELECT 1 FROM platform.publication_access_receipts p WHERE p.review_id=r.id) ORDER BY created_at LIMIT 2",&[&user]).await?;
    Ok(
        json!({"scope":"native-publication-access-pending","userId":user,"reviews":rows.iter().map(|r|json!({"reviewId":r.get::<_,String>(0),"operatorLabel":r.get::<_,String>(1),"operation":r.get::<_,String>(2),"digest":r.get::<_,String>(3),"expiresAt":r.get::<_,String>(4)})).collect::<Vec<_>>(),"message":"An absent receipt is not a cancellation. Check or cancel each exact review before a new one; no command is replayed."}),
    )
}
pub async fn receipt(state: &AppState, id: &str, label: &str, hash: &str) -> ApiResult<Value> {
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    bounded(&tx).await?;
    stored(&tx, id, label, hash).await?;
    let result=tx.query_opt("SELECT receipt FROM platform.publication_access_receipts WHERE review_id=$1::text::uuid",&[&id]).await?.map(|r|r.get(0)).unwrap_or_else(||json!({"scope":"native-publication-access-receipt","reviewId":id,"digest":hash,"pending":true,"message":"No committed receipt is visible; do not create a replacement or assume the old command cannot apply."}));
    tx.commit().await?;
    Ok(result)
}
pub async fn settle(
    state: &AppState,
    id: &str,
    label: &str,
    hash: &str,
    cancel: bool,
) -> ApiResult<Value> {
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    serialize(&tx).await?;
    let reviewed = stored(&tx, id, label, hash).await?;
    if let Some(r)=tx.query_opt("SELECT receipt FROM platform.publication_access_receipts WHERE review_id=$1::text::uuid",&[&id]).await?{let result=r.get(0);tx.commit().await?;return Ok(result)}
    let summary = &reviewed["summary"];
    let value = |name: &str| summary[name].as_str().ok_or_else(ApiError::unavailable);
    let user = summary["nativeAccount"]["id"]
        .as_str()
        .ok_or_else(ApiError::unavailable)?;
    let op = value("operation")?;
    let publisher = value("requestedPublisher")?;
    let reason = value("reason")?;
    if cancel {
        let audit = audit_in(
            &tx,
            user,
            "offline-operator",
            label,
            Some(id),
            "cancel-review",
            "Operator cancelled an unapplied publication-access review",
            &None,
            &None,
        )
        .await?;
        let result = json!({"scope":"native-publication-access-receipt","reviewId":id,"digest":hash,"userId":user,"publisher":publisher,"operation":op,"outcome":"cancelled-before-apply","changed":false,"auditId":audit.to_string(),"sourceEventsRetracted":false});
        tx.execute("INSERT INTO platform.publication_access_receipts(review_id,operator_label,digest,receipt) VALUES($1::text::uuid,$2,$3,$4)",&[&id,&label,&hash,&result]).await?;
        tx.commit().await?;
        return Ok(result);
    }
    account_lock(&tx, user).await?;
    let current = snapshot(state, &tx, op, user, publisher, label, reason).await?;
    if &current != summary {
        return Err(ApiError::conflict(
            "Account, delegation, configuration or original admission changed since review. Nothing was changed; cancel this review and inspect a fresh one.",
        ));
    }
    if !tx.query_one("SELECT expires_at>clock_timestamp() FROM platform.publication_access_reviews WHERE id=$1::text::uuid",&[&id]).await?.get::<_,bool>(0){return Err(ApiError::conflict("Publication access review expired. Nothing was changed; cancel it and make a fresh review."));}
    let before = delegation_in(&tx, user, false).await?;
    if op == "grant" {
        let epoch = summary["originalPublisherAdmission"]["version"]
            .as_i64()
            .ok_or_else(ApiError::unavailable)?;
        tx.execute("INSERT INTO platform.native_publication_delegations(user_id,publisher,publisher_admission_version) VALUES($1,$2,$3) ON CONFLICT(user_id) DO UPDATE SET publisher=EXCLUDED.publisher,publisher_admission_version=EXCLUDED.publisher_admission_version,revoked_at=NULL",&[&user,&publisher,&epoch]).await.map_err(lock_error)?;
    } else {
        tx.execute("UPDATE platform.native_publication_delegations SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL",&[&user]).await.map_err(lock_error)?;
    }
    let after = delegation_in(&tx, user, false).await?;
    let changed = before != after;
    let audit = audit_in(
        &tx,
        user,
        "offline-operator",
        label,
        Some(id),
        op,
        reason,
        &before,
        &after,
    )
    .await?;
    let result = json!({"scope":"native-publication-access-receipt","reviewId":id,"digest":hash,"userId":user,"publisher":publisher,"operation":op,"outcome":"applied","changed":changed,"delegation":after,"auditId":audit.to_string(),"sourceEventsRetracted":false,"nativeRolesOrGrantsChanged":false,"originalAdmissionChanged":false,"unresolvedWork":summary["unresolvedWork"]});
    tx.execute("INSERT INTO platform.publication_access_receipts(review_id,operator_label,digest,receipt) VALUES($1::text::uuid,$2,$3,$4)",&[&id,&label,&hash,&result]).await?;
    tx.commit().await?;
    Ok(result)
}
