-- Durable protection for originals that an external serving worker may hold open.
-- No timed expiry: only a verified withdrawal/drain may release one of these rows.
CREATE TABLE public_export_holds (
 hash BYTEA PRIMARY KEY REFERENCES blob_intents(hash),
 generation BYTEA NOT NULL CHECK(octet_length(generation)=16),
 owner BYTEA NOT NULL CHECK(octet_length(owner)=32),
 volume TEXT NOT NULL,
 mime TEXT NOT NULL CHECK(mime IN ('image/png','image/jpeg','image/webp')),
 state TEXT NOT NULL DEFAULT 'creating' CHECK(state IN ('creating','active','draining')),
 withdrawal BYTEA CHECK(withdrawal IS NULL OR octet_length(withdrawal)=16),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX public_export_holds_pending ON public_export_holds(owner,volume,state,hash);
