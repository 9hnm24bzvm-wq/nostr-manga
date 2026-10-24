//! Bounded live reader selection, including one native-account Continue answer.
//! No unbounded aggregate or history map is returned.
//! A transaction protects one answer only; cursors do not promise a snapshot.
use super::{query::{QueryParams, Sql, chapter_filters, manga_filters}, reader_order::{self, Profile}, validation};
use crate::{AppState, error::{ApiError, ApiResult}};
use axum::{Json, extract::{Path, RawQuery, State}, http::{HeaderMap, HeaderValue, header::CACHE_CONTROL}};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, time::Duration};
use tokio_postgres::Transaction;

pub(crate) const MAX_PAGE: usize = 100;
const MAX_GROUPS: usize = 100;
const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_CURSOR_BYTES: usize = 2048;
const TABLE: &str = "platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="kebab-case")]
pub(crate) enum Operation { Groups, Editions, First, Last, Around, Next, Previous, ResolveGroup }
impl Operation {
    fn parse(value: &str) -> ApiResult<Self> {
        match value {
            "groups" => Ok(Self::Groups), "editions" => Ok(Self::Editions),
            "first" => Ok(Self::First), "last" => Ok(Self::Last), "around" => Ok(Self::Around),
            "next" => Ok(Self::Next), "previous" => Ok(Self::Previous), "resolve-group" => Ok(Self::ResolveGroup),
            _ => Err(ApiError::bad_request("Invalid reader operation")),
        }
    }
}
#[derive(Debug)]
pub(crate) struct Request {
    pub operation: Operation,
    pub limit: usize,
    pub reverse: bool,
    pub current: Option<String>,
    pub group: Option<String>,
    pub cursor: Option<Cursor>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all="camelCase")]
pub(crate) struct Cursor {
    version: u8,
    scope: String,
    pub anchor: String,
    witness: String,
    current_witness: Option<String>,
    group_witness: Option<String>,
}
impl Cursor {
    fn encode(&self) -> ApiResult<String> {
        let value = URL_SAFE_NO_PAD.encode(serde_json::to_vec(self).map_err(|_| ApiError::unavailable())?);
        if value.len() > MAX_CURSOR_BYTES { return Err(ApiError::unavailable()); }
        Ok(value)
    }
    fn decode(value: &str) -> ApiResult<Self> {
        if value.is_empty() || value.len() > MAX_CURSOR_BYTES { return Err(ApiError::bad_request("Invalid reader cursor")); }
        let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| ApiError::bad_request("Invalid reader cursor"))?;
        let cursor: Self = serde_json::from_slice(&bytes).map_err(|_| ApiError::bad_request("Invalid reader cursor"))?;
        let digest = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if cursor.version != 1 || cursor.anchor.is_empty() || cursor.anchor.len() > 128
            || !digest(&cursor.scope) || !digest(&cursor.witness)
            || cursor.current_witness.as_deref().is_some_and(|v| !digest(v))
            || cursor.group_witness.as_deref().is_some_and(|v| !digest(v))
            || cursor.encode()? != value {
            return Err(ApiError::bad_request("Invalid reader cursor"));
        }
        Ok(cursor)
    }
}
impl Request {
    /// Controls are removed before the profile's existing filter parser runs.
    /// No history, completion IDs, offsets or client-supplied sort keys exist.
    pub fn parse(raw: Option<&str>) -> ApiResult<(Self, String)> {
        let raw = raw.unwrap_or("");
        if raw.len() > 16_384 { return Err(ApiError::bad_request("Reader query is too long")); }
        validate_wire(raw)?;
        let mut controls = BTreeMap::new();
        let mut filters = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            if key.len() > 128 || value.len() > 4096 || key.contains('\0') || value.contains('\0') {
                return Err(ApiError::bad_request("Invalid reader query"));
            }
            if ["operation", "limit", "direction", "current", "groupChapter", "after"].contains(&key.as_ref()) {
                if controls.insert(key.into_owned(), value.into_owned()).is_some() {
                    return Err(ApiError::bad_request("Duplicate reader control"));
                }
            } else { filters.append_pair(&key, &value); }
        }
        let operation = Operation::parse(controls.get("operation").map(String::as_str).unwrap_or("groups"))?;
        let limit = controls.get("limit").map(|s| {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) { return Err(ApiError::bad_request("Invalid reader limit")); }
            s.parse::<usize>().ok().filter(|n| (1..=MAX_PAGE).contains(n)).ok_or_else(|| ApiError::bad_request("Reader limit must be 1..100"))
        }).transpose()?.unwrap_or(50);
        let reverse = match controls.get("direction").map(String::as_str) {
            None | Some("newest") => false, Some("oldest") if operation == Operation::Groups => true,
            _ => return Err(ApiError::bad_request("Direction applies only to group pages")),
        };
        if controls.contains_key("direction") && operation != Operation::Groups {
            return Err(ApiError::bad_request("Direction applies only to group pages"));
        }
        let current = controls.remove("current");
        let group = controls.remove("groupChapter");
        if current.as_ref().is_some_and(|id| id.is_empty() || id.len() > 128)
            || group.as_ref().is_some_and(|id| id.is_empty() || id.len() > 128) {
            return Err(ApiError::bad_request("Invalid reader reference"));
        }
        if matches!(operation, Operation::Around | Operation::Next | Operation::Previous) && current.is_none() {
            return Err(ApiError::bad_request("This operation requires the actual current chapter"));
        }
        if matches!(operation, Operation::Editions | Operation::ResolveGroup) && group.is_none() && current.is_none() {
            return Err(ApiError::bad_request("This operation requires a group chapter reference"));
        }
        if group.is_some() && !matches!(operation, Operation::Editions | Operation::ResolveGroup) {
            return Err(ApiError::bad_request("Group reference applies only to editions or resolve-group"));
        }
        let cursor = controls.get("after").map(|v| Cursor::decode(v)).transpose()?;
        if cursor.is_some() && !matches!(operation, Operation::Groups | Operation::Editions) {
            return Err(ApiError::bad_request("Semantic navigation is always freshly resolved"));
        }
        Ok((Self { operation, limit, reverse, current, group, cursor }, filters.finish()))
    }
    pub fn scope(&self, identity: &CatalogIdentity, profile: Profile, parent: &str, effective_filters: &str) -> String {
        let mut pairs = url::form_urlencoded::parse(effective_filters.as_bytes()).map(|(k,v)|(k.into_owned(),v.into_owned())).collect::<Vec<_>>();
        pairs.sort();
        digest(&json!([identity, profile_name(profile), parent, pairs, self.operation, self.reverse, self.current, self.group]))
    }
}
// Do not let URL's lossy percent decoder turn malformed public input into a
// different accepted filter. Native repeated filters remain legal; profile
// parsers own their vocabulary and repetition limits after this transport check.
pub(crate) fn validate_wire(raw: &str) -> ApiResult<()> {
    if raw.is_empty() { return Ok(()); }
    for pair in raw.split('&') {
        if pair.is_empty() { return Err(ApiError::bad_request("Empty reader query field")); }
        for value in pair.splitn(2,'=') {
            let bytes=value.as_bytes(); let mut decoded=Vec::with_capacity(bytes.len()); let mut i=0;
            while i<bytes.len() {
                if bytes[i]==b'%' {
                    let digit=|b:u8| (b as char).to_digit(16).map(|n|n as u8);
                    let high=bytes.get(i+1).and_then(|b|digit(*b)); let low=bytes.get(i+2).and_then(|b|digit(*b));
                    let (Some(high),Some(low))=(high,low) else {return Err(ApiError::bad_request("Malformed reader query encoding"));};
                    decoded.push(high*16+low);i+=3;
                } else {decoded.push(bytes[i]);i+=1;}
            }
            if decoded.contains(&0) || std::str::from_utf8(&decoded).is_err() {return Err(ApiError::bad_request("Invalid reader query text"));}
        }
    }
    Ok(())
}
fn digest(value: &Value) -> String { hex::encode(Sha256::digest(value.to_string().as_bytes())) }
fn profile_name(profile: Profile) -> &'static str {
    match profile { Profile::Native => "native-aggregate-v1", Profile::Public => "public-retained-v1" }
}
pub(crate) async fn deadline(tx: &Transaction<'_>) -> ApiResult<()> {
    tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL lock_timeout='250ms'; SET LOCAL idle_in_transaction_session_timeout='3s'; SET LOCAL TIME ZONE 'UTC'").await?;
    Ok(())
}

