//! Catalog-owned writer for complete instance derivative capabilities.
use super::{CatalogPage, CatalogRepository};
use crate::error::{ApiError, ApiResult};
use serde_json::{json, Value};
use tokio_postgres::Transaction;

impl CatalogRepository {
    pub(crate) async fn install_optimized_pages(
        tx: &Transaction<'_>,
        id: &str,
        pages: &[CatalogPage],
    ) -> ApiResult<()> {
        let row = tx.query_opt("SELECT metadata->'data' FROM platform.catalog_chapters WHERE id=$1 AND deleted_at IS NULL FOR UPDATE", &[&id]).await?.ok_or_else(|| ApiError::not_found("Chapter not found"))?;
        // This capability changes only instance derivative metadata. Signed source
        // event identity and original metadata.data are never rewritten.
        let data: Value = row.get(0);
        let originals = data.as_array().ok_or_else(ApiError::unavailable)?;
        if pages.is_empty() || pages.len() != originals.len() || pages.len() > 500 {
            return Err(ApiError::bad_request(
                "Optimized pages must match the complete original page set",
            ));
        }
        let rows = tx.query("SELECT d.name,o.width,o.height,o.digest,o.bytes,p.position,original.digest,original.bytes FROM platform.chapter_page_derivatives d JOIN platform.media_objects o ON o.id=d.object_id JOIN platform.chapter_pages p ON p.chapter_id=d.chapter_id AND p.position=d.position JOIN platform.media_objects original ON original.id=p.object_id WHERE d.chapter_id=$1 ORDER BY d.position", &[&id]).await?;
        if rows.len() != pages.len() {
            return Err(ApiError::bad_request("Derivative objects are incomplete"));
        }
        for (index, ((page, original), row)) in
            pages.iter().zip(originals).zip(rows.iter()).enumerate()
        {
            let dimensions = original
                .get("dimensions")
                .and_then(Value::as_array)
                .ok_or_else(ApiError::unavailable)?;
            if dimensions.len() != 2
                || page.dimensions.iter().enumerate().any(|(i, n)| {
                    *n <= 0
                        || *n > 20000
                        || dimensions[i]
                            .as_i64()
                            .is_none_or(|source| i64::from(*n) > source)
                })
                || row.get::<_, i32>(5) != index as i32
                || row.get::<_, String>(0) != page.name
                || [row.get::<_, i32>(1), row.get::<_, i32>(2)] != page.dimensions
                || row.get::<_, String>(3) == row.get::<_, String>(6)
                || row.get::<_, i64>(4) >= row.get::<_, i64>(7)
            {
                return Err(ApiError::bad_request(
                    "Derivative page order, dimensions or byte savings are invalid",
                ));
            }
        }
        let patch = json!({"data_optimized":pages});
        tx.execute(
            "UPDATE platform.catalog_chapters SET metadata=metadata || $2 WHERE id=$1",
            &[&id, &patch],
        )
        .await?;
        Ok(())
    }
}
