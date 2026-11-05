-- A committed writing intent owns reserved credits until durable receipt or cleanup.
-- Hash + generation identify exact bytes; age alone never releases a reservation.
CREATE TABLE blob_intents (
 hash BYTEA PRIMARY KEY CHECK(octet_length(hash)=32),
 generation BYTEA NOT NULL CHECK(octet_length(generation)=16),
 volume TEXT NOT NULL CHECK(length(volume) BETWEEN 1 AND 160),
 owner BYTEA NOT NULL CHECK(octet_length(owner)=32),
 size BIGINT NOT NULL CHECK(size>0),
 state TEXT NOT NULL DEFAULT 'writing' CHECK(state IN ('writing','available')),
 reservation BYTEA NOT NULL UNIQUE REFERENCES capacity_reservations(id) DEFERRABLE INITIALLY DEFERRED,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 verified_at TIMESTAMPTZ,
 CHECK ((state='available')=(verified_at IS NOT NULL))
);
CREATE INDEX blob_intents_recovery ON blob_intents(volume,hash) WHERE state='writing';
CREATE INDEX blob_intents_owner ON blob_intents(owner,hash);

CREATE INDEX capacity_grants_active_selection ON capacity_grants(budget_key,holder,available,id) WHERE state='active';
