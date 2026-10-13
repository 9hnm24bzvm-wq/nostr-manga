-- Instance discussion state only. This migration does not write signed protocol records.
ALTER TABLE platform.catalog_threads ADD COLUMN next_reply_number INTEGER NOT NULL DEFAULT 1 CHECK(next_reply_number>0);
CREATE TABLE platform.community_replies (
 id BIGSERIAL PRIMARY KEY CHECK(id<=2147483647), thread_id TEXT NOT NULL REFERENCES platform.catalog_threads(id),
 reply_number INTEGER NOT NULL CHECK(reply_number>0), parent_id BIGINT REFERENCES platform.community_replies(id) ON DELETE SET NULL,
 author_id TEXT NOT NULL REFERENCES platform.users(id), body TEXT NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 50000),
 state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('pending','active','locked','hidden','flagged','archived','deleted_by_user','deleted_by_moderator','deleted_by_system')),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), UNIQUE(thread_id,reply_number) DEFERRABLE INITIALLY DEFERRED, UNIQUE(thread_id,id)
);
CREATE INDEX community_replies_feed ON platform.community_replies(thread_id,reply_number DESC);
CREATE INDEX community_replies_author ON platform.community_replies(author_id,state);
CREATE TABLE platform.community_thread_votes (
 thread_id TEXT NOT NULL REFERENCES platform.catalog_threads(id), user_id TEXT NOT NULL REFERENCES platform.users(id),
 vote SMALLINT NOT NULL CHECK(vote IN (-1,1)), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(thread_id,user_id)
);
CREATE TABLE platform.community_reply_votes (
 reply_id BIGINT NOT NULL REFERENCES platform.community_replies(id) ON DELETE CASCADE, user_id TEXT NOT NULL REFERENCES platform.users(id),
 vote SMALLINT NOT NULL CHECK(vote IN (-1,1)), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(reply_id,user_id)
);
CREATE TABLE platform.community_audit (
 id BIGSERIAL PRIMARY KEY, actor_id TEXT NOT NULL REFERENCES platform.users(id), thread_id TEXT NOT NULL,
 reply_id BIGINT, operation TEXT NOT NULL, old_version INTEGER, new_version INTEGER,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION platform.community_user_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_build_object('id',u.id,'name',u.username,'version',u.version,'roles',u.roles) FROM platform.users u WHERE u.id=value AND NOT u.disabled
$$;
CREATE FUNCTION platform.community_thread_stats(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_build_object('replies',(SELECT count(*) FROM platform.community_replies r WHERE r.thread_id=value),
 'up',(SELECT count(*) FROM platform.community_thread_votes v WHERE v.thread_id=value AND v.vote=1),
 'down',(SELECT count(*) FROM platform.community_thread_votes v WHERE v.thread_id=value AND v.vote=-1))
$$;
CREATE OR REPLACE FUNCTION platform.catalog_thread_json(kind TEXT,value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_build_object('id',t.id,'created_at',platform.catalog_timestamp(t.created_at),'updated_at',platform.catalog_timestamp(t.updated_at),'subject',t.subject,'body',t.body,'locked',t.locked,'relationships',jsonb_build_object('author',COALESCE(platform.community_user_json(t.author_id),jsonb_build_object('id',t.author_id,'name',t.author_name,'version',1)),'stats',platform.community_thread_stats(t.id))) FROM platform.catalog_threads t WHERE t.entity_type=kind AND t.entity_id=value
$$;
CREATE FUNCTION platform.community_thread_visible(value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM platform.catalog_threads t WHERE t.id=value AND CASE t.entity_type
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id=t.entity_id AND m.deleted_at IS NULL AND m.state='published')
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=t.entity_id AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published')
 WHEN 'group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups g WHERE g.id=t.entity_id AND g.deleted_at IS NULL)
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users u WHERE u.id=t.entity_id AND NOT u.disabled) ELSE FALSE END)
$$;
CREATE FUNCTION platform.community_thread_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT platform.catalog_thread_json(t.entity_type,t.entity_id) || jsonb_build_object('relationships',
 (platform.catalog_thread_json(t.entity_type,t.entity_id)->'relationships') || jsonb_strip_nulls(jsonb_build_object(
 'manga',CASE WHEN t.entity_type='manga' THEN platform.catalog_manga_json(t.entity_id) END,
 'chapter',CASE WHEN t.entity_type='chapter' THEN platform.catalog_chapter_json(t.entity_id) END,
 'group',CASE WHEN t.entity_type='group' THEN platform.catalog_group_json(t.entity_id) END,
 'user',CASE WHEN t.entity_type='user' THEN platform.community_user_json(t.entity_id) END)))
 FROM platform.catalog_threads t WHERE t.id=value AND platform.community_thread_visible(t.id)
$$;
CREATE FUNCTION platform.community_reply_json(value BIGINT,viewer TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(jsonb_build_object('id',r.id,'reply_number',r.reply_number,'created_at',platform.catalog_timestamp(r.created_at),'updated_at',platform.catalog_timestamp(r.updated_at),'state',r.state,'version',r.version,
 'body',CASE WHEN r.state IN ('active','locked','flagged') THEN r.body END,'relationships',jsonb_strip_nulls(jsonb_build_object(
 'author',CASE WHEN r.state IN ('active','locked','flagged') THEN platform.community_user_json(r.author_id) END,
 'stats',jsonb_build_object('up',(SELECT count(*) FROM platform.community_reply_votes v WHERE v.reply_id=r.id AND v.vote=1),'down',(SELECT count(*) FROM platform.community_reply_votes v WHERE v.reply_id=r.id AND v.vote=-1)),
 'vote_state',(SELECT v.vote FROM platform.community_reply_votes v WHERE v.reply_id=r.id AND v.user_id=viewer)))))
 FROM platform.community_replies r WHERE r.id=value
$$;
