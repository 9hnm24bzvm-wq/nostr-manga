-- Derived local aliases only. The original protocol's 52 migrations and authority
-- tables remain owned exclusively by manga-node.
CREATE TABLE platform.protocol_resource_ids (
 kind TEXT NOT NULL CHECK(kind IN ('manga','chapter','cover','author','tag','group')),
 source_target TEXT NOT NULL,
 compatibility_id TEXT NOT NULL CHECK(length(compatibility_id)=36),
 catalog_id TEXT NOT NULL CHECK(length(catalog_id)=10),
 PRIMARY KEY(kind,source_target), UNIQUE(kind,catalog_id), UNIQUE(kind,compatibility_id)
);
CREATE TABLE platform.protocol_import_cursor (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 after_event TEXT CHECK(after_event IS NULL OR after_event ~ '^[0-9a-f]{64}$'),
 completed_sweeps BIGINT NOT NULL DEFAULT 0 CHECK(completed_sweeps>=0),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
INSERT INTO platform.protocol_import_cursor(singleton) VALUES(TRUE);
CREATE TABLE platform.protocol_import_status (
 event_id TEXT PRIMARY KEY CHECK(event_id ~ '^[0-9a-f]{64}$'),
 source_version BIGINT NOT NULL CHECK(source_version>0),
 source_visible BOOLEAN NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('imported','hidden','deferred','rejected')),
 reason TEXT,
 checked_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
-- A signer is never inferred from a login or a chapter attribution. Provision
-- these mappings separately under authenticated operator control.
CREATE TABLE platform.protocol_identity_bindings (
 publisher TEXT PRIMARY KEY CHECK(publisher ~ '^[0-9a-f]{64}$'),
 user_id TEXT REFERENCES platform.users(id),
 group_id TEXT REFERENCES platform.catalog_groups(id),
 CHECK((user_id IS NOT NULL)::integer+(group_id IS NOT NULL)::integer=1)
);
CREATE TABLE platform.protocol_cover_sources (
 cover_id TEXT PRIMARY KEY REFERENCES platform.catalog_covers(id),
 event_id TEXT NOT NULL CHECK(event_id ~ '^[0-9a-f]{64}$'),
 blob_hash TEXT NOT NULL CHECK(blob_hash ~ '^[0-9a-f]{64}$'),
 mime TEXT NOT NULL, size BIGINT NOT NULL CHECK(size>0),
 derivatives_ready BOOLEAN NOT NULL DEFAULT FALSE, derivative_error TEXT,
 checked_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX protocol_import_status_retry ON platform.protocol_import_status(status,checked_at,event_id);

-- Source visibility is independent of local editorial state and tombstones.
ALTER TABLE platform.catalog_manga ADD COLUMN protocol_source_visible BOOLEAN NOT NULL DEFAULT TRUE;

CREATE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$ SELECT source_event IS NULL $$;
ALTER TABLE platform.catalog_covers ADD COLUMN protocol_event_id TEXT UNIQUE;
ALTER TABLE platform.catalog_authors ADD COLUMN protocol_event_id TEXT;
CREATE TABLE platform.protocol_publication_commands (
 command_id TEXT PRIMARY KEY CHECK(length(command_id) BETWEEN 1 AND 200),
 user_id TEXT NOT NULL REFERENCES platform.users(id),
 group_id TEXT REFERENCES platform.catalog_groups(id),
 input_hash TEXT NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
 resource_kind TEXT NOT NULL CHECK(resource_kind IN ('manga','chapter')),
 catalog_id TEXT NOT NULL CHECK(length(catalog_id)=10),
 -- This independently committed intent must not lock the active session.
 upload_session_id TEXT,
 native_commit_hash TEXT,
 created_at BIGINT NOT NULL CHECK(created_at>=0),
 event_id TEXT CHECK(event_id IS NULL OR event_id ~ '^[0-9a-f]{64}$'),
 state TEXT NOT NULL DEFAULT 'prepared' CHECK(state IN ('prepared','source_accepted','materialized')),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

-- Nested DTOs use the same fail-closed canonical-policy function.
CREATE OR REPLACE FUNCTION platform.catalog_timestamp(value TIMESTAMPTZ) RETURNS TEXT LANGUAGE SQL IMMUTABLE PARALLEL SAFE AS $$ SELECT to_char(value AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') $$;
CREATE OR REPLACE FUNCTION platform.catalog_author_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(a.metadata || jsonb_build_object('id',a.id,'group','author','name',a.name,'created_at',platform.catalog_timestamp(a.created_at),'updated_at',platform.catalog_timestamp(a.updated_at),'locked',a.locked,'version',a.version)) FROM platform.catalog_authors a WHERE a.id=value AND a.deleted_at IS NULL AND platform.protocol_source_is_visible(a.protocol_event_id)
$$;
CREATE OR REPLACE FUNCTION platform.catalog_group_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(g.metadata || jsonb_build_object('id',g.id,'name',g.name,'created_at',platform.catalog_timestamp(g.created_at),'updated_at',platform.catalog_timestamp(g.updated_at),'locked',g.locked,'inactive',g.inactive,'version',g.version,'relationships',jsonb_build_object('members',COALESCE((SELECT jsonb_agg(jsonb_build_object('id',gm.user_id,'name',gm.name,'version',1,'is_leader',gm.is_leader,'is_officer',gm.is_officer) ORDER BY gm.is_leader DESC,gm.name,gm.user_id) FROM platform.catalog_group_members gm WHERE gm.group_id=g.id),'[]'::jsonb),'thread',platform.catalog_thread_json('group',g.id),'stats',jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id WHERE cg.group_id=g.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=c.manga_id AND platform.protocol_source_is_visible(parent.protocol_series_id))))))) FROM platform.catalog_groups g WHERE g.id=value AND g.deleted_at IS NULL
$$;
CREATE OR REPLACE FUNCTION platform.catalog_cover_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(jsonb_build_object('id',c.id,'created_at',platform.catalog_timestamp(c.created_at),'updated_at',platform.catalog_timestamp(c.updated_at),'ext',c.ext,'dimensions',c.dimensions,'volume',c.volume,'language',c.language,'description',c.description,'version',c.version,'relationships',jsonb_build_object('uploader',jsonb_build_object('id',c.uploader_id,'name',c.uploader_name,'version',1)))) FROM platform.catalog_covers c WHERE c.id=value AND platform.protocol_source_is_visible(c.protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=c.manga_id AND platform.protocol_source_is_visible(parent.protocol_series_id)) AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id)
$$;
CREATE OR REPLACE FUNCTION platform.catalog_manga_base(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls((m.metadata - 'links') || jsonb_build_object('id',m.id,'created_at',platform.catalog_timestamp(m.created_at),'updated_at',platform.catalog_timestamp(m.updated_at),'published_at',platform.catalog_timestamp(m.published_at),'deleted_at',platform.catalog_timestamp(m.deleted_at),'state',m.state,'title',m.title,'year',m.year,'language',m.language,'demographic',m.demographic,'status',m.publication_status,'content_rating',m.content_rating,'locked',m.locked,'version',m.version)) FROM platform.catalog_manga m WHERE m.id=value AND platform.protocol_source_is_visible(m.protocol_series_id)
$$;
CREATE OR REPLACE FUNCTION platform.catalog_manga_tags_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT COALESCE(jsonb_agg(jsonb_build_object('id',t.id,'group',t.tag_group,'name',t.name) ORDER BY t.tag_group,t.name,t.id),'[]'::jsonb) FROM platform.catalog_manga_tags mt JOIN platform.catalog_tags t ON t.id=mt.tag_id WHERE mt.manga_id=value
$$;
CREATE OR REPLACE FUNCTION platform.catalog_relations_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT COALESCE(jsonb_agg(platform.catalog_manga_base(m.id) || jsonb_build_object('type',r.relation_type,'relationships',jsonb_strip_nulls(jsonb_build_object('tags',platform.catalog_manga_tags_json(m.id),'cover',(SELECT platform.catalog_cover_json(c.id) FROM platform.catalog_covers c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) ORDER BY c.is_main DESC,c.created_at DESC,c.id LIMIT 1)))) ORDER BY r.relation_type,m.title,m.id),'[]'::jsonb) FROM platform.catalog_relations r JOIN platform.catalog_manga m ON m.id=r.related_id WHERE r.manga_id=value AND EXISTS(SELECT 1 FROM platform.catalog_manga root WHERE root.id=value AND platform.protocol_source_is_visible(root.protocol_series_id)) AND m.state='published' AND m.deleted_at IS NULL AND platform.protocol_source_is_visible(m.protocol_series_id)
$$;
CREATE OR REPLACE FUNCTION platform.catalog_manga_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT platform.catalog_manga_base(m.id) || jsonb_build_object('relationships',jsonb_strip_nulls(jsonb_build_object(
 'submitter',jsonb_build_object('id',m.owner_id,'name',m.owner_name,'version',1),
 'cover',(SELECT platform.catalog_cover_json(c.id) FROM platform.catalog_covers c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) ORDER BY c.is_main DESC,c.created_at DESC,c.id LIMIT 1),
 'tags',platform.catalog_manga_tags_json(m.id),
 'authors',COALESCE((SELECT jsonb_agg(platform.catalog_author_json(a.id) ORDER BY a.name,a.id) FROM platform.catalog_manga_creators mc JOIN platform.catalog_authors a ON a.id=mc.author_id WHERE mc.manga_id=m.id AND mc.role='author' AND a.deleted_at IS NULL AND platform.protocol_source_is_visible(a.protocol_event_id)),'[]'::jsonb),
 'artists',COALESCE((SELECT jsonb_agg(platform.catalog_author_json(a.id) ORDER BY a.name,a.id) FROM platform.catalog_manga_creators mc JOIN platform.catalog_authors a ON a.id=mc.author_id WHERE mc.manga_id=m.id AND mc.role='artist' AND a.deleted_at IS NULL AND platform.protocol_source_is_visible(a.protocol_event_id)),'[]'::jsonb),
 'links',m.metadata->'links','relations',platform.catalog_relations_json(m.id),'thread',platform.catalog_thread_json('manga',m.id),
 'available_languages',COALESCE((SELECT jsonb_agg(x.language ORDER BY x.language) FROM (SELECT DISTINCT c.language FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND NOT c.is_unavailable)x),'[]'::jsonb),
 'available_groups',COALESCE((SELECT jsonb_agg(platform.catalog_group_json(x.group_id) ORDER BY x.group_id) FROM (SELECT DISTINCT cg.group_id FROM platform.catalog_chapter_groups cg JOIN platform.catalog_chapters c ON c.id=cg.chapter_id WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id))x),'[]'::jsonb),
 'stats',jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id)),'views',COALESCE(s.views,0),'follows',COALESCE(s.follows,0))))) FROM platform.catalog_manga m LEFT JOIN platform.catalog_manga_counters s ON s.manga_id=m.id WHERE m.id=value AND platform.protocol_source_is_visible(m.protocol_series_id)
