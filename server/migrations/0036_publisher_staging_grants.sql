-- Logical publisher authority is independent of the physical storage owner.
-- Epoch and generation bindings prevent re-admission or hash reuse from reviving
-- an old grant. The staging lease follows the existing five-minute upload policy.
CREATE TABLE publisher_blob_grants (
 publisher BYTEA NOT NULL REFERENCES publisher_permissions(publisher),
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 permission_version BIGINT NOT NULL CHECK(permission_version>0),
 lease_until TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(publisher,hash),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
CREATE INDEX publisher_blob_grants_protection ON publisher_blob_grants(hash,generation,lease_until);
