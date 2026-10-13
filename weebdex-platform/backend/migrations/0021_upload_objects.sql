CREATE TABLE platform.media_objects (
 id TEXT PRIMARY KEY CHECK(id ~ '^[a-f0-9]{64}$'),
 digest TEXT NOT NULL CHECK(digest ~ '^[a-f0-9]{64}$'),
 storage_key TEXT NOT NULL UNIQUE CHECK(storage_key ~ '^[a-f0-9]{64}$'),
 mime TEXT NOT NULL CHECK(mime IN ('image/jpeg','image/png','image/webp','image/gif')),
 extension TEXT NOT NULL CHECK(extension IN ('jpg','png','webp','gif')),
 bytes BIGINT NOT NULL CHECK(bytes>0 AND bytes<=33554432),
 width INTEGER NOT NULL CHECK(width>0 AND width<=20000), height INTEGER NOT NULL CHECK(height>0 AND height<=20000),
 CHECK(width::BIGINT*height::BIGINT<=20000000),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE platform.upload_files (
 id TEXT PRIMARY KEY CHECK(id ~ '^[A-Za-z0-9]{10}$'), session_id TEXT NOT NULL REFERENCES platform.upload_sessions(id),
 object_id TEXT NOT NULL REFERENCES platform.media_objects(id),
 original_name TEXT NOT NULL CHECK(length(original_name)<=255),
 position INTEGER NOT NULL CHECK(position>=0), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(session_id,position)
);
CREATE INDEX upload_file_session ON platform.upload_files(session_id,position);
CREATE INDEX upload_file_object ON platform.upload_files(object_id);
