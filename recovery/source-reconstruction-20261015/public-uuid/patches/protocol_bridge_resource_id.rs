// RECONSTRUCTED-UNVERIFIED. Replace only this function in catalog/protocol_bridge.rs.
async fn resource_id(
    tx: &tokio_postgres::Transaction<'_>,
    kind: &str,
    target: &str,
    compatibility: &str,
) -> crate::error::ApiResult<String> {
    if !manga_node::mangadex_compat::valid_uuid(compatibility) {
        return Err(crate::error::ApiError::bad_request("Invalid protocol compatibility alias"));
    }
    let id = v::new_id(kind);
    tx.execute("INSERT INTO platform.protocol_resource_ids(kind,source_target,compatibility_id,catalog_id) VALUES($1,$2,$3,$4) ON CONFLICT(kind,source_target) DO NOTHING",&[&kind,&target,&compatibility,&id]).await?;
    let row=tx.query_one("SELECT catalog_id,compatibility_id FROM platform.protocol_resource_ids WHERE kind=$1 AND source_target=$2",&[&kind,&target]).await?;
    let native:String=row.get(0);
    if row.get::<_,String>(1)!=compatibility {
        let verified=tx.query_opt(
            "SELECT a.native_id FROM public.compatibility_ids i LEFT JOIN platform.public_resource_ids a ON a.kind=i.kind AND a.public_id=i.id::uuid WHERE i.kind=$1 AND i.id=$2 AND i.target=$3 FOR SHARE OF i",
            &[&kind,&compatibility,&target],
        ).await?;
        let verified=verified.ok_or_else(|| crate::error::ApiError::conflict("Protocol compatibility alias does not identify this source"))?;
        if let Some(assigned)=verified.get::<_,Option<String>>(0) {
            if assigned!=native { return Err(crate::error::ApiError::conflict("Protocol compatibility alias is already assigned")); }
            return Ok(native);
        }
        tx.query_one("SELECT platform.public_register_resource_alias($1,$2,$3::text::uuid)",&[&kind,&native,&compatibility]).await.map_err(|error| {
            if error.code()==Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                crate::error::ApiError::conflict("Protocol compatibility alias is already assigned")
            } else { error.into() }
        })?;
    }
    Ok(native)
}
