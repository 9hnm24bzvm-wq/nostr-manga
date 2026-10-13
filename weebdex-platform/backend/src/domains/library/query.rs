use super::validation;
use crate::{
    domains::catalog::query::QueryParams,
    error::{ApiError, ApiResult},
};
pub fn parse(raw: Option<String>) -> ApiResult<QueryParams> {
    let q = QueryParams::parse(raw)?;
    for key in [
        "limit",
        "page",
        "sort",
        "order",
        "title",
        "name",
        "username",
        "yearFrom",
        "yearTo",
        "tmod",
        "txmod",
        "hasChapters",
    ] {
        if q.values.get(key).is_some_and(|v| v.len() > 1) {
            return Err(ApiError::bad_request(format!("{key} must occur once")));
        }
    }
    for key in [
        "lang",
        "langx",
        "tlang",
        "tlangx",
        "availableTranslatedLang",
    ] {
        for language in q.list(key) {
            crate::domains::catalog::validation::language(&language)?;
        }
    }
    for key in [
        "id",
        "tag",
        "tagx",
        "author",
        "artist",
        "artists",
        "manga",
        "uploader",
        "uploaderx",
        "group",
        "groupx",
    ] {
        validation::ids(&q.list(key), 100)?;
    }
    for (key, allowed) in [
        (
            "reading_status",
            &[
                "reading",
                "plan_to_read",
                "completed",
                "on_hold",
                "re_reading",
                "dropped",
            ][..],
        ),
        (
            "status",
            &["ongoing", "completed", "hiatus", "cancelled"][..],
        ),
        (
            "contentRating",
            &["safe", "suggestive", "erotica", "pornographic", "__none__"][..],
        ),
        (
            "demographic",
            &["shounen", "shoujo", "josei", "seinen", "none"][..],
        ),
    ] {
        if q.list(key).iter().any(|v| !allowed.contains(&v.as_str())) {
            return Err(ApiError::bad_request(format!("Invalid {key}")));
        }
    }
    Ok(q)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_filter_values_survive_and_singular_values_fail_closed() {
        let q = parse(Some(
            "lang=en&lang=ja&reading_status=reading&reading_status=completed&limit=0&page=2".into(),
        ))
        .unwrap();
        assert_eq!(q.list("lang"), vec!["en", "ja"]);
        assert_eq!(q.limit, 0);
        assert_eq!(q.page, 2);
        assert!(parse(Some("page=1&page=2".into())).is_err());
        assert!(parse(Some("reading_status=unrecognized".into())).is_err());
        assert!(parse(Some("lang=en-US".into())).is_err());
    }
}