/// The same public instance identity advertised by protocol capabilities. This
/// identifies the catalogue, not its revision or an across-request snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct CatalogIdentity {
    catalog_namespace: String,
    service_key: Option<String>,
}
impl CatalogIdentity {
    /// Strict expected-instance admission. The explicit text `null` represents
    /// a native-only service; omission is never equivalent to that value.
    pub(crate) fn expected(namespace: &str, service_key: &str) -> ApiResult<Self> {
        let parsed = crate::public_api::PublicUuid::parse(namespace)
            .map_err(|_| ApiError::bad_request("Invalid catalogue namespace"))?;
        if parsed.as_str() != namespace {
            return Err(ApiError::bad_request("Catalogue namespace must be canonical"));
        }
        let service_key = if service_key == "null" { None }
            else if manga_node::auth::valid_hash(service_key) { Some(service_key.to_owned()) }
            else { return Err(ApiError::bad_request("Invalid service key")); };
        Ok(Self { catalog_namespace: namespace.to_owned(), service_key })
    }
}
pub(crate) async fn catalog_identity(tx: &Transaction<'_>, state: &AppState) -> ApiResult<CatalogIdentity> {
    catalog_identity_with_error(tx, state, ApiError::from).await
}
pub(crate) async fn native_continue_identity(tx: &Transaction<'_>, state: &AppState) -> ApiResult<CatalogIdentity> {
    catalog_identity_with_error(tx, state, native_continue_database_error).await
}
async fn catalog_identity_with_error(tx: &Transaction<'_>, state: &AppState, database_error: fn(tokio_postgres::Error) -> ApiError) -> ApiResult<CatalogIdentity> {
    // Establish/read the exact selector transaction's snapshot, never a second
    // pool connection or an earlier capabilities response. PostgreSQL UUID text
    // is canonical lowercase and bounded at 36 ASCII bytes.
    let catalog_namespace = tx.query_one(
        "SELECT namespace::text FROM platform.catalogue_namespace WHERE singleton", &[],
    ).await.map_err(database_error)?.get(0);
    // AppState's immutable config is also the existing capabilities authority.
    // Reuse its exact eligibility/validation, including native-only null keys.
    let (_, service_key, _) = crate::protocol::adapters::configured_service(state);
    Ok(CatalogIdentity { catalog_namespace, service_key })
}

