CREATE TABLE social_targets (
 event_id BYTEA PRIMARY KEY REFERENCES events(id) ON DELETE CASCADE,
 root_id BYTEA NOT NULL CHECK(octet_length(root_id)=32),
 parent_id BYTEA NOT NULL CHECK(octet_length(parent_id)=32),
 kind INTEGER NOT NULL CHECK(kind IN (7,1111)),
 created_at BIGINT NOT NULL CHECK(created_at>=0)
);
CREATE INDEX social_root_order ON social_targets(root_id,created_at DESC,event_id);
CREATE INDEX social_parent_order ON social_targets(parent_id,created_at DESC,event_id);
