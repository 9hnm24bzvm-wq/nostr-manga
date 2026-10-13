-- Counts start at zero and are changed only by real persisted activity, never seed popularity.
CREATE TABLE platform.catalog_manga_counters (manga_id TEXT PRIMARY KEY REFERENCES platform.catalog_manga(id), views BIGINT NOT NULL DEFAULT 0 CHECK(views>=0), follows BIGINT NOT NULL DEFAULT 0 CHECK(follows>=0), rating NUMERIC NOT NULL DEFAULT 0);
CREATE TABLE platform.catalog_chapter_counters (chapter_id TEXT PRIMARY KEY REFERENCES platform.catalog_chapters(id), views BIGINT NOT NULL DEFAULT 0 CHECK(views>=0));
CREATE TABLE platform.catalog_view_events (id BIGSERIAL PRIMARY KEY,entity_type TEXT NOT NULL CHECK(entity_type IN ('manga','chapter')),entity_id TEXT NOT NULL,visitor_hash BYTEA NOT NULL,viewed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), bucket BIGINT NOT NULL, UNIQUE(entity_type,entity_id,visitor_hash,bucket));
CREATE INDEX catalog_views_window ON platform.catalog_view_events(entity_type,viewed_at DESC,entity_id);
