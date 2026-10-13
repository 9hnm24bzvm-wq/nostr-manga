CREATE TABLE deletion_requests (
 id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
 author BYTEA NOT NULL CHECK(octet_length(author)=32),
 raw_event JSONB NOT NULL,
 received_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE deletion_targets (
 request_id BYTEA NOT NULL REFERENCES deletion_requests(id),
 event_id BYTEA NOT NULL CHECK(octet_length(event_id)=32),
 approved_at TIMESTAMPTZ,
 approved_generation BIGINT,
 PRIMARY KEY(request_id,event_id),
 CHECK((approved_at IS NULL)=(approved_generation IS NULL))
);
CREATE INDEX deletion_targets_event ON deletion_targets(event_id,request_id);
CREATE TABLE publication_tombstones (
 event_id BYTEA PRIMARY KEY REFERENCES events(id),
 request_id BYTEA NOT NULL REFERENCES deletion_requests(id),
 generation BIGINT NOT NULL,
 active BOOLEAN NOT NULL DEFAULT TRUE,
 approved_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 restored_at TIMESTAMPTZ
);
