-- One-time staging-credit release is separate from immutable receipt/health rows.
CREATE TABLE blob_retention_credits (
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 staging_released BOOLEAN NOT NULL DEFAULT FALSE,
 PRIMARY KEY(hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
INSERT INTO blob_retention_credits(hash,generation,staging_released)
 SELECT hash,generation,retained FROM blob_intents;
ALTER TABLE blob_intents DROP COLUMN retained;
