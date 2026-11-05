ALTER TABLE jobs ADD COLUMN queue TEXT NOT NULL DEFAULT 'general';
CREATE INDEX jobs_queue_ready ON jobs(queue,available_at,created_at,id) WHERE state IN ('queued','leased');
CREATE TABLE sync_batches (
 id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
 request_key BYTEA NOT NULL CHECK(octet_length(request_key)=32),
 spec JSONB NOT NULL,
 state TEXT NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','running','completed','cancelled','failed')),
 partitions_created INTEGER NOT NULL DEFAULT 1 CHECK(partitions_created BETWEEN 1 AND 4096),
 partitions_done INTEGER NOT NULL DEFAULT 0,
 acknowledged_items BIGINT NOT NULL DEFAULT 0,
 last_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE UNIQUE INDEX sync_batches_active_request ON sync_batches(request_key) WHERE state IN ('queued','running');
CREATE TABLE sync_partitions (
 job_id BYTEA PRIMARY KEY REFERENCES jobs(id),
 batch_id BYTEA NOT NULL REFERENCES sync_batches(id),
 prefix TEXT NOT NULL CHECK(length(prefix)<=64),
 reconciled BOOLEAN NOT NULL DEFAULT FALSE,
 failures INTEGER NOT NULL DEFAULT 0,
 UNIQUE(batch_id,prefix)
);
CREATE TABLE sync_items (
 job_id BYTEA NOT NULL REFERENCES sync_partitions(job_id),
 event_id BYTEA NOT NULL CHECK(octet_length(event_id)=32),
 PRIMARY KEY(job_id,event_id)
);
CREATE TABLE sync_queue_budget (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 pending_items BIGINT NOT NULL DEFAULT 0 CHECK(pending_items BETWEEN 0 AND 200000)
);
INSERT INTO sync_queue_budget(singleton) VALUES(TRUE);
