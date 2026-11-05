-- Trusted summaries exist only for locally verified immutable storage generations.
CREATE TABLE library_chunk_summaries (
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 family TEXT NOT NULL CHECK(family IN ('chapters','series','lists','members')),
 size BIGINT NOT NULL CHECK(size BETWEEN 1 AND 1000000),
 records BIGINT NOT NULL CHECK(records BETWEEN 1 AND 4096),
 bucket_base BIGINT NOT NULL CHECK(bucket_base BETWEEN 0 AND 4294967295),
 common_low_bits INTEGER NOT NULL CHECK(common_low_bits BETWEEN 0 AND 32),
 PRIMARY KEY(hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
CREATE TABLE account_blob_grants (
 publisher BYTEA NOT NULL REFERENCES account_permissions(publisher),
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 object_type TEXT NOT NULL CHECK(object_type IN ('chunk','root')),
 PRIMARY KEY(publisher,hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
CREATE INDEX account_blob_grants_object ON account_blob_grants(hash,generation);
