//! Shared author-visible feedback for every local catalogue review transport.
use crate::error::{ApiError, ApiResult};
use serde_json::{Value, json};
use tokio_postgres::Transaction;

const FIELDS: &[&str] = &[
    "title",
    "description",
    "language",
    "cover",
    "authors",
    "artists",
    "tags",
    "relations",
    "links",
    "rating",
    "status",
    "year",
    "other",
];
/// Remove feedback before passing the unchanged version-only core command.
pub(crate) fn take_feedback(input: &mut Value, next: &str) -> ApiResult<(String, Vec<String>)> {
    let object = input
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("Expected review command"))?;
    let reason = object.remove("reason");
    let fields = object.remove("affectedFields");
    if next != "rejected" {
        if reason.is_some() || fields.is_some() {
            return Err(ApiError::bad_request("Feedback belongs to a rejection"));
        }
        return Ok((String::new(), Vec::new()));
    }
    let reason = reason
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().count() <= 2000 && !s.contains('\0'))
        .ok_or_else(|| {
            ApiError::bad_request("Explain what the author needs to change (1–2000 characters)")
        })?
        .to_owned();
    let mut out = Vec::new();
    if let Some(fields) = fields {
        let fields = fields
            .as_array()
            .filter(|v| v.len() <= 16)
            .ok_or_else(|| ApiError::bad_request("Invalid affected fields"))?;
        for field in fields {
            let field = field
                .as_str()
                .filter(|v| FIELDS.contains(v))
                .ok_or_else(|| ApiError::bad_request("Unknown affected field"))?;
            if out.iter().any(|v| v == field) {
                return Err(ApiError::bad_request("Duplicate affected field"));
            }
            out.push(field.to_owned());
        }
    }
    Ok((reason, out))
}
pub(crate) async fn record(
    tx: &Transaction<'_>,
    manga: &str,
    actor: &str,
    version: i32,
    action: &str,
    reason: &str,
    fields: &[String],
) -> ApiResult<()> {
    // An exact accepted publication replay must not append another review.
    tx.execute("INSERT INTO platform.native_draft_reviews(manga_id,actor_id,version,action,reason,affected_fields) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(manga_id,version) DO NOTHING", &[&manga,&actor,&version,&action,&reason,&fields]).await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejection_requires_author_feedback_and_known_fields() {
        for reason in [
            json!(null),
            json!(""),
            json!("   "),
            json!("x".repeat(2001)),
        ] {
            assert!(take_feedback(&mut json!({"version":1,"reason":reason}), "rejected").is_err());
        }
        let mut value = json!({"version":3,"reason":" Fix the cover ","affectedFields":["cover"]});
        assert_eq!(
            take_feedback(&mut value, "rejected").unwrap(),
            ("Fix the cover".into(), vec!["cover".into()])
        );
        assert_eq!(value, json!({"version":3}));
        assert!(
            take_feedback(
                &mut json!({"reason":"x","affectedFields":["privateNotes"]}),
                "rejected"
            )
            .is_err()
        );
        assert!(take_feedback(&mut json!({"reason":"x"}), "submitted").is_err());
    }
}
