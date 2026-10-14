-- Preserve canonical fields and visibility predicates. These zero-offset
-- correlation boundaries prevent stale-statistics plans from scanning unrelated
-- parents (or hashing all chapter identities) for each selected chapter.
-- They change query planning only; no visibility result or timeout is cached.
CREATE OR REPLACE FUNCTION platform.catalog_chapter_json(value TEXT,parent_json JSONB,groups_json JSONB) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(c.metadata || jsonb_build_object('id',c.id,'source_id',c.source_id,'created_at',platform.catalog_timestamp(c.created_at),'updated_at',platform.catalog_timestamp(c.updated_at),'published_at',platform.catalog_timestamp(c.published_at),'deleted_at',platform.catalog_timestamp(c.deleted_at),'title',c.title,'volume',c.volume,'chapter',c.chapter,'language',c.language,'is_unavailable',c.is_unavailable,'version',c.version,'relationships',jsonb_build_object('uploader',jsonb_build_object('id',c.uploader_id,'name',c.uploader_name,'version',1),'manga',parent_json,'groups',groups_json,'thread',platform.catalog_thread_json('chapter',c.id),'stats',jsonb_build_object('views',COALESCE(s.views,0))))) FROM platform.catalog_chapters c LEFT JOIN platform.catalog_chapter_counters s ON s.chapter_id=c.id WHERE c.id=value AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=c.manga_id AND parent.deleted_at IS NULL AND parent.state='published' AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id) OFFSET 0)
$$;

CREATE OR REPLACE FUNCTION platform.community_thread_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM platform.catalog_threads t WHERE t.id=value AND CASE t.entity_type
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id=t.entity_id AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id))
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters c JOIN LATERAL(SELECT * FROM platform.catalog_manga parent WHERE parent.id=c.manga_id OFFSET 0)m ON TRUE WHERE c.id=t.entity_id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id) OFFSET 0)
 WHEN 'group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups g WHERE g.id=t.entity_id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id))
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users u WHERE u.id=t.entity_id AND NOT u.disabled) ELSE FALSE END)
$$;