#[derive(Clone, Debug)]
struct Chapter {
    native: String,
    reference: String,
    volume: Option<String>,
    chapter: Option<String>,
    language: String,
    published: String,
    version: i32,
    native_groups: Vec<String>,
    groups: Vec<String>,
    score: i64,
}
impl Chapter {
    fn witness(&self) -> String {
        // Public witnesses hash PUBLIC identities and public scalar metadata only.
        digest(&json!([self.reference,self.volume,self.chapter,self.language,self.published,self.version,self.groups]))
    }
    fn dto(&self) -> Value {
        json!({"id":self.reference,"volume":self.volume,"chapter":self.chapter,"language":self.language,
            "publishedAt":self.published,"version":self.version,"groupIds":self.groups,
            "sharedGroupScore":self.score,"witness":self.witness()})
    }
}
fn projection(profile: Profile, score: &str) -> String {
    let reference = match profile { Profile::Native => "c.id", Profile::Public => "(SELECT a.public_id::text FROM platform.public_resource_ids a WHERE a.kind='chapter' AND a.native_id=c.id AND a.preferred AND a.principal_type='native')" };
    let group_reference = match profile { Profile::Native => "g.id", Profile::Public => "(SELECT a.public_id::text FROM platform.public_resource_ids a WHERE a.kind='group' AND a.native_id=g.id AND a.preferred AND a.principal_type='native')" };
    // Group identities are bounded before transport; no metadata, names, members,
    // images or nested manga DTOs are expanded. Oversized real groups fail closed.
    format!("c.id,{reference},c.volume,c.chapter,c.language,c.published_at::text,c.version,COALESCE((SELECT jsonb_agg(jsonb_build_array(g.native,g.reference) ORDER BY g.native) FROM (SELECT g.id AS native,{group_reference} AS reference FROM platform.catalog_chapter_groups cg JOIN platform.catalog_groups g ON g.id=cg.group_id WHERE cg.chapter_id=c.id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id) ORDER BY g.id LIMIT 101) g),'[]'::jsonb),({score})::bigint")
}
fn chapter(row: tokio_postgres::Row, profile: Profile) -> ApiResult<Chapter> {
    let reference = row.get::<_, Option<String>>(1).ok_or_else(ApiError::unavailable)?;
    let rows: Vec<(String, Option<String>)> = serde_json::from_value(row.get(7)).map_err(|_| ApiError::unavailable())?;
    if rows.len() > MAX_GROUPS { return Err(ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE, "reader_resource_too_large", "Chapter group identities exceed the reader resource budget")); }
    let mut native_groups = Vec::new(); let mut groups = Vec::new();
    for (native, reference) in rows {
        let reference = reference.ok_or_else(ApiError::unavailable)?;
        validate_reference(profile, &reference)?;
        native_groups.push(native); groups.push(reference);
    }
    validate_reference(profile, &reference)?;
    let entry = Chapter { native:row.get(0),reference,volume:row.get(2),chapter:row.get(3),language:row.get(4),published:row.get(5),version:row.get(6),native_groups,groups,score:row.get(8) };
    if entry.volume.as_ref().is_some_and(|s| s.chars().count()>32) || entry.chapter.as_ref().is_some_and(|s| s.chars().count()>32) || entry.language.len()>48 || entry.published.len()>64 || entry.version<1 {
        return Err(ApiError::unavailable());
    }
    Ok(entry)
}
fn validate_reference(profile: Profile, reference: &str) -> ApiResult<()> {
    match profile {
        Profile::Native => validation::id(reference),
        Profile::Public => crate::public_api::PublicUuid::parse(reference).map(|_|()).map_err(|_| ApiError::unavailable()),
    }
}
async fn load(tx: &Transaction<'_>, profile: Profile, sql: &mut Sql, native: Option<&str>) -> ApiResult<Option<Chapter>> {
    let Some(native) = native else { return Ok(None); };
    let checkpoint = sql.parameters.len();
    let id = sql.bind(native.to_owned());
    let statement = format!("SELECT {} FROM {TABLE} WHERE {} AND c.id={id}", projection(profile,"0"), sql.predicate());
    let rows = tx.query_opt(&statement,&sql.params()).await;
    sql.parameters.truncate(checkpoint);
    rows?.map(|r|chapter(r,profile)).transpose()
}
fn group_equal(a: &str, b: &str) -> String {
    format!("{a}.volume IS NOT DISTINCT FROM {b}.volume AND {a}.chapter IS NOT DISTINCT FROM {b}.chapter")
}
fn anchor_sql(sql: &mut Sql, anchor: &Chapter) -> String {
    let volume = sql.bind(anchor.volume.clone()); let chapter = sql.bind(anchor.chapter.clone());
    let id = sql.bind(anchor.native.clone()); let published = sql.bind(anchor.published.clone());
    format!("(SELECT {volume}::text AS volume,{chapter}::text AS chapter,{id}::text AS id,{published}::text::timestamptz AS published_at) a")
}
fn score_sql(sql: &mut Sql, current: Option<&Chapter>) -> String {
    let groups = sql.bind(current.map(|c| c.native_groups.clone()).unwrap_or_default());
    // The current persisted DTO always has IDs. Legacy name fallback is only
    // relevant to missing-ID client records, which this API never accepts.
    format!("SELECT count(*) FROM platform.catalog_chapter_groups cg JOIN platform.catalog_groups g ON g.id=cg.group_id WHERE cg.chapter_id=c.id AND cg.group_id=ANY({groups}::text[]) AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)")
}
async fn group_page(tx: &Transaction<'_>, profile: Profile, sql: &mut Sql, current: Option<&Chapter>, boundary: Option<&Chapter>, reverse: bool, limit: usize) -> ApiResult<Vec<Chapter>> {
    let checkpoint = sql.parameters.len();
    let score = score_sql(sql,current);
    let predicate = sql.predicate();
    let bound = if let Some(anchor) = boundary {
        let anchor = anchor_sql(sql,anchor);
        format!(" AND EXISTS(SELECT 1 FROM {anchor} WHERE {})",reader_order::group_after(profile,"c","a",reverse))
    } else { String::new() };
    let limit = sql.bind(limit as i64);
    let order = reader_order::group_order(profile,"g",reverse);
    let editions = reader_order::edition_order(profile,"c");
    let same_group = group_equal("c","g");
    // DISTINCT positions may examine a large eligible set in PostgreSQL, under
    // the deadline, but only <=101 minimal positions/rows leave the statement.
    // Winner resolution deliberately has NO preliminary edition page limit.
    let statement = format!("WITH positions AS (SELECT g.* FROM (SELECT DISTINCT c.volume,c.chapter FROM {TABLE} WHERE {predicate}{bound}) g ORDER BY {order} LIMIT {limit}) SELECT {} FROM positions g CROSS JOIN LATERAL(SELECT c.* FROM {TABLE} WHERE {predicate} AND {same_group} ORDER BY ({score}) DESC,{editions} LIMIT 1)c ORDER BY {order}",projection(profile,&score));
    let rows = tx.query(&statement,&sql.params()).await;
    sql.parameters.truncate(checkpoint);
    rows?.into_iter().map(|r|chapter(r,profile)).collect()
}
async fn edition_page(tx: &Transaction<'_>, profile: Profile, sql: &mut Sql, current: Option<&Chapter>, group: &Chapter, boundary: Option<&Chapter>, winner: bool, limit: usize) -> ApiResult<Vec<Chapter>> {
    let checkpoint = sql.parameters.len();
    let score = score_sql(sql,current);
    let group = anchor_sql(sql,group);
    let same_group = group_equal("c","a");
    let bound = if let Some(anchor) = boundary {
        let id = sql.bind(anchor.native.clone()); let published = sql.bind(anchor.published.clone());
        let after = reader_order::edition_after(profile,"c","b");
        format!(" AND EXISTS(SELECT 1 FROM (SELECT {id}::text AS id,{published}::text::timestamptz AS published_at)b WHERE {after})")
    } else { String::new() };
    let limit = sql.bind(limit as i64);
    let order = reader_order::edition_order(profile,"c");
    let priority = if winner { format!("({score}) DESC,") } else { String::new() };
    let statement = format!("SELECT {} FROM {TABLE} CROSS JOIN {group} WHERE {} AND {same_group}{bound} ORDER BY {priority}{order} LIMIT {limit}",projection(profile,&score),sql.predicate());
    let rows = tx.query(&statement,&sql.params()).await;
    sql.parameters.truncate(checkpoint);
    rows?.into_iter().map(|r|chapter(r,profile)).collect()
}
fn envelope(identity: &CatalogIdentity, profile: Profile, request: &Request, scope: &str, parent: &str, state: &str) -> Value {
    json!({"version":1,"catalogIdentity":identity,"mangaId":parent,"operation":request.operation,"orderingProfile":profile_name(profile),"scope":scope,
        "consistency":"live-keyset-v1","state":state,"restartRequired":state=="stale-anchor",
        "data":[],"nextCursor":null,"limit":request.limit,
        "history":"device-local-only","continueProof":"observed-only","crossRequestSnapshot":false})
}
fn state_response(identity: &CatalogIdentity, profile: Profile, request: &Request, scope: &str, parent: &str, state: &str) -> Value { envelope(identity,profile,request,scope,parent,state) }

