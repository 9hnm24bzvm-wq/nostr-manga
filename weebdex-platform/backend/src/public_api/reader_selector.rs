//! Bounded public reader adapter. Public aliases identify resources, but never
//! establish their visibility: the shared selector rechecks every reference
//! against the same eligible set as the returned page.
use super::{
    PostgresPublicIdResolver, PublicApiError, PublicApiResult, PublicIdTarget, PublicUuid,
    ResourceKind, catalogue, query::PublicQuery,
};
use crate::{
    AppState,
    domains::catalog::{
        query::Sql,
        reader_order::Profile,
        reader_selector::{Request, catalog_identity, deadline, select},
    },
};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    http::header::CACHE_CONTROL,
    response::IntoResponse,
    routing::get,
};
use serde_json::Value;
use std::time::Duration;

pub fn router() -> Router<AppState> {
    Router::new().route("/manga/{p0}/reader", get(reader))
}

async fn reader(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> PublicApiResult<impl IntoResponse> {
    // router::admitted owns the Catalogue permit through this handler. Do not
    // acquire it again: a configured one-slot class would reject every read.
    // Bound pool acquisition and all alias/filter SQL, not just one statement.
    let value = tokio::time::timeout(Duration::from_secs(8), read(state, id, raw))
        .await
        .map_err(|_| PublicApiError::unavailable())??;
    Ok(([(CACHE_CONTROL, "no-store")], Json(value)))
}

async fn read(state: AppState, id: String, raw: Option<String>) -> PublicApiResult<Value> {
    let parent_public = PublicUuid::parse(&id)?;
    let (request, filter_wire) = Request::parse(raw.as_deref())?;
    let (query, effective_filter_wire) = reader_filters(&filter_wire)?;
    let current_public = request.current.as_deref().map(PublicUuid::parse).transpose()?;
    let anchor_public = request.cursor.as_ref().map(|cursor| PublicUuid::parse(&cursor.anchor))
        .transpose()?;
    let group_public = request.group.as_deref().map(PublicUuid::parse).transpose()?;

    let mut client = state.db.client().await?;
    let tx = client.build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true).start().await?;
    deadline(&tx).await?;
    let identity = catalog_identity(&tx,&state).await?;

    let parents = PostgresPublicIdResolver::resolve_many_in(
        &*tx, ResourceKind::Manga, std::slice::from_ref(&parent_public),
    ).await?;
    let parent = parents.get(&parent_public)
        .filter(|resolved| resolved.target == PublicIdTarget::NativeResource)
        .ok_or_else(PublicApiError::not_found)?;
    let visibility = catalogue::parts(ResourceKind::Manga)?.3;
    let visible = tx.query_opt(
        &format!("SELECT m.id FROM platform.catalog_manga m WHERE m.id=$1 AND {visibility}"),
        &[&parent.native_id],
    ).await?;
    if visible.is_none() {
        return Err(PublicApiError::not_found());
    }

    // Resolving an alias is deliberately not a chapter read. A missing or
    // excluded current/anchor/group is classified by the shared selector;
    // neither this adapter nor the identity table can admit an invisible row.
    let references = current_public.iter().chain(anchor_public.iter()).chain(group_public.iter())
        .cloned().collect::<Vec<_>>();
    let chapters = PostgresPublicIdResolver::resolve_many_in(
        &*tx, ResourceKind::Chapter, &references,
    ).await?;
    let native = |public: Option<&PublicUuid>| {
        public.and_then(|public| chapters.get(public))
            .filter(|resolved| resolved.target == PublicIdTarget::NativeResource)
            .map(|resolved| resolved.native_id.clone())
    };
    let current_native = native(current_public.as_ref());
    let anchor_native = native(anchor_public.as_ref());
    let group_native = native(group_public.as_ref());

    let mut sql = Sql::default();
    catalogue::filters(
        &state, Some(&tx), ResourceKind::Chapter, &query, &mut sql,
        Some(std::slice::from_ref(&parent.native_id)), None,
    ).await?;
    let parent_reference = parent.preferred_id.as_str();
    let scope = request.scope(&identity,Profile::Public, parent_reference, &effective_filter_wire);
    let value = select(
        &tx, &identity, Profile::Public, &request, &scope, parent_reference, &mut sql,
        current_native, anchor_native, group_native,
    ).await?;
    tx.commit().await?;
    Ok(value)
}

