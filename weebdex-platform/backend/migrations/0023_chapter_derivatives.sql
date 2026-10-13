-- Authenticated upload receipt supports idempotent retry while source projection is pending.
ALTER TABLE platform.upload_sessions ADD COLUMN committed_result JSONB;

-- Protocol original names are canonical hashes without extensions. A chapter
-- may repeat the same original at more than one position; preserve each page.
ALTER TABLE platform.chapter_pages DROP CONSTRAINT chapter_pages_name_check;
ALTER TABLE platform.chapter_pages ADD CONSTRAINT chapter_pages_name_check
 CHECK(name ~ '^[A-Za-z0-9]{10}[.](jpg|png|webp|gif)$' OR name ~ '^[a-f0-9]{64}$');
ALTER TABLE platform.chapter_pages DROP CONSTRAINT chapter_pages_pkey;
ALTER TABLE platform.chapter_pages DROP CONSTRAINT chapter_pages_chapter_id_position_key;
ALTER TABLE platform.chapter_pages ADD PRIMARY KEY(chapter_id,position);
CREATE INDEX chapter_page_name ON platform.chapter_pages(chapter_id,name);

-- Optional instance-only derivatives never alter signed protocol original media.
CREATE TABLE platform.upload_file_derivatives (
 file_id TEXT PRIMARY KEY REFERENCES platform.upload_files(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK(name ~ '^[A-Za-z0-9]{10}[.](jpg|webp)$'),
 object_id TEXT NOT NULL REFERENCES platform.media_objects(id)
);
CREATE INDEX upload_derivative_object ON platform.upload_file_derivatives(object_id);
CREATE TABLE platform.chapter_page_derivatives (
 chapter_id TEXT NOT NULL REFERENCES platform.catalog_chapters(id),
 name TEXT NOT NULL CHECK(name ~ '^[A-Za-z0-9]{10}[.](jpg|webp)$'),
 object_id TEXT NOT NULL REFERENCES platform.media_objects(id),
 position INTEGER NOT NULL CHECK(position>=0),
 PRIMARY KEY(chapter_id,name), UNIQUE(chapter_id,position)
);
CREATE INDEX chapter_derivative_object ON platform.chapter_page_derivatives(object_id);