/// The caller owns parent visibility, exact profile filters, aliases, read
/// admission and one short repeatable-read transaction. SQL values stay bound.
pub(crate) async fn select(tx: &Transaction<'_>, identity: &CatalogIdentity, profile: Profile, request: &Request, scope: &str, parent: &str, sql: &mut Sql, current_native: Option<String>, anchor_native: Option<String>, group_native: Option<String>) -> ApiResult<Value> {
    let current = load(tx,profile,sql,current_native.as_deref()).await?;
    if request.current.is_some() && current.is_none() { return Ok(state_response(identity,profile,request,scope,parent,"excluded-current")); }
    let group = if request.group.is_some() { load(tx,profile,sql,group_native.as_deref()).await? } else { current.clone() };
    if request.group.is_some() && group.is_none() { return Ok(state_response(identity,profile,request,scope,parent,"stale-anchor")); }
    let anchor = if request.cursor.is_some() { load(tx,profile,sql,anchor_native.as_deref()).await? } else { None };
    if let Some(cursor) = &request.cursor {
        if cursor.scope != scope || anchor.as_ref().is_none_or(|a| a.witness()!=cursor.witness)
            || cursor.current_witness != current.as_ref().map(Chapter::witness)
            || cursor.group_witness != (if request.operation==Operation::Editions {group.as_ref().map(Chapter::witness)} else {None})
            || (request.operation==Operation::Editions && anchor.as_ref().zip(group.as_ref()).is_none_or(|(a,g)| a.volume!=g.volume || a.chapter!=g.chapter)) {
            return Ok(state_response(identity,profile,request,scope,parent,"stale-anchor"));
        }
    }
    let mut result = envelope(identity,profile,request,scope,parent,"ready");
    if let Some(current) = &current { result["current"] = current.dto(); }
    let mut rows = match request.operation {
        Operation::Groups => group_page(tx,profile,sql,current.as_ref(),anchor.as_ref(),request.reverse,request.limit+1).await?,
        Operation::Editions => edition_page(tx,profile,sql,current.as_ref(),group.as_ref().ok_or_else(ApiError::unavailable)?,anchor.as_ref(),false,request.limit+1).await?,
        Operation::ResolveGroup => edition_page(tx,profile,sql,current.as_ref(),group.as_ref().ok_or_else(ApiError::unavailable)?,None,true,1).await?,
        Operation::First => group_page(tx,profile,sql,current.as_ref(),None,true,1).await?,
        Operation::Last => group_page(tx,profile,sql,current.as_ref(),None,false,1).await?,
        Operation::Next => group_page(tx,profile,sql,current.as_ref(),current.as_ref(),true,1).await?,
        Operation::Previous => group_page(tx,profile,sql,current.as_ref(),current.as_ref(),false,1).await?,
        Operation::Around => {
            let current = current.as_ref().ok_or_else(ApiError::unavailable)?;
            let next = group_page(tx,profile,sql,Some(current),Some(current),true,1).await?;
            let previous = group_page(tx,profile,sql,Some(current),Some(current),false,1).await?;
            result["next"] = next.first().map(Chapter::dto).unwrap_or(Value::Null);
            result["previous"] = previous.first().map(Chapter::dto).unwrap_or(Value::Null);
            edition_page(tx,profile,sql,Some(current),current,None,true,1).await?
        }
    };
    if matches!(request.operation,Operation::Editions | Operation::ResolveGroup) {
        let group = group.as_ref().ok_or_else(ApiError::unavailable)?;
        // This independent top-one winner considers ALL eligible editions, even
        // when the winner falls beyond the requested edition page or 10K rows.
        let winner = edition_page(tx,profile,sql,current.as_ref(),group,None,true,1).await?;
        result["winner"] = winner.first().map(Chapter::dto).unwrap_or(Value::Null);
        let lookahead = edition_page(tx,profile,sql,None,group,None,false,2).await?;
        result["hasMultipleEditions"] = json!(lookahead.len()>1);
        result["groupAnchor"] = group.dto();
    }
    if matches!(request.operation,Operation::First | Operation::Last) {
        if let Some(selected) = rows.first() {
            let lookahead = edition_page(tx,profile,sql,None,selected,None,false,2).await?;
            result["hasMultipleEditions"] = json!(lookahead.len()>1);
        }
    }
    let paged = matches!(request.operation,Operation::Groups | Operation::Editions);
    let mut more = paged && rows.len()>request.limit;
    rows.truncate(request.limit);
    let mut seen = BTreeSet::new();
    for row in &rows {
        let key = if request.operation==Operation::Groups { json!([row.volume,row.chapter]).to_string() } else { row.reference.clone() };
        if !seen.insert(key) { return Err(ApiError::unavailable()); }
    }
    if rows.is_empty() { result["state"]=json!("boundary"); }
    loop {
        result["data"] = Value::Array(rows.iter().map(Chapter::dto).collect());
        result["nextCursor"] = if more {
            let last = rows.last().ok_or_else(ApiError::unavailable)?;
            json!(Cursor { version:1,scope:scope.into(),anchor:last.reference.clone(),witness:last.witness(),current_witness:current.as_ref().map(Chapter::witness),group_witness:if request.operation==Operation::Editions {group.as_ref().map(Chapter::witness)} else {None} }.encode()?)
        } else { Value::Null };
        if serde_json::to_vec(&result).map_err(|_|ApiError::unavailable())?.len()<=MAX_RESPONSE_BYTES { break; }
        if !paged || rows.len()<=1 { return Err(ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE,"reader_resource_too_large","Complete reader resource exceeds the response budget")); }
        rows.pop(); more=true;
    }
    Ok(result)
}

