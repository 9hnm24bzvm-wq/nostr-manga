ALTER TABLE sync_batches DROP CONSTRAINT sync_batches_state_check;
ALTER TABLE sync_batches ADD CONSTRAINT sync_batches_state_check CHECK(state IN ('queued','running','completed','completed_with_skips','cancelled','failed'));
ALTER TABLE sync_batches ADD COLUMN skipped_items BIGINT NOT NULL DEFAULT 0 CHECK(skipped_items>=0);
ALTER TABLE sync_batches ADD COLUMN skip_details_count INTEGER NOT NULL DEFAULT 0 CHECK(skip_details_count BETWEEN 0 AND 256);
ALTER TABLE sync_batches ADD COLUMN skip_counts JSONB NOT NULL DEFAULT '{}';
CREATE TABLE sync_skipped_items (
 batch_id BYTEA NOT NULL REFERENCES sync_batches(id) ON DELETE CASCADE,
 event_id BYTEA NOT NULL CHECK(octet_length(event_id)=32),
 category TEXT NOT NULL CHECK(category IN ('unsupported','policy_rejected','invalid')),
 reason TEXT NOT NULL CHECK(length(reason) BETWEEN 1 AND 64),
 PRIMARY KEY(batch_id,event_id)
);
