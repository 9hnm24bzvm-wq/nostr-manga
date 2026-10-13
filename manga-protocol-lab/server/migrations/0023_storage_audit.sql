CREATE TABLE storage_audit_cursors (
 owner BYTEA NOT NULL CHECK(octet_length(owner)=32),
 volume TEXT NOT NULL,
 last_hash BYTEA CHECK(last_hash IS NULL OR octet_length(last_hash)=32),
 laps BIGINT NOT NULL DEFAULT 0 CHECK(laps>=0),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(owner,volume)
);
CREATE INDEX blob_intents_available_audit ON blob_intents(owner,volume,hash) WHERE state='available';
