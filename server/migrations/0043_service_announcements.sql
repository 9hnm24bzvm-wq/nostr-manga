-- Supported signed replaceable service metadata; never confers peer permissions.
CREATE TABLE service_announcements (
 service_key BYTEA PRIMARY KEY CHECK(octet_length(service_key)=32),
 event_id BYTEA NOT NULL UNIQUE CHECK(octet_length(event_id)=32),
 created_at BIGINT NOT NULL CHECK(created_at>=0),
 raw_event JSONB NOT NULL CHECK(octet_length(raw_event::text)<=65536),
 cached_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
