-- Canonical source visibility is evaluated at every read; imported cached flags are not authority.
-- This migration follows70 so it can safely reference the bridge helper and mappings.
CREATE FUNCTION platform.protocol_group_is_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT NOT EXISTS(SELECT 1 FROM platform.protocol_resource_ids r WHERE r.kind='group' AND r.catalog_id=value)
 OR EXISTS(SELECT 1 FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE cg.group_id=value AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(c.protocol_event_id) AND platform.protocol_source_is_visible(m.protocol_series_id))
$$;
CREATE OR REPLACE FUNCTION platform.community_thread_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM platform.catalog_threads t WHERE t.id=value AND CASE t.entity_type
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id=t.entity_id AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id))
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=t.entity_id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id))
 WHEN 'group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups g WHERE g.id=t.entity_id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id))
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users u WHERE u.id=t.entity_id AND NOT u.disabled) ELSE FALSE END)
$$;
CREATE OR REPLACE FUNCTION platform.catalog_thread_json(kind TEXT,value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_build_object('id',t.id,'created_at',platform.catalog_timestamp(t.created_at),'updated_at',platform.catalog_timestamp(t.updated_at),'subject',t.subject,'body',t.body,'locked',t.locked,'relationships',jsonb_build_object('author',COALESCE(platform.community_user_json(t.author_id),jsonb_build_object('id',t.author_id,'name',t.author_name,'version',1)),'stats',platform.community_thread_stats(t.id))) FROM platform.catalog_threads t WHERE t.entity_type=kind AND t.entity_id=value AND platform.community_thread_visible(t.id)
$$;
CREATE OR REPLACE FUNCTION platform.report_entity_exists(kind TEXT,value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT CASE kind
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users u WHERE u.id=value AND NOT u.disabled)
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id=value AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id) AND m.deleted_at IS NULL)
 WHEN 'author' THEN EXISTS(SELECT 1 FROM platform.catalog_authors a WHERE a.id=value AND a.deleted_at IS NULL AND platform.protocol_source_is_visible(a.protocol_event_id))
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=value AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id))
 WHEN 'scanlation_group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups g WHERE g.id=value AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id))
 WHEN 'thread' THEN platform.community_thread_visible(value)
 WHEN 'reply' THEN EXISTS(SELECT 1 FROM platform.community_replies r WHERE r.id::TEXT=value AND r.state='active' AND platform.community_thread_visible(r.thread_id))
 ELSE FALSE END
$$;
CREATE OR REPLACE FUNCTION platform.analytics_user_stats(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.uploader_id=value AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND m.deleted_at IS NULL AND m.state='published' AND platform.protocol_source_is_visible(m.protocol_series_id)),
 'messages',(SELECT count(*) FROM platform.community_replies r WHERE r.author_id=value AND r.state IN ('active','locked','flagged') AND platform.community_thread_visible(r.thread_id)),
 'points',COALESCE((SELECT sum(CASE WHEN v.vote=1 THEN 1 ELSE -1 END) FROM platform.community_reply_votes v JOIN platform.community_replies r ON r.id=v.reply_id WHERE r.author_id=value AND r.state IN ('active','locked','flagged') AND platform.community_thread_visible(r.thread_id)),0))
 FROM platform.users u WHERE u.id=value AND NOT u.disabled
$$;
CREATE OR REPLACE FUNCTION platform.catalog_group_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(g.metadata || jsonb_build_object('id',g.id,'name',g.name,'created_at',platform.catalog_timestamp(g.created_at),'updated_at',platform.catalog_timestamp(g.updated_at),'locked',g.locked,'inactive',g.inactive,'version',g.version,'relationships',jsonb_build_object('members',COALESCE((SELECT jsonb_agg(jsonb_build_object('id',gm.user_id,'name',gm.name,'version',1,'is_leader',gm.is_leader,'is_officer',gm.is_officer) ORDER BY gm.is_leader DESC,gm.name,gm.user_id) FROM platform.catalog_group_members gm WHERE gm.group_id=g.id),'[]'::jsonb),'thread',platform.catalog_thread_json('group',g.id),'stats',jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id JOIN platform.catalog_manga parent ON parent.id=c.manga_id WHERE cg.group_id=g.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND parent.deleted_at IS NULL AND parent.state='published' AND platform.protocol_source_is_visible(parent.protocol_series_id)))))) FROM platform.catalog_groups g WHERE g.id=value AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)
$$;

CREATE OR REPLACE FUNCTION platform.community_reply_json(value BIGINT,viewer TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(jsonb_build_object('id',r.id,'reply_number',r.reply_number,'created_at',platform.catalog_timestamp(r.created_at),'updated_at',platform.catalog_timestamp(r.updated_at),'state',r.state,'version',r.version,
 'body',CASE WHEN r.state IN ('active','locked','flagged') THEN r.body END,'relationships',jsonb_strip_nulls(jsonb_build_object(
 'author',CASE WHEN r.state IN ('active','locked','flagged') THEN platform.community_user_json(r.author_id) END,
 'stats',jsonb_build_object('up',(SELECT count(*) FROM platform.community_reply_votes v WHERE v.reply_id=r.id AND v.vote=1),'down',(SELECT count(*) FROM platform.community_reply_votes v WHERE v.reply_id=r.id AND v.vote=-1)),
 'vote_state',(SELECT v.vote FROM platform.community_reply_votes v WHERE v.reply_id=r.id AND v.user_id=viewer)))))
 FROM platform.community_replies r WHERE r.id=value AND platform.community_thread_visible(r.thread_id)
$$;

CREATE OR REPLACE FUNCTION platform.library_user_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(u.profile || jsonb_build_object('id',u.id,'name',u.username,'version',u.version,'roles',u.roles,
 'relationships',jsonb_build_object('groups',COALESCE((SELECT jsonb_agg(platform.catalog_group_json(g.id) ORDER BY g.name,g.id) FROM platform.catalog_group_members gm JOIN platform.catalog_groups g ON g.id=gm.group_id WHERE gm.user_id=u.id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)),'[]'::jsonb),'thread',platform.catalog_thread_json('user',u.id))))
 FROM platform.users u WHERE u.id=value AND NOT u.disabled
$$;
