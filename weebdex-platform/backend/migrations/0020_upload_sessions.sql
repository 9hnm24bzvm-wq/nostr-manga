CREATE TABLE platform.upload_sessions (
 id TEXT PRIMARY KEY CHECK(id ~ '^[A-Za-z0-9]{10}$'),
 owner_id TEXT NOT NULL, manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id),
 chapter_id TEXT REFERENCES platform.catalog_chapters(id), edit_version INTEGER,
 groups TEXT[] NOT NULL DEFAULT '{}',
 state TEXT NOT NULL DEFAULT 'open' CHECK(state IN ('open','committed','deleted')),
 committed_chapter_id TEXT REFERENCES platform.catalog_chapters(id), commit_hash TEXT,
 version INTEGER NOT NULL DEFAULT 1, created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), expires_at TIMESTAMPTZ NOT NULL DEFAULT (clock_timestamp()+interval '24 hours')
);
CREATE UNIQUE INDEX upload_one_open_per_owner ON platform.upload_sessions(owner_id) WHERE state='open';
CREATE INDEX upload_session_expiry ON platform.upload_sessions(expires_at) WHERE state='open';
