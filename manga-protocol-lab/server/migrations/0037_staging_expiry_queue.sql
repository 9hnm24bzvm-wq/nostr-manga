-- Staging (bit4) shares the same durable generation/receipt reclamation path.
ALTER TABLE blob_gc_queue DROP CONSTRAINT blob_gc_queue_source_mask_check;
ALTER TABLE blob_gc_queue ADD CONSTRAINT blob_gc_queue_source_mask_check CHECK(source_mask BETWEEN 1 AND 7);
CREATE TABLE staging_gc_cursors (
 owner BYTEA NOT NULL CHECK(octet_length(owner)=32),
 volume TEXT NOT NULL,
 last_hash BYTEA CHECK(last_hash IS NULL OR octet_length(last_hash)=32),
 PRIMARY KEY(owner,volume)
);
-- Pre-schema24 staged objects have no clock. Give them a fresh upgrade grace,
-- never an immediately expired guessed historical timestamp.
UPDATE blob_intents b SET staging_until=clock_timestamp()+interval '5 minutes'
 FROM blob_retention_credits r WHERE b.hash=r.hash AND b.generation=r.generation
 AND NOT r.staging_released AND b.state='available' AND b.staging_until IS NULL;