/// Preserve authority/snapshot conflict semantics on the Continue query and
/// commit as well as in the shared native principal's NOWAIT lock statements.
pub(crate) fn native_continue_database_error(error: tokio_postgres::Error) -> ApiError {
    if error.code().is_some_and(|code| matches!(code.code(), "55P03" | "40P01" | "40001")) {
        ApiError::conflict("Account or catalogue snapshot changed or is busy; refresh before continuing")
    } else { error.into() }
}

fn native_continue_statement(sql: &mut Sql, account: &str) -> String {
    let account = sql.bind(account.to_owned());
    let predicate = sql.predicate();
    let oldest = reader_order::group_order(Profile::Native, "g", true);
    let newest = reader_order::group_order(Profile::Native, "g", false);
    let editions = reader_order::edition_order(Profile::Native, "c");
    let same_group = group_equal("c", "g");
    let probe_group = group_equal("e", "g");
    // eligible is the COMPLETE eligible chapter set. History membership of ANY
    // eligible edition completes that exact nullable-label group. Never cap
    // eligible/positions, join foreign history, or substitute progress.completed.
    // Both choice branches rank the complete positions; only their final answer
    // is limited. The independent two-row probe does not constrain the winner.
    // This bounds result cardinality, NOT SQL work or materialization cost.
    format!("WITH eligible AS MATERIALIZED (\
        SELECT c.id,c.volume,c.chapter,c.language,c.published_at,c.version,EXISTS(SELECT 1 FROM platform.library_chapter_history h \
            WHERE h.user_id={account} AND h.manga_id=m.id AND h.chapter_id=c.id) AS completed \
        FROM {TABLE} WHERE {predicate}), \
        positions AS MATERIALIZED (SELECT volume,chapter,bool_or(completed) AS completed FROM eligible GROUP BY volume,chapter), \
        oldest_unread AS (SELECT g.volume,g.chapter,false AS all_completed FROM positions g WHERE NOT g.completed ORDER BY {oldest} LIMIT 1), \
        selected AS (SELECT * FROM oldest_unread UNION ALL \
            (SELECT g.volume,g.chapter,true AS all_completed FROM positions g WHERE NOT EXISTS(SELECT 1 FROM oldest_unread) ORDER BY {newest} LIMIT 1)) \
        SELECT {},g.all_completed, \
            (SELECT count(*)>1 FROM (SELECT 1 FROM eligible e WHERE {probe_group} LIMIT 2) probe) \
        FROM selected g CROSS JOIN LATERAL \
            (SELECT c.* FROM eligible c WHERE {same_group} ORDER BY {editions} LIMIT 1)c",
        projection(Profile::Native, "0"))
}

