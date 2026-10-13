//! Bounded, local-only entity previews consumed by the retained forum UI.
//! SVG contains text and fixed primitives only; no scripts, URLs or external assets.
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use manga_node::read_admission::Class;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub fn router() -> Router<AppState> {
    Router::new().route("/og-image/{kind}/{id}", get(preview).head(preview))
}

async fn preview(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, String)>,
    headers: HeaderMap,
    method: Method,
) -> ApiResult<Response> {
    crate::domains::catalog::validation::id(&id)?;
    let _request = state
        .requests
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::busy())?;
    let _read = state
        .reads
        .acquire(Class::Catalogue)
        .await
        .map_err(|_| ApiError::busy())?;
    // A public image endpoint never reveals draft, deleted or unavailable entities.
    let statement = match kind.as_str() {
        "manga" => {
            "SELECT left(title,500),concat_ws(' · ',NULLIF(upper(language),''),NULLIF(year::text,''),publication_status),left(COALESCE(metadata->>'description',''),800) FROM platform.catalog_manga WHERE id=$1 AND state='published' AND deleted_at IS NULL AND protocol_source_visible AND platform.protocol_source_is_visible(protocol_series_id)"
        }
        "chapter" => {
            "SELECT left(m.title,500),concat_ws(' · ',CASE WHEN c.volume IS NOT NULL AND c.volume<>'' THEN 'Volume '||c.volume END,CASE WHEN c.chapter IS NOT NULL AND c.chapter<>'' THEN 'Chapter '||c.chapter END,NULLIF(c.title,''),upper(c.language)),''::text FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=$1 AND c.deleted_at IS NULL AND NOT c.is_unavailable AND m.state='published' AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) AND platform.protocol_source_is_visible(c.protocol_event_id)"
        }
        "group" => {
            "SELECT left(name,500),'Scanlation group'::text,left(COALESCE(metadata->>'description',''),800) FROM platform.catalog_groups WHERE id=$1 AND deleted_at IS NULL AND platform.protocol_group_is_visible(id)"
        }
        _ => return Err(ApiError::not_found("Preview kind not found")),
    };
    let row = state
        .db
        .client()
        .await?
        .query_opt(statement, &[&id])
        .await?
        .ok_or_else(|| ApiError::not_found("Public preview not found"))?;
    // Cache immutable rendered content only, never the eligibility decision.
    // Each request completed the current canonical visibility read above first.
    let title: String = row.get(0);
    let subtitle: String = row.get(1);
    let description: String = row.get(2);
    let inputs = serde_json::to_vec(&["svg-preview-v1", &title, &subtitle, &description])
        .map_err(|_| ApiError::unavailable())?;
    let cache_key = format!("preview-svg:{}", hex::encode(Sha256::digest(&inputs)));
    let svg = match state.public_cache.get(&cache_key).await {
        Some(value) if value.as_str().is_some() => value.as_str().unwrap().to_owned(),
        _ => {
            let svg = render(&title, &subtitle, &description);
            state
                .public_cache
                .insert(cache_key, Arc::new(serde_json::Value::String(svg.clone())))
                .await;
            svg
        }
    };
    let etag = format!("\"{}\"", hex::encode(Sha256::digest(svg.as_bytes())));
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, "public, max-age=0, must-revalidate")
        .header(header::ETAG, &etag)
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; sandbox",
        );
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|values| {
            values.split(',').any(|v| {
                let v = v.trim();
                v == "*" || v == etag || v.strip_prefix("W/") == Some(etag.as_str())
            })
        })
    {
        return builder
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .map_err(|_| ApiError::unavailable());
    }
    builder = builder.header(header::CONTENT_LENGTH, svg.len());
    builder
        .body(if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from(svg)
        })
        .map_err(|_| ApiError::unavailable())
}

fn escape(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .flat_map(|c| match c {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect(),
            '>' => "&gt;".chars().collect(),
            '"' => "&quot;".chars().collect(),
            '\'' => "&apos;".chars().collect(),
            _ => vec![c],
        })
        .collect()
}
fn lines(value: &str, width: usize, maximum: usize) -> Vec<String> {
    let clean = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = clean.chars().take(width * maximum + 1).collect::<Vec<_>>();
    let mut result = chars
        .chunks(width)
        .take(maximum)
        .map(|line| line.iter().collect::<String>())
        .collect::<Vec<_>>();
    if chars.len() > width * maximum {
        if let Some(last) = result.last_mut() {
            last.pop();
            last.push('…');
        }
    }
    result
}
fn text_lines(
    value: &str,
    width: usize,
    maximum: usize,
    start: u32,
    step: u32,
    size: u32,
    color: &str,
) -> String {
    lines(value,width,maximum).iter().enumerate().map(|(i,line)|format!("<text x=\"68\" y=\"{}\" font-family=\"sans-serif\" font-size=\"{size}\" fill=\"{color}\">{}</text>",start+i as u32*step,escape(line))).collect()
}
fn render(title: &str, subtitle: &str, description: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"630\" viewBox=\"0 0 1200 630\" role=\"img\"><title>{}</title><rect width=\"1200\" height=\"630\" fill=\"#101827\"/><rect x=\"0\" y=\"0\" width=\"1200\" height=\"12\" fill=\"#4ade80\"/><text x=\"68\" y=\"86\" font-family=\"sans-serif\" font-size=\"32\" fill=\"#4ade80\">WeebDex</text>{}{}{}<text x=\"68\" y=\"587\" font-family=\"sans-serif\" font-size=\"22\" fill=\"#94a3b8\">Read · Discuss · Discover</text></svg>",
        escape(title),
        text_lines(title, 34, 3, 170, 65, 52, "#f8fafc"),
        text_lines(subtitle, 68, 2, 375, 35, 27, "#cbd5e1"),
        text_lines(description, 83, 3, 455, 30, 23, "#94a3b8")
    )
}
