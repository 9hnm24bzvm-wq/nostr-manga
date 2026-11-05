-- Bounded in-flight transfer pins survive process interruption until their deadline.
CREATE TABLE staging_operations (
 id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
 owner BYTEA NOT NULL CHECK(octet_length(owner)=32),
 volume TEXT NOT NULL,
 expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX staging_operations_expiry ON staging_operations(expires_at,id);
CREATE TABLE staging_operation_blobs (
 operation BYTEA NOT NULL REFERENCES staging_operations(id) ON DELETE CASCADE,
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 PRIMARY KEY(operation,hash),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
CREATE INDEX staging_operation_blob_protection ON staging_operation_blobs(hash,generation);
