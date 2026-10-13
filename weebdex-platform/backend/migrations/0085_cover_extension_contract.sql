-- The retained frontend builds original URLs as cover.id || cover.ext.
-- File/storage extensions stay bare; only the public Cover DTO includes the dot.
-- Keep the final82 canonical source/parent eligibility predicates unchanged.
CREATE OR REPLACE FUNCTION platform.catalog_cover_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(jsonb_build_object('id',c.id,'created_at',platform.catalog_timestamp(c.created_at),'updated_at',platform.catalog_timestamp(c.updated_at),'ext','.'||c.ext,'dimensions',c.dimensions,'volume',c.volume,'language',c.language,'description',c.description,'version',c.version,'relationships',jsonb_build_object('uploader',jsonb_build_object('id',c.uploader_id,'name',c.uploader_name,'version',1)))) FROM platform.catalog_covers c WHERE c.id=value AND platform.protocol_source_is_visible(c.protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=c.manga_id AND parent.deleted_at IS NULL AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id)) AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id)
$$;
