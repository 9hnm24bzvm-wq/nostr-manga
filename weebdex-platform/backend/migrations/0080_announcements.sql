-- Additive reconstruction: original archive linked announcements but omitted that source subtree.
-- No sample/news content or account authority is fabricated.
CREATE TABLE platform.announcements (
 id TEXT PRIMARY KEY, author_id TEXT NOT NULL REFERENCES platform.users(id),
 title TEXT NOT NULL CHECK(octet_length(title) BETWEEN 1 AND 300), body TEXT NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 100000),
 pinned BOOLEAN NOT NULL DEFAULT FALSE, published BOOLEAN NOT NULL DEFAULT FALSE,
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), published_at TIMESTAMPTZ, deleted_at TIMESTAMPTZ
);
CREATE INDEX announcements_public ON platform.announcements(pinned DESC,published_at DESC,id) WHERE published AND deleted_at IS NULL;
CREATE TABLE platform.announcement_audit (
 id BIGSERIAL PRIMARY KEY, announcement_id TEXT NOT NULL REFERENCES platform.announcements(id),
 actor_id TEXT NOT NULL REFERENCES platform.users(id), operation TEXT NOT NULL CHECK(operation IN ('create','update','delete')),
 old_version INTEGER, new_version INTEGER NOT NULL, published BOOLEAN NOT NULL, pinned BOOLEAN NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION platform.announcement_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(jsonb_build_object('id',a.id,'title',a.title,'body',a.body,'pinned',a.pinned,'published',a.published,'version',a.version,
 'created_at',platform.catalog_timestamp(a.created_at),'updated_at',platform.catalog_timestamp(a.updated_at),'published_at',platform.catalog_timestamp(a.published_at),
 'relationships',jsonb_build_object('author',platform.community_user_json(a.author_id)))) FROM platform.announcements a WHERE a.id=value AND a.deleted_at IS NULL
$$;
