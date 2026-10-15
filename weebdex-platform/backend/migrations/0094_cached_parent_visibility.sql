-- Additive cached-withdrawal consistency; original live authority remains required.
-- Version93 was a retired, never-written reservation. Settings must use95+.
CREATE OR REPLACE FUNCTION platform.protocol_group_is_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT NOT EXISTS(SELECT 1 FROM platform.protocol_resource_ids r WHERE r.kind='group' AND r.catalog_id=value)
 OR EXISTS(SELECT 1 FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE cg.group_id=value AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id))
$$;

CREATE OR REPLACE FUNCTION platform.community_thread_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM platform.catalog_threads t WHERE t.id=value AND CASE t.entity_type
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id=t.entity_id AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id))
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters c JOIN LATERAL(SELECT * FROM platform.catalog_manga parent WHERE parent.id=c.manga_id OFFSET 0)m ON TRUE WHERE c.id=t.entity_id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) OFFSET 0)
 WHEN 'group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups g WHERE g.id=t.entity_id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id))
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users u WHERE u.id=t.entity_id AND NOT u.disabled) ELSE FALSE END)
$$;

CREATE OR REPLACE FUNCTION platform.catalog_group_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(g.metadata || jsonb_build_object('id',g.id,'name',g.name,'created_at',platform.catalog_timestamp(g.created_at),'updated_at',platform.catalog_timestamp(g.updated_at),'locked',g.locked,'inactive',g.inactive,'version',g.version,'relationships',jsonb_build_object('members',COALESCE((SELECT jsonb_agg(jsonb_build_object('id',gm.user_id,'name',gm.name,'version',1,'is_leader',gm.is_leader,'is_officer',gm.is_officer) ORDER BY gm.is_leader DESC,gm.name,gm.user_id) FROM platform.catalog_group_members gm WHERE gm.group_id=g.id),'[]'::jsonb),'thread',platform.catalog_thread_json('group',g.id),'stats',jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id JOIN platform.catalog_manga parent ON parent.id=c.manga_id WHERE cg.group_id=g.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND parent.deleted_at IS NULL AND parent.state='published' AND parent.protocol_source_visible AND platform.protocol_source_is_visible(parent.protocol_series_id)))))) FROM platform.catalog_groups g WHERE g.id=value AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)
$$;