/// Keep the existing public chapter vocabulary and strict transport parser.
/// Reader controls were removed by Request::parse; list pagination, relationship
/// includes, caller ordering and a second parent scope are not reader filters.
/// The canonical wire describes the effective filters, including reader
/// defaults, so equivalent spelling/order cannot silently change cursor scope.
pub(super) fn reader_filters(raw: &str) -> PublicApiResult<(PublicQuery, String)> {
    let mut schema = catalogue::schema(ResourceKind::Chapter);
    schema.includes = &[];
    schema.order = &[];
    let parsed = PublicQuery::parse(Some(raw), &schema)?;
    let mut pairs = Vec::new();
    for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
        if matches!(key.as_ref(), "limit" | "offset" | "includes" | "includes[]" | "manga")
            || key.starts_with("order[")
        {
            return Err(PublicApiError::bad_request(
                "Reader filters cannot change pagination, ordering, includes or manga scope",
            ));
        }
        let key = if key == "chapter" { "chapter[]".to_owned() } else { key.into_owned() };
        let value = match key.as_str() {
            "ids[]" | "groups[]" | "uploader" | "excludedGroups[]" | "excludedUploaders[]" =>
                PublicUuid::parse(&value)?.to_string(),
            "externalUrl" | "excludeExternalUrl" => value.to_ascii_lowercase(),
            _ => value.into_owned(),
        };
        pairs.push((key, value));
    }
    if parsed.list("contentRating[]").is_empty() {
        for rating in ["safe", "suggestive", "erotica", "pornographic"] {
            pairs.push(("contentRating[]".to_owned(), rating.to_owned()));
        }
    }
    // Match the retained reader aggregate's permissive presentation defaults,
    // while preserving the catalogue's exclusion of unavailable chapters.
    for (key, default) in [
        ("includeEmptyPages", "1"),
        ("includeFuturePublishAt", "1"),
        ("includeFutureUpdates", "1"),
        ("includeExternalUrl", "1"),
        ("includeUnavailable", "0"),
    ] {
        match parsed.first(key) {
            None => pairs.push((key.to_owned(), default.to_owned())),
            Some("0" | "1") => {},
            Some(_) => return Err(PublicApiError::bad_request("Expected toggle0 or1")),
        }
    }
    // All retained array predicates are set-valued. Scalar duplicates have
    // already failed PublicQuery's strict parsing before this normalization.
    pairs.sort();
    pairs.dedup();
    let mut wire = url::form_urlencoded::Serializer::new(String::new());
    wire.extend_pairs(pairs.iter().map(|(key, value)| (key.as_str(), value.as_str())));
    let wire = wire.finish();
    // Reparse to apply the normal combined array/byte bounds after defaults
    // and the synonymous chapter/chapter[] fields have been normalized.
    let query = PublicQuery::parse(Some(&wire), &schema)?;
    Ok((query, wire))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_defaults_preserve_all_content_and_available_only() {
        let (query, wire) = reader_filters("").unwrap();
        assert_eq!(query.list("contentRating[]"), vec!["erotica", "pornographic", "safe", "suggestive"]);
        for key in ["includeEmptyPages", "includeFuturePublishAt", "includeFutureUpdates", "includeExternalUrl"] {
            assert_eq!(query.first(key), Some("1"));
        }
        assert_eq!(query.first("includeUnavailable"), Some("0"));
        assert_eq!(reader_filters(&wire).unwrap().1, wire);
    }

    #[test]
    fn explicit_filters_override_defaults_without_overriding_scope() {
        let (query, _) = reader_filters(
            "contentRating[]=safe&includeEmptyPages=0&includeFuturePublishAt=0&includeFutureUpdates=0&includeExternalUrl=0&includeUnavailable=1",
        ).unwrap();
        assert_eq!(query.list("contentRating[]"), vec!["safe"]);
        assert_eq!(query.first("includeEmptyPages"), Some("0"));
        assert_eq!(query.first("includeUnavailable"), Some("1"));
        for raw in [
            "offset=0", "limit=1", "manga=00000000-0000-0000-0000-000000000001",
            "includes[]=manga", "includes=manga", "order[chapter]=asc",
            "history=chapter", "includeUnavailable=true", "includeEmptyPages=2",
        ] {
            assert!(reader_filters(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn equivalent_sets_have_identical_effective_wire() {
        let (_, first) = reader_filters(
            "chapter=2&chapter[]=1&chapter=1&translatedLanguage[]=en&externalUrl=EXAMPLE.COM&groups[]=00000000-0000-0000-0000-00000000000A",
        ).unwrap();
        let (_, second) = reader_filters(
            "groups[]=00000000-0000-0000-0000-00000000000a&externalUrl=example.com&translatedLanguage[]=en&chapter[]=1&chapter[]=2",
        ).unwrap();
        assert_eq!(first, second);
        assert_ne!(first, reader_filters("chapter[]=3").unwrap().1);
    }

    #[test]
    fn invalid_transport_and_native_filter_ids_are_rejected_before_normalization() {
        for raw in [
            "title=%Q1", "title=%FF", "title=%00", "title=a&title=b", "title=a&&title=b",
            "groups[]=native-chapter", "ids[]=native-chapter", "uploader=native-user",
            "excludedGroups[]=native-group", "excludedUploaders[]=native-user",
        ] {
            assert!(reader_filters(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn group_chapter_control_does_not_replace_scanlation_group_filter() {
        let chapter = "00000000-0000-0000-0000-000000000001";
        let scanlation_group = "00000000-0000-0000-0000-000000000002";
        let raw = format!(
            "operation=editions&limit=7&groupChapter={chapter}&groups[]={scanlation_group}",
        );
        let (request, raw_filters) = Request::parse(Some(&raw)).unwrap();
        assert_eq!(request.group.as_deref(), Some(chapter));
        assert_eq!(request.limit, 7);
        let (query, wire) = reader_filters(&raw_filters).unwrap();
        assert_eq!(query.list("groups[]"), vec![scanlation_group]);
        assert!(!wire.contains("groupChapter"));
        assert!(!wire.contains("operation"));
        assert!(!wire.contains("limit"));
    }
}
