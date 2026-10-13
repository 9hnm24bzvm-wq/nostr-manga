-- Existing groups receive a fresh upgrade grace, never immediate expiry.
ALTER TABLE group_blob_grants ADD COLUMN generation BYTEA;
ALTER TABLE group_blob_grants ADD COLUMN lease_until TIMESTAMPTZ;
UPDATE group_blob_grants g SET generation=b.generation,
 lease_until=GREATEST(b.staging_until,clock_timestamp()+interval '5 minutes')
 FROM blob_intents b WHERE b.hash=g.hash;
ALTER TABLE group_blob_grants ALTER COLUMN generation SET NOT NULL;
ALTER TABLE group_blob_grants ALTER COLUMN lease_until SET NOT NULL;
ALTER TABLE group_blob_grants DROP CONSTRAINT group_blob_grants_hash_fkey;
ALTER TABLE group_blob_grants ADD CONSTRAINT group_blob_grants_generation_fkey
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE;
CREATE INDEX group_blob_grants_protection ON group_blob_grants(hash,generation,lease_until);
