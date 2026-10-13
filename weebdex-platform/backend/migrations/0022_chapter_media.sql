CREATE TABLE platform.chapter_pages (
 chapter_id TEXT NOT NULL REFERENCES platform.catalog_chapters(id), name TEXT NOT NULL CHECK(name ~ '^[A-Za-z0-9]{10}[.](jpg|png|webp|gif)$'),
 object_id TEXT NOT NULL REFERENCES platform.media_objects(id), position INTEGER NOT NULL CHECK(position>=0),
 PRIMARY KEY(chapter_id,name), UNIQUE(chapter_id,position)
);
CREATE INDEX chapter_page_object ON platform.chapter_pages(object_id);
CREATE TABLE platform.cover_media (
 cover_id TEXT NOT NULL REFERENCES platform.catalog_covers(id), variant TEXT NOT NULL CHECK(variant IN ('original','256','512')),
 object_id TEXT NOT NULL REFERENCES platform.media_objects(id), PRIMARY KEY(cover_id,variant)
);
CREATE INDEX cover_media_object ON platform.cover_media(object_id);
CREATE TABLE platform.media_cleanup (
 storage_key TEXT PRIMARY KEY CHECK(storage_key ~ '^[a-f0-9]{64}$'), queued_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