const MAX_CONTINUE_RESPONSE_BYTES: usize = 32 * 1024;
fn continue_envelope(
    identity: &CatalogIdentity, parent: &str, account: &str, epoch: u64,
    request_id: &str, filters: &QueryParams, winner: Option<&Chapter>,
    all_completed: bool, multiple: bool,
) -> ApiResult<Value> {
    // Native array filters are sets for eligibility. Canonicalize only those
    // query values, never chapter labels, catalogue order or edition identity.
    let mut effective = filters.values.clone();
    for values in effective.values_mut() { values.sort(); values.dedup(); }
    let scope = digest(&json!([identity, "native-aggregate-v1", "native-continue", parent, account, effective]));
    let reason = if winner.is_none() { "empty" }
        else if all_completed { "all-completed-newest" } else { "oldest-unread" };
    let value = json!({
        "version": 1, "operation": "native-continue", "orderingProfile": "native-aggregate-v1",
        "mangaId": parent, "catalogIdentity": identity, "account": {"id":account,"epoch":epoch},
        "requestId": request_id, "scope": scope, "consistency": "live-request-v1",
        "continueProof": "native-account-snapshot-v1", "crossRequestSnapshot": false,
        "state": if winner.is_some() { "ready" } else { "boundary" }, "reason": reason,
        "target": winner.map(|c| json!({"volume":c.volume,"chapter":c.chapter,"anchorChapterId":c.reference})),
        "winner": winner.map(Chapter::dto), "hasMultipleEditions": multiple,
    });
    if serde_json::to_vec(&value).map_err(|_| ApiError::unavailable())?.len() > MAX_CONTINUE_RESPONSE_BYTES {
        return Err(ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "reader_resource_too_large", "Complete Continue resource exceeds the response budget"));
    }
    Ok(value)
}

/// The library route owns principal locking, parent visibility and the same
/// short RepeatableRead transaction. One minimal winner leaves PostgreSQL;
/// no aggregate pages, history map, counts or intermediate labels leave it.
pub(crate) async fn native_continue(
    tx: &Transaction<'_>, identity: &CatalogIdentity, parent: &str, account: &str,
    epoch: u64, request_id: &str, filters: &QueryParams, sql: &mut Sql,
) -> ApiResult<Value> {
    let checkpoint = sql.parameters.len();
    let statement = native_continue_statement(sql, account);
    let row = tx.query_opt(&statement, &sql.params()).await;
    sql.parameters.truncate(checkpoint);
    let row = row.map_err(native_continue_database_error)?;
    let (winner, all_completed, multiple) = match row {
        Some(row) => {
            let all_completed: bool = row.get(9);
            let multiple: bool = row.get(10);
            (Some(chapter(row, Profile::Native)?), all_completed, multiple)
        }
        None => (None, false, false),
    };
    continue_envelope(identity, parent, account, epoch, request_id, filters, winner.as_ref(), all_completed, multiple)
}

