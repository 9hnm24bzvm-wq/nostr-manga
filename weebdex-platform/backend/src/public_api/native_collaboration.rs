//! Voluntary native team membership. Membership and credits never grant source authority.
use super::{
    PublicApiError, PublicApiResult, PublicUuid, ResourceKind,
    native_account_profile_settings as account,
    native_contributions::{self, NativeActor},
    uploads::{preferred, resolve},
};
use crate::{
    AppState,
    domains::catalog::{CatalogRepository, entity_mutations::CatalogMutationActor, validation},
};
use axum::{
    Json, Router,
    extract::{OriginalUri, Path, Request, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/groups", get(groups))
        .route("/account/group-requests", get(inbox))
        .route("/account/group/{id}/team", get(team))
        .route("/account/group/{id}/requests", post(create_request))
        .route(
            "/account/group/{id}/requests/{request}/decision",
            post(decide),
        )
        .route(
            "/account/chapter/{id}/credits",
            get(credits_read).put(credits_write),
        )
        .route("/chapter/{id}/credits", get(public_credits))
}
fn eligible(actor: &NativeActor) -> PublicApiResult<()> {
    actor.0.require_browser_session()?;
    if actor
        .0
        .roles
        .iter()
        .any(|r| matches!(r.as_str(), "banned" | "restricted"))
    {
        return Err(PublicApiError::forbidden(
            "This account cannot join or manage teams",
        ));
    }
    Ok(())
}
fn text<'a>(input: &'a Value, key: &str, max: usize, required: bool) -> PublicApiResult<&'a str> {
    let v = input.get(key).and_then(Value::as_str).unwrap_or("");
    if (required && v.trim().is_empty())
        || v.chars().count() > max
        || v.contains('\0')
        || input.get(key).is_some_and(|v| !v.is_string())
    {
        return Err(PublicApiError::bad_request(format!("Invalid {key}")));
    }
    Ok(v.trim())
}
async fn group_lock(tx: &Transaction<'_>, group: &str) -> PublicApiResult<i32> {
    Ok(tx.query_opt("SELECT version FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND NOT locked AND platform.protocol_group_is_visible(id) FOR UPDATE",&[&group]).await?.ok_or_else(PublicApiError::not_found)?.get(0))
}
async fn leader(tx: &Transaction<'_>, group: &str, user: &str) -> PublicApiResult<bool> {
    Ok(tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND user_id=$2 AND is_leader)",&[&group,&user]).await?.get(0))
}
async fn require_leader(
    tx: &Transaction<'_>,
    group: &str,
    actor: &NativeActor,
) -> PublicApiResult<()> {
    actor.0.require_permission("self.scanlation_group.update")?;
    if !leader(tx, group, actor.user_id()).await? {
        return Err(PublicApiError::forbidden(
            "Only the current group leader can manage invitations",
        ));
    }
    Ok(())
}
async fn account_active(
    tx: &Transaction<'_>,
    user: &str,
    leader_grant: bool,
) -> PublicApiResult<String> {
    // NOWAIT avoids reversing the actor/group lock order in two-account races.
    let row=tx.query_opt("SELECT username,verified,roles,permissions FROM platform.users WHERE id=$1 AND NOT disabled FOR SHARE NOWAIT",&[&user]).await?.ok_or_else(||PublicApiError::forbidden("The selected account is unavailable"))?;
    let roles: Value = row.get(2);
    let permissions: Value = row.get(3);
    if !row.get::<_, bool>(1)
        || roles
            .as_array()
            .is_none_or(|v| v.iter().any(|r| r == "banned" || r == "restricted"))
        || leader_grant
            && !permissions
                .as_array()
                .is_some_and(|v| v.iter().any(|p| p == "self.scanlation_group.update"))
    {
        return Err(PublicApiError::forbidden(
            "The selected account is no longer eligible",
        ));
    }
    Ok(row.get(0))
}
pub(super) async fn own_groups(
    tx: &Transaction<'_>,
    actor: &NativeActor,
) -> PublicApiResult<Value> {
    let rows=tx.query("SELECT g.id,g.name,m.is_leader,m.is_officer FROM platform.catalog_group_members m JOIN platform.catalog_groups g ON g.id=m.group_id WHERE m.user_id=$1 AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) ORDER BY m.is_leader DESC,lower(g.name),g.id LIMIT 101",&[&actor.user_id()]).await?;
    let more = rows.len() > 100;
    let mut data = Vec::new();
    for r in rows.into_iter().take(100) {
        let id: String = r.get(0);
        data.push(json!({"id":preferred(tx,ResourceKind::Group,&id).await?,"name":r.get::<_,String>(1),"role":if r.get::<_,bool>(2){"leader"}else if r.get::<_,bool>(3){"officer"}else{"member"}}));
    }
    Ok(json!({"data":data,"hasMore":more,"limit":100}))
}
fn response(mut value: Value, account: &str) -> Json<Value> {
    value["result"] = json!("ok");
    account::provenance(&mut value, account);
    Json(value)
}
async fn groups(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    account::body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = native_contributions::actor(&state, &headers, &tx, false).await?;
    eligible(&actor)?;
    let id = preferred(&tx, ResourceKind::User, actor.user_id()).await?;
    let result = own_groups(&tx, &actor).await?;
    tx.commit().await?;
    Ok(response(result, id.as_str()))
}
async fn request_rows(
    tx: &Transaction<'_>,
    user: &str,
    group: Option<&str>,
    is_leader: bool,
    offset: i64,
) -> PublicApiResult<Value> {
    let rows=tx.query("SELECT r.id::text,r.group_id,g.name,r.user_id,u.username,r.kind,r.version,r.message,platform.catalog_timestamp(r.expires_at),r.created_by FROM platform.native_group_requests r JOIN platform.catalog_groups g ON g.id=r.group_id JOIN platform.users u ON u.id=r.user_id WHERE r.status='pending' AND r.expires_at>clock_timestamp() AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) AND ($2::text IS NULL OR r.group_id=$2) AND (r.user_id=$1 OR ($3 AND $2::text IS NOT NULL)) ORDER BY r.created_at DESC,r.id LIMIT 101 OFFSET $4",&[&user,&group,&is_leader,&offset]).await?;
    let more = rows.len() > 100;
    let mut data = Vec::new();
    for r in rows.into_iter().take(100) {
        let gid: String = r.get(1);
        let uid: String = r.get(3);
        let created: String = r.get(9);
        data.push(json!({"id":r.get::<_,String>(0),"groupId":preferred(tx,ResourceKind::Group,&gid).await?,"groupName":r.get::<_,String>(2),"userId":preferred(tx,ResourceKind::User,&uid).await?,"username":r.get::<_,String>(4),"kind":r.get::<_,String>(5),"version":r.get::<_,i32>(6),"message":r.get::<_,String>(7),"expiresAt":r.get::<_,String>(8),"createdBy":preferred(tx,ResourceKind::User,&created).await?}));
    }
    Ok(json!({"data":data,"hasMore":more,"limit":100,"offset":offset}))
}
async fn inbox(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    account::body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = native_contributions::actor(&state, &headers, &tx, false).await?;
    eligible(&actor)?;
    let id = preferred(&tx, ResourceKind::User, actor.user_id()).await?;
    let result = request_rows(&tx, actor.user_id(), None, false, 0).await?;
    tx.commit().await?;
    Ok(response(result, id.as_str()))
}
async fn team(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    let mut offset = 0_i64;
    let mut seen = false;
    for (key, value) in url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()) {
        if key != "offset" || seen || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(PublicApiError::bad_request("Invalid request page"));
        }
        offset = value
            .parse::<i64>()
            .ok()
            .filter(|v| *v <= 500 && *v % 100 == 0)
            .ok_or_else(|| PublicApiError::bad_request("Invalid request offset"))?;
        seen = true;
    }
    account::body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = native_contributions::actor(&state, &headers, &tx, false).await?;
    eligible(&actor)?;
    let account = preferred(&tx, ResourceKind::User, actor.user_id()).await?;
    let public = PublicUuid::parse(&id)?;
    let group = resolve(&tx, ResourceKind::Group, &public).await?;
    let row=tx.query_opt("SELECT name,version,locked FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id)",&[&group]).await?.ok_or_else(PublicApiError::not_found)?;
    let can_manage = leader(&tx, &group, actor.user_id()).await?
        && actor.has_perm("self.scanlation_group.update")
        && !row.get::<_, bool>(2);
    let rows=tx.query("SELECT user_id,name,is_leader,is_officer FROM platform.catalog_group_members WHERE group_id=$1 ORDER BY is_leader DESC,is_officer DESC,name,user_id LIMIT 101",&[&group]).await?;
    let more = rows.len() > 100;
    let mut members = Vec::new();
    let mut own_role = None;
    for r in rows.into_iter().take(100) {
        let user: String = r.get(0);
        let role = if r.get::<_, bool>(2) {
            "leader"
        } else if r.get::<_, bool>(3) {
            "officer"
        } else {
            "member"
        };
        if user == actor.user_id() {
            own_role = Some(role);
        }
        members.push(json!({"id":preferred(&tx,ResourceKind::User,&user).await?,"name":r.get::<_,String>(1),"role":role}));
    }
    let requests = request_rows(&tx, actor.user_id(), Some(&group), can_manage, offset).await?;
    let result = json!({"group":{"id":public,"name":row.get::<_,String>(0),"version":row.get::<_,i32>(1)},"members":members,"hasMoreMembers":more,"ownRole":own_role,"canManage":can_manage,"requests":requests});
    tx.commit().await?;
    Ok(response(result, account.as_str()))
}
async fn create_request(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let bytes = account::body(request, &headers, true, 12000).await?;
    let mut input: Value = super::native_json::parse(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid team request"))?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = native_contributions::actor(&state, &headers, &tx, true).await?;
    eligible(&actor)?;
    let account = native_contributions::capture(&tx, &actor, &mut input).await?;
    validation::allowed(&input, &["kind", "recipient", "message", "version"])?;
    let version = validation::version(&input)?;
    let kind = text(&input, "kind", 16, true)?;
    let message = text(&input, "message", 1000, false)?;
    if !["invitation", "application"].contains(&kind) {
        return Err(PublicApiError::bad_request(
            "Choose an invitation or join application",
        ));
    }
    let group = resolve(&tx, ResourceKind::Group, &PublicUuid::parse(&id)?).await?;
    if group_lock(&tx, &group).await? != version {
        return Err(PublicApiError::conflict(
            "Group changed; reload before continuing",
        ));
    }
    let user = if kind == "invitation" {
        require_leader(&tx, &group, &actor).await?;
        resolve(
            &tx,
            ResourceKind::User,
            &PublicUuid::parse(text(&input, "recipient", 36, true)?)?,
        )
        .await?
    } else {
        if input.get("recipient").is_some() {
            return Err(PublicApiError::bad_request(
                "Apply only for your own account",
            ));
        }
        actor.user_id().to_owned()
    };
    // Serialize this recipient's cross-group queue budget, not only this group.
    // SHARE would allow two different leaders to both add request number 100.
    tx.query_opt(
        "SELECT id FROM platform.users WHERE id=$1 AND NOT disabled FOR NO KEY UPDATE NOWAIT",
        &[&user],
    )
    .await?
    .ok_or_else(|| PublicApiError::forbidden("The selected account is unavailable"))?;
    account_active(&tx, &user, false).await?;
    if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND user_id=$2)",&[&group,&user]).await?.get::<_,bool>(0){return Err(PublicApiError::conflict("This account already belongs to the group"));}
    tx.execute("UPDATE platform.native_group_requests SET status='expired',version=version+1,decided_at=clock_timestamp() WHERE group_id=$1 AND status='pending' AND expires_at<=clock_timestamp()",&[&group]).await?;
    if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.native_group_requests WHERE group_id=$1 AND user_id=$2 AND kind=$3 AND status='pending') OR (SELECT count(*) FROM platform.native_group_requests WHERE user_id=$2 AND status='pending' AND expires_at>clock_timestamp())>=100 OR (SELECT count(*) FROM platform.native_group_requests WHERE group_id=$1 AND status='pending' AND expires_at>clock_timestamp())>=500",&[&group,&user,&kind]).await?.get::<_,bool>(0){return Err(PublicApiError::conflict("A request already exists or the pending request limit has been reached"));}
    let id:String=tx.query_one("INSERT INTO platform.native_group_requests(id,group_id,user_id,created_by,kind,message) VALUES(gen_random_uuid(),$1,$2,$3,$4,$5) RETURNING id::text",&[&group,&user,&actor.user_id(),&kind,&message]).await?.get(0);
    tx.commit().await?;
    Ok(response(
        json!({"id":id,"version":1,"status":"pending","role":"member"}),
        &account,
    ))
}
async fn decide(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path((id, request_id)): Path<(String, String)>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let bytes = account::body(request, &headers, true, 4096).await?;
    let mut input: Value = super::native_json::parse(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid team decision"))?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = native_contributions::actor(&state, &headers, &tx, true).await?;
    eligible(&actor)?;
    let account = native_contributions::capture(&tx, &actor, &mut input).await?;
    validation::allowed(&input, &["action", "version"])?;
    let version = validation::version(&input)?;
    let action = text(&input, "action", 16, true)?;
    let request_id = PublicUuid::parse(&request_id)?;
    let group = resolve(&tx, ResourceKind::Group, &PublicUuid::parse(&id)?).await?;
    let group_version = group_lock(&tx, &group).await?;
    let row=tx.query_opt("SELECT user_id,created_by,kind,status,version,expires_at>clock_timestamp() FROM platform.native_group_requests WHERE id=$1::text::uuid AND group_id=$2 FOR UPDATE",&[&request_id.as_str(),&group]).await?.ok_or_else(PublicApiError::not_found)?;
    let user: String = row.get(0);
    let creator: String = row.get(1);
    let kind: String = row.get(2);
    if row.get::<_, String>(3) != "pending"
        || row.get::<_, i32>(4) != version
        || !row.get::<_, bool>(5)
    {
        return Err(PublicApiError::conflict(
            "This request changed or expired; reload the team",
        ));
    }
    let recipient = user == actor.user_id();
    match (kind.as_str(), action) {
        ("invitation", "accept" | "decline") if recipient => {}
        ("invitation", "revoke") => require_leader(&tx, &group, &actor).await?,
        ("application", "cancel") if recipient => {}
        ("application", "accept" | "decline") => require_leader(&tx, &group, &actor).await?,
        _ => {
            return Err(PublicApiError::forbidden(
                "This account cannot make that decision",
            ));
        }
    }
    if action == "accept" {
        if kind == "invitation" {
            if !leader(&tx, &group, &creator).await? {
                return Err(PublicApiError::forbidden(
                    "The inviter is no longer the group leader",
                ));
            }
            account_active(&tx, &creator, true).await?;
        }
        let name = account_active(&tx, &user, false).await?;
        if tx
            .query_one(
                "SELECT count(*)>=100 FROM platform.catalog_group_members WHERE group_id=$1",
                &[&group],
            )
            .await?
            .get::<_, bool>(0)
        {
            return Err(PublicApiError::conflict(
                "The group membership limit has been reached",
            ));
        }
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_group_members WHERE group_id=$1 AND user_id=$2)",&[&group,&user]).await?.get::<_,bool>(0){return Err(PublicApiError::conflict("This account already belongs to the group"));}
        tx.execute("INSERT INTO platform.catalog_group_members(group_id,user_id,name,is_leader,is_officer) VALUES($1,$2,$3,FALSE,FALSE)",&[&group,&user,&name]).await?;
        tx.execute("UPDATE platform.catalog_groups SET version=version+1,updated_at=clock_timestamp() WHERE id=$1",&[&group]).await?;
        crate::domains::catalog::repository::audit(
            &tx,
            actor.user_id(),
            "group",
            &group,
            "member_join",
            Some(group_version),
            Some(group_version + 1),
        )
        .await?;
    }
    let status = match action {
        "accept" => "accepted",
        "decline" => "declined",
        "revoke" => "revoked",
        "cancel" => "cancelled",
        _ => unreachable!(),
    };
    tx.execute("UPDATE platform.native_group_requests SET status=$2,version=version+1,decided_at=clock_timestamp(),decided_by=$3 WHERE id=$1::text::uuid",&[&request_id.as_str(),&status,&actor.user_id()]).await?;
    tx.commit().await?;
    Ok(response(
        json!({"id":request_id,"version":version+1,"status":status,"role":"member"}),
        &account,
    ))
}
fn credits(input: &Value) -> PublicApiResult<Value> {
    let rows = input
        .as_array()
        .filter(|v| v.len() <= 30)
        .ok_or_else(|| PublicApiError::bad_request("Use at most 30 credits"))?;
    let mut out = Vec::new();
    for row in rows {
        validation::allowed(row, &["role", "name"])?;
        let role = text(row, "role", 20, true)?;
        let name = text(row, "name", 120, true)?;
        if name.chars().any(char::is_control) {
            return Err(PublicApiError::bad_request(
                "Credit names cannot contain control characters",
            ));
        }
        if !["translator", "editor", "typesetter"].contains(&role) {
            return Err(PublicApiError::bad_request("Unknown credit role"));
        }
        let item = json!({"role":role,"name":name});
        if out.contains(&item) {
            return Err(PublicApiError::bad_request("Duplicate chapter credit"));
        }
        out.push(item);
    }
    Ok(json!(out))
}
async fn read_credits(tx: &Transaction<'_>, id: &str) -> PublicApiResult<Value> {
    let row=tx.query_opt("SELECT c.version,COALESCE(a.credits,'[]'::jsonb) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id LEFT JOIN platform.native_chapter_credits a ON a.chapter_id=c.id WHERE c.id=$1 AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)",&[&id]).await?.ok_or_else(PublicApiError::not_found)?;
    Ok(
        json!({"version":row.get::<_,i32>(0),"credits":row.get::<_,Value>(1),"scope":"instance-local-public-attribution"}),
    )
}
async fn credits_read(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    account::body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = native_contributions::actor(&state, &headers, &tx, false).await?;
    eligible(&actor)?;
    let account = preferred(&tx, ResourceKind::User, actor.user_id()).await?;
    let id = resolve(&tx, ResourceKind::Chapter, &PublicUuid::parse(&id)?).await?;
    let result = read_credits(&tx, &id).await?;
    tx.commit().await?;
    Ok(response(result, account.as_str()))
}
async fn public_credits(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    account::body(request, &headers, false, 1024).await?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let id = resolve(&tx, ResourceKind::Chapter, &PublicUuid::parse(&id)?).await?;
    let mut result = read_credits(&tx, &id).await?;
    result["result"] = json!("ok");
    tx.commit().await?;
    Ok(Json(result))
}
async fn credits_write(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> PublicApiResult<Json<Value>> {
    super::uploads::no_query(&uri)?;
    let bytes = account::body(request, &headers, true, 16000).await?;
    let mut input: Value = super::native_json::parse(&bytes)
        .map_err(|_| PublicApiError::bad_request("Invalid credits"))?;
    let mut db = state.db.client().await?;
    let tx = db.transaction().await?;
    let actor = native_contributions::actor(&state, &headers, &tx, true).await?;
    eligible(&actor)?;
    let account = native_contributions::capture(&tx, &actor, &mut input).await?;
    validation::allowed(&input, &["version", "credits"])?;
    let version = validation::version(&input)?;
    let credits = credits(&input["credits"])?;
    let id = resolve(&tx, ResourceKind::Chapter, &PublicUuid::parse(&id)?).await?;
    // Reuse the sole catalogue writer for exact current chapter authority and CAS.
    CatalogRepository::update_chapter(&tx, &id, &actor, &json!({"version":version})).await?;
    tx.execute("INSERT INTO platform.native_chapter_credits(chapter_id,credits,version,updated_by) VALUES($1,$2,$3,$4) ON CONFLICT(chapter_id) DO UPDATE SET credits=EXCLUDED.credits,version=EXCLUDED.version,updated_by=EXCLUDED.updated_by,updated_at=clock_timestamp()",&[&id,&credits,&(version+1),&actor.user_id()]).await?;
    let result = read_credits(&tx, &id).await?;
    tx.commit().await?;
    Ok(response(result, &account))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credit_validation_is_bounded_and_has_no_authority_fields() {
        assert!(credits(&json!([{"role":"admin","name":"Person"}])).is_err());
        assert!(
            credits(&json!([{"role":"editor","name":"Person","permissions":["all"]}])).is_err()
        );
        assert!(credits(&json!([{"role":"editor","name":" "}])).is_err());
        assert_eq!(
            credits(&json!([{"role":"translator","name":" Example "}])).unwrap(),
            json!([{"role":"translator","name":"Example"}])
        );
    }
}
