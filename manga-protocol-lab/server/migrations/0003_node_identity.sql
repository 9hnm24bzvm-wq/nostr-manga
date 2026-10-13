CREATE TABLE node_identity (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
    service_key BYTEA NOT NULL CHECK(octet_length(service_key)=32),
    admin_key BYTEA NOT NULL CHECK(octet_length(admin_key)=32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE auth_used (
    id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
    expires_at BIGINT NOT NULL
);
CREATE INDEX auth_used_expiry ON auth_used(expires_at,id);
