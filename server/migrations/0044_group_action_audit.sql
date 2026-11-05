-- Local operator records only. Never exported as Nostr events or peer inventory.
CREATE TABLE group_action_audit (
 id BIGSERIAL PRIMARY KEY,
 group_key BYTEA NOT NULL CHECK(octet_length(group_key)=32),
 member BYTEA NOT NULL CHECK(octet_length(member)=32),
 action TEXT NOT NULL CHECK(action IN ('publish','deletion_request')),
 event_id BYTEA NOT NULL CHECK(octet_length(event_id)=32),
 permission_version BIGINT NOT NULL,
 recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(group_key,action,event_id)
);
CREATE INDEX group_action_audit_page ON group_action_audit(group_key,id DESC);
