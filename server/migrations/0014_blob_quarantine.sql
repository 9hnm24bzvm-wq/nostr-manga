-- Sparse negative health index; available receipts remain distinct from health.
CREATE TABLE quarantined_blobs (
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 observed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 reason TEXT NOT NULL CHECK(reason IN ('read_verification_failed','write_verification_failed','audit_failed')),
 PRIMARY KEY(hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
CREATE TABLE storage_health_epoch (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 epoch BIGINT NOT NULL DEFAULT 0 CHECK(epoch>=0)
);
INSERT INTO storage_health_epoch(singleton) VALUES(TRUE);
ALTER TABLE inventory_snapshot_slots ADD COLUMN health_epoch BIGINT NOT NULL DEFAULT 0 CHECK(health_epoch>=0);
