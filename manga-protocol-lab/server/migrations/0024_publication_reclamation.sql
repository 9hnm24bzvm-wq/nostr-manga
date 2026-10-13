-- Existing library claims retain their identity and recovery semantics.
ALTER TABLE library_gc_queue RENAME TO blob_gc_queue;
ALTER TABLE library_reclaims RENAME TO blob_reclaims;
ALTER TABLE blob_gc_queue ADD COLUMN source_mask INTEGER NOT NULL DEFAULT 1 CHECK(source_mask BETWEEN 1 AND 3);
ALTER TABLE blob_intents ADD COLUMN staging_until TIMESTAMPTZ;
-- New/restaged objects set this in admission. Existing retained objects remain
-- protected by strong references and newly scheduled retirement grace.
CREATE TABLE publication_blob_grace (
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 grace_until TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
-- Existing acknowledged removals have already dropped their references. Do not
-- guess historical ownership: only future explicit retirement schedules cleanup.
ALTER TABLE blob_reclaims ADD COLUMN retry_after TIMESTAMPTZ NOT NULL DEFAULT '-infinity';
ALTER TABLE blob_reclaims ADD COLUMN failures INTEGER NOT NULL DEFAULT 0 CHECK(failures>=0);
CREATE INDEX blob_reclaims_ready ON blob_reclaims(retry_after,claimed_at,hash);