$$;
CREATE OR REPLACE FUNCTION platform.catalog_chapter_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(c.metadata || jsonb_build_object('id',c.id,'source_id',c.source_id,'created_at',platform.catalog_timestamp(c.created_at),'updated_at',platform.catalog_timestamp(c.updated_at),'published_at',platform.catalog_timestamp(c.published_at),'deleted_at',platform.catalog_timestamp(c.deleted_at),'title',c.title,'volume',c.volume,'chapter',c.chapter,'language',c.language,'is_unavailable',c.is_unavailable,'version',c.version,'relationships',jsonb_build_object('uploader',jsonb_build_object('id',c.uploader_id,'name',c.uploader_name,'version',1),'manga',platform.catalog_manga_json(c.manga_id),'groups',COALESCE((SELECT jsonb_agg(platform.catalog_group_json(cg.group_id) ORDER BY cg.group_id) FROM platform.catalog_chapter_groups cg WHERE cg.chapter_id=c.id),'[]'::jsonb),'thread',platform.catalog_thread_json('chapter',c.id),'stats',jsonb_build_object('views',COALESCE(s.views,0))))) FROM platform.catalog_chapters c LEFT JOIN platform.catalog_chapter_counters s ON s.chapter_id=c.id WHERE c.id=value AND platform.protocol_source_is_visible(c.protocol_event_id) AND EXISTS(SELECT 1 FROM platform.catalog_manga parent WHERE parent.id=c.manga_id AND platform.protocol_source_is_visible(parent.protocol_series_id))
$$;
