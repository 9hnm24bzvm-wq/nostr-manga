CREATE TABLE ordinary_deletion_requests (
 request_id BYTEA PRIMARY KEY REFERENCES deletion_requests(id)
);
CREATE TABLE ordinary_event_tombstones (
 event_id BYTEA NOT NULL CHECK(octet_length(event_id)=32),
 publisher BYTEA NOT NULL CHECK(octet_length(publisher)=32),
 request_id BYTEA NOT NULL REFERENCES deletion_requests(id),
 PRIMARY KEY(event_id,publisher)
);
CREATE TABLE account_deletion_cutoffs (
 publisher BYTEA NOT NULL REFERENCES account_permissions(publisher),
 kind INTEGER NOT NULL CHECK(kind IN (0,3,10002,11999)),
 deleted_through BIGINT NOT NULL CHECK(deleted_through>=0),
 request_id BYTEA NOT NULL REFERENCES deletion_requests(id),
 PRIMARY KEY(publisher,kind)
);
