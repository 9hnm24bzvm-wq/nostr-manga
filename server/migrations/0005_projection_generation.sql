ALTER TABLE backend_outbox ADD COLUMN event_version BIGINT NOT NULL DEFAULT 1 CHECK(event_version>0);
CREATE INDEX backend_outbox_event_pending ON backend_outbox(event_id,id) WHERE completed_at IS NULL;
CREATE TABLE backend_identity (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 target_id BYTEA NOT NULL CHECK(octet_length(target_id)=32)
);
