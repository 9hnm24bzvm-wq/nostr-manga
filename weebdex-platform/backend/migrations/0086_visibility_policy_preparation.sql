-- Historical migrations remain unchanged. Only explicit privileged preparation
-- writes this attestation; serve/verify compare it AND the installed pg_proc body.
CREATE TABLE platform.protocol_visibility_policy (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 version BIGINT NOT NULL CHECK(version>0),
 mode TEXT NOT NULL CHECK(mode IN ('canonical','native-only')),
 checksum BYTEA NOT NULL CHECK(octet_length(checksum)=32),
 prepared_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
