-- Persisted discussion roots are shared with the community domain through an explicit seam.
CREATE TABLE platform.catalog_threads (
 id TEXT PRIMARY KEY, entity_type TEXT NOT NULL CHECK(entity_type IN ('manga','chapter','group')),
 entity_id TEXT NOT NULL, author_id TEXT NOT NULL,author_name TEXT NOT NULL,
 subject TEXT NOT NULL, body TEXT NOT NULL DEFAULT '', locked BOOLEAN NOT NULL DEFAULT FALSE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(entity_type,entity_id)
);
