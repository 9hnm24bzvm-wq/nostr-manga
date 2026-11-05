ALTER TABLE account_blob_grants ADD COLUMN lease_until TIMESTAMPTZ;
CREATE INDEX account_blob_grants_lease ON account_blob_grants(hash,generation,lease_until DESC) WHERE lease_until IS NOT NULL;
CREATE TABLE library_gc_queue (
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 due_at TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
CREATE INDEX library_gc_due ON library_gc_queue(due_at,hash,generation);
CREATE TABLE library_reclaims (
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 claimed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
-- Existing retired objects become candidates, not automatically deleted objects.
INSERT INTO library_gc_queue(hash,generation,due_at)
 SELECT hash,generation,max(grace_until) FROM library_object_holds WHERE state='grace' GROUP BY hash,generation;
