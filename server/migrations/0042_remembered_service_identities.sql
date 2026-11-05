-- First possession-verified key per endpoint. No permission or peer row is created.
CREATE TABLE remembered_service_identities (
 endpoint TEXT PRIMARY KEY CHECK(length(endpoint) BETWEEN 1 AND 2048),
 service_key BYTEA NOT NULL CHECK(octet_length(service_key)=32),
 first_verified_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 last_verified_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX remembered_service_identity_key ON remembered_service_identities(service_key);