pub async fn native(State(state): State<AppState>,Path(id): Path<String>,RawQuery(raw): RawQuery) -> ApiResult<(HeaderMap,Json<Value>)> {
    tokio::time::timeout(Duration::from_secs(8),native_in(state,id,raw)).await.map_err(|_|ApiError::unavailable())?
}
async fn native_in(state: AppState,id:String,raw:Option<String>) -> ApiResult<(HeaderMap,Json<Value>)> {
    validation::id(&id)?;
    let (request, filters) = Request::parse(raw.as_deref())?;
    // Native aggregate applies only chapter filters. Do not accidentally inherit
    // public defaults (availability, empty pages, content rating or future flags).
    let q = QueryParams::parse(Some(filters.clone()))?;
    let allowed = ["tlang","tlangx","uploader","uploaderx","group","groupx","id","manga"];
    if q.values.keys().any(|k| !allowed.contains(&k.as_str())) { return Err(ApiError::bad_request("Unknown native reader filter")); }
    for reference in request.current.iter().chain(request.group.iter()).chain(request.cursor.iter().map(|c|&c.anchor)) { validation::id(reference)?; }
    // router::admitted owns the Catalogue permit across this handler.
    // A second acquisition would deadlock/refuse a valid single-slot config.
    let mut db = state.db.client().await?;
    let tx = db.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead).read_only(true).start().await?;
    deadline(&tx).await?;
    let identity = catalog_identity(&tx,&state).await?;
    let mut parent = Sql::default();
    parent.conditions.push("m.deleted_at IS NULL AND m.state='published'".into());
    manga_filters(&QueryParams {values:BTreeMap::new(),limit:1,page:1},&mut parent,"m",None)?;
    let parent_id=parent.bind(id.clone());
    if tx.query_opt(&format!("SELECT m.id FROM platform.catalog_manga m WHERE {} AND m.id={parent_id}",parent.predicate()),&parent.params()).await?.is_none() { return Err(ApiError::not_found("Manga not found")); }
    let mut sql = parent;
    sql.conditions.push(format!("c.manga_id={parent_id} AND c.deleted_at IS NULL"));
    chapter_filters(&q,&mut sql,"c")?;
    let scope=request.scope(&identity,Profile::Native,&id,&filters);
    let result=select(&tx,&identity,Profile::Native,&request,&scope,&id,&mut sql,request.current.clone(),request.cursor.as_ref().map(|c|c.anchor.clone()),request.group.clone()).await?;
    tx.commit().await?;
    let mut headers=HeaderMap::new();headers.insert(CACHE_CONTROL,HeaderValue::from_static("no-store"));
    Ok((headers,Json(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_continue_statement_never_caps_eligible_groups_or_editions() {
        let mut sql = Sql::default();
        let parent = sql.bind("m".to_owned());
        sql.conditions.push(format!("m.id={parent}"));
        let query = native_continue_statement(&mut sql, "do-not-interpolate-account");
        assert!(!query.contains("do-not-interpolate-account"));
        assert!(query.contains("h.user_id=$2 AND h.manga_id=m.id AND h.chapter_id=c.id"));
        assert!(query.contains("bool_or(completed) AS completed FROM eligible GROUP BY volume,chapter"));
        assert!(query.contains("WHERE NOT g.completed ORDER BY"));
        assert!(query.contains("WHERE NOT EXISTS(SELECT 1 FROM oldest_unread) ORDER BY"));
        assert!(query.contains("c.volume IS NOT DISTINCT FROM g.volume AND c.chapter IS NOT DISTINCT FROM g.chapter"));
        assert!(query.contains("e.volume IS NOT DISTINCT FROM g.volume AND e.chapter IS NOT DISTINCT FROM g.chapter LIMIT 2"));
        assert!(!query.contains("library_chapter_progress"));
        assert!(!query.contains("OFFSET") && !query.contains("10000") && !query.contains("10001"));
        let eligible = query.split("positions AS MATERIALIZED").next().unwrap();
        assert!(!eligible.contains("LIMIT"));
        assert!(query.contains(&reader_order::group_order(Profile::Native, "g", true)));
        assert!(query.contains(&reader_order::group_order(Profile::Native, "g", false)));
        assert!(query.contains(&reader_order::edition_order(Profile::Native, "c")));
    }
    #[test]
    fn native_continue_envelope_has_fixed_bytes_scoped_identity_and_exact_nullable_target() {
        let identity = CatalogIdentity { catalog_namespace:"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(), service_key:None };
        let filters = QueryParams::parse(Some("tlang=en&group=g".into())).unwrap();
        let row = Chapter { native:"c".into(), reference:"c".into(), volume:None, chapter:Some("".into()), language:"en".into(), published:"2026-10-22 12:00:00.000001+00".into(), version:1, native_groups:vec![], groups:vec![], score:0 };
        let ready = continue_envelope(&identity,"m","account",7,"click",&filters,Some(&row),false,true).unwrap();
        assert_eq!(ready["target"], json!({"volume":null,"chapter":"","anchorChapterId":"c"}));
        assert_eq!(ready["reason"], "oldest-unread");
        assert_eq!(ready["continueProof"], "native-account-snapshot-v1");
        assert_eq!(ready["hasMultipleEditions"], true);
        let empty = continue_envelope(&identity,"m","account",7,"click",&filters,None,false,false).unwrap();
        assert_eq!(empty["state"], "boundary"); assert_eq!(empty["reason"], "empty");
        assert!(empty["target"].is_null() && empty["winner"].is_null());
        let all = continue_envelope(&identity,"m","account",8,"later",&filters,Some(&row),true,false).unwrap();
        assert_eq!(all["reason"], "all-completed-newest");
        assert_eq!(all["scope"], ready["scope"], "Generations and history are not scope revisions");
        let reordered = QueryParams::parse(Some("group[]=g&tlang=en&tlang=en".into())).unwrap();
        assert_eq!(ready["scope"], continue_envelope(&identity,"m","account",7,"click",&reordered,Some(&row),false,false).unwrap()["scope"]);
        assert_ne!(ready["scope"], continue_envelope(&identity,"m","other",7,"click",&filters,Some(&row),false,false).unwrap()["scope"]);
        let mut other_identity = identity.clone(); other_identity.service_key = Some("a".repeat(64));
        assert_ne!(ready["scope"], continue_envelope(&other_identity,"m","account",7,"click",&filters,Some(&row),false,false).unwrap()["scope"]);
        let mut oversized = row.clone(); oversized.groups = vec!["g".repeat(MAX_CONTINUE_RESPONSE_BYTES)];
        assert!(continue_envelope(&identity,"m","account",7,"click",&filters,Some(&oversized),false,false).is_err());
        let mut maximum = row.clone(); maximum.reference = ":".repeat(128);
        maximum.volume = Some("\"".repeat(32)); maximum.chapter = Some("\"".repeat(32));
        maximum.groups = (0..100).map(|n| format!("{n:03}{}", ":".repeat(125))).collect();
        let value = continue_envelope(&identity,&":".repeat(128),&":".repeat(128),9_007_199_254_740_991,&"r".repeat(64),&filters,Some(&maximum),true,true).unwrap();
        assert_eq!(value["winner"]["groupIds"].as_array().unwrap().len(),100);
        assert!(serde_json::to_vec(&value).unwrap().len() <= MAX_CONTINUE_RESPONSE_BYTES);
    }
    #[test]
    fn request_controls_are_bounded_and_semantic_actions_cannot_reuse_cursors() {
        assert_eq!(Request::parse(None).unwrap().0.limit,50);
        for raw in ["limit=0","limit=101","limit=1&limit=2","operation=next","operation=around","operation=editions","operation=resolve-group","current=","direction=sideways","operation=next&current=c&direction=newest","operation=groups&groupChapter=c","after=","title=%FF","title=%Q1","title=a&&title=b","title=%00"] {
            assert!(Request::parse(Some(raw)).is_err(),"{raw}");
        }
        assert!(Request::parse(Some("operation=editions&groupChapter=g&current=c")).is_ok());
        assert!(Request::parse(Some("operation=first")).is_ok());
    }
    #[test]
    fn cursor_round_trip_is_canonical_and_only_has_public_references() {
        let id="11111111-1111-8111-8111-111111111111";
        let cursor=Cursor {version:1,scope:"a".repeat(64),anchor:id.into(),witness:"b".repeat(64),current_witness:None,group_witness:None};
        let encoded=cursor.encode().unwrap();assert_eq!(Cursor::decode(&encoded).unwrap().anchor,id);
        assert!(Cursor::decode(&(encoded.clone()+"=")).is_err());
        assert!(!String::from_utf8(URL_SAFE_NO_PAD.decode(encoded).unwrap()).unwrap().contains("native"));
    }
    #[test]
    fn scope_fences_profile_filters_parent_current_group_operation_and_direction_not_page_size() {
        let identity=CatalogIdentity {catalog_namespace:"11111111-1111-4111-8111-111111111111".into(),service_key:None};
        let (a,_)=Request::parse(Some("current=c&limit=1")).unwrap();
        let (b,_)=Request::parse(Some("current=c&limit=100")).unwrap();
        let s=a.scope(&identity,Profile::Native,"m","tlang=en&group=g");
        assert_eq!(s,b.scope(&identity,Profile::Native,"m","group=g&tlang=en"));
        assert_ne!(s,b.scope(&identity,Profile::Public,"m","group=g&tlang=en"));
        assert_ne!(s,b.scope(&identity,Profile::Native,"n","group=g&tlang=en"));
        for raw in ["current=d","current=c&direction=oldest","operation=editions&current=c&groupChapter=g"] {
            assert_ne!(s,Request::parse(Some(raw)).unwrap().0.scope(&identity,Profile::Native,"m","tlang=en&group=g"));
        }
    }
    #[test]
    fn catalogue_identity_fences_same_ids_and_is_present_on_every_selector_state() {
        let request=Request::parse(None).unwrap().0;
        let original=CatalogIdentity {catalog_namespace:"11111111-1111-4111-8111-111111111111".into(),service_key:Some("a".repeat(64))};
        for profile in [Profile::Native,Profile::Public] {
            let scope=request.scope(&original,profile,"same-parent","tlang=en");
            let mut replacement=original.clone();replacement.catalog_namespace="22222222-2222-4222-8222-222222222222".into();
            assert_ne!(scope,request.scope(&replacement,profile,"same-parent","tlang=en"));
            replacement=original.clone();replacement.service_key=Some("b".repeat(64));
            assert_ne!(scope,request.scope(&replacement,profile,"same-parent","tlang=en"));
            replacement.service_key=None;
            assert_ne!(scope,request.scope(&replacement,profile,"same-parent","tlang=en"));
            for identity in [&original,&replacement] {
                for state in ["ready","boundary","stale-anchor","excluded-current"] {
                    let value=state_response(identity,profile,&request,&scope,"same-parent",state);
                    assert_eq!(value["catalogIdentity"],json!({"catalogNamespace":identity.catalog_namespace,"serviceKey":identity.service_key}));
                    assert_eq!(value["catalogIdentity"].as_object().unwrap().len(),2);
                    assert_eq!(value["crossRequestSnapshot"],false);
                    assert_eq!(value["continueProof"],"observed-only");
                }
            }
        }
    }
    #[test]
    fn witnesses_preserve_nullable_exact_labels_sub_ms_dates_and_group_identity() {
        let row=Chapter {native:"secret-native".into(),reference:"public".into(),volume:None,chapter:Some("9007199254740993".into()),language:"en".into(),published:"2026-10-22 12:00:00.000001+00".into(),version:1,native_groups:vec!["secret-group".into()],groups:vec!["public-group".into()],score:0};
        let witness=row.witness();let mut changed=row.clone();changed.native="another-native".into();changed.native_groups=vec!["another-secret".into()];assert_eq!(witness,changed.witness());
        changed.volume=Some("".into());assert_ne!(witness,changed.witness());
        changed=row.clone();changed.groups.clear();assert_ne!(witness,changed.witness());
        assert!(!row.dto().to_string().contains("secret"));
    }
}
