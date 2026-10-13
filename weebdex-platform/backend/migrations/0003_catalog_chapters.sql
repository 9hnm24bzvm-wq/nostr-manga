CREATE TABLE platform.catalog_chapters (
 id TEXT PRIMARY KEY CHECK(length(id) BETWEEN 1 AND 128), manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id),
 uploader_id TEXT NOT NULL, uploader_name TEXT NOT NULL,
 title TEXT, volume TEXT CHECK(length(volume)<=32), chapter TEXT CHECK(length(chapter)<=32), language TEXT NOT NULL CHECK(length(language) BETWEEN 2 AND 12),
 metadata JSONB NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(metadata)='object'),
 source_id TEXT, protocol_event_id TEXT UNIQUE,
 is_unavailable BOOLEAN NOT NULL DEFAULT FALSE, version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 published_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), deleted_at TIMESTAMPTZ
);
CREATE TABLE platform.catalog_chapter_groups (chapter_id TEXT REFERENCES platform.catalog_chapters(id), group_id TEXT REFERENCES platform.catalog_groups(id), PRIMARY KEY(chapter_id,group_id));
CREATE INDEX catalog_chapter_manga_feed ON platform.catalog_chapters(manga_id,published_at DESC,id) WHERE deleted_at IS NULL;
CREATE INDEX catalog_chapter_public_feed ON platform.catalog_chapters(published_at DESC,id) WHERE deleted_at IS NULL;
CREATE INDEX catalog_chapter_language ON platform.catalog_chapters(manga_id,language,id) WHERE deleted_at IS NULL;
CREATE INDEX catalog_chapter_uploader ON platform.catalog_chapters(uploader_id,id) WHERE deleted_at IS NULL;
CREATE INDEX catalog_chapter_group_filter ON platform.catalog_chapter_groups(group_id,chapter_id);
