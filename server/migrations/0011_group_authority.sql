CREATE TABLE managed_groups (
 group_key BYTEA PRIMARY KEY CHECK(octet_length(group_key)=32),
 key_ref TEXT NOT NULL UNIQUE,
 enabled BOOLEAN NOT NULL DEFAULT TRUE,
 version BIGINT NOT NULL DEFAULT 1 CHECK(version>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE group_members (
 group_key BYTEA NOT NULL REFERENCES managed_groups(group_key),
 member BYTEA NOT NULL CHECK(octet_length(member)=32),
 role TEXT NOT NULL CHECK(role IN ('admin','publisher','remove')),
 PRIMARY KEY(group_key,member)
);
CREATE TABLE group_blob_grants (
 group_key BYTEA NOT NULL REFERENCES managed_groups(group_key),
 hash BYTEA NOT NULL REFERENCES blob_intents(hash) ON DELETE CASCADE,
 PRIMARY KEY(group_key,hash)
);
