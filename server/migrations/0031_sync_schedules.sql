ALTER TABLE sync_batches ADD COLUMN scheduled BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX sync_batches_scheduled_history ON sync_batches(created_at,id) WHERE scheduled;
CREATE TABLE sync_schedules (
 id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
 spec JSONB NOT NULL,
 interval_seconds BIGINT NOT NULL CHECK(interval_seconds BETWEEN 30 AND 86400),
 enabled BOOLEAN NOT NULL,
 revision BIGINT NOT NULL DEFAULT 1,
 next_run TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp() CHECK(isfinite(next_run)),
 last_batch BYTEA REFERENCES sync_batches(id),
 last_error TEXT,
 dispatches BIGINT NOT NULL DEFAULT 0,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX sync_schedules_due ON sync_schedules(next_run,id) WHERE enabled;
