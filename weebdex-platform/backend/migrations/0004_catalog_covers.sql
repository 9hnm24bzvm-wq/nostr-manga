CREATE TABLE platform.catalog_covers (
 id TEXT PRIMARY KEY, manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id), uploader_id TEXT NOT NULL,uploader_name TEXT NOT NULL,
 ext TEXT NOT NULL CHECK(ext IN ('jpg','jpeg','png','webp','avif')), dimensions INTEGER[] NOT NULL CHECK(array_length(dimensions,1)=2 AND dimensions[1]>0 AND dimensions[2]>0),
 is_main BOOLEAN NOT NULL DEFAULT FALSE,
 volume TEXT NOT NULL DEFAULT '', language TEXT NOT NULL DEFAULT 'en', description TEXT,
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), deleted_at TIMESTAMPTZ
);
CREATE INDEX catalog_cover_manga ON platform.catalog_covers(manga_id,created_at DESC,id) WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX catalog_cover_one_main ON platform.catalog_covers(manga_id) WHERE is_main AND deleted_at IS NULL;
