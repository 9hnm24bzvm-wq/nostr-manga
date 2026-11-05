CREATE TABLE events (
    id BYTEA PRIMARY KEY CHECK (octet_length(id)=32),
    publisher BYTEA NOT NULL CHECK (octet_length(publisher)=32),
    kind INTEGER NOT NULL CHECK (kind>=0),
    created_at BIGINT NOT NULL CHECK (created_at>=0),
    raw_event JSONB NOT NULL CHECK (jsonb_typeof(raw_event)='object'),
    state TEXT NOT NULL CHECK (state IN ('candidate','hosted','withdrawing','removed')),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version>0),
    manifest BYTEA CHECK (manifest IS NULL OR octet_length(manifest)=32),
    series BYTEA CHECK (series IS NULL OR octet_length(series)=32),
    language TEXT COLLATE "C",
    chapter TEXT COLLATE "C",
    pages BIGINT CHECK (pages IS NULL OR pages>=0),
    size BIGINT CHECK (size IS NULL OR size>=0)
);
CREATE INDEX events_discovery_desc ON events(kind,state,created_at DESC,id ASC);
CREATE INDEX events_discovery_asc ON events(kind,state,created_at ASC,id ASC);
CREATE INDEX events_series ON events(series,state,created_at DESC,id ASC);
CREATE INDEX events_publisher ON events(publisher,state,created_at DESC,id ASC);
CREATE TABLE backend_outbox (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id BYTEA NOT NULL REFERENCES events(id),
    action TEXT NOT NULL CHECK (action IN ('import','delete')),
    payload JSONB NOT NULL,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX backend_outbox_pending ON backend_outbox(id) WHERE completed_at IS NULL;
-- IDs order allocations, not transaction commits. A consumer must not treat MAX(id)
-- as a durable "all earlier transactions committed" watermark.
CREATE TABLE jobs (
    id BYTEA PRIMARY KEY CHECK (octet_length(id)=32),
    task_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    state TEXT NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','leased','completed','cancelled','failed')),
    generation BIGINT NOT NULL DEFAULT 0 CHECK(generation>=0),
    claim_token BYTEA CHECK(claim_token IS NULL OR octet_length(claim_token)=16),
    lease_until TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),
    available_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE UNIQUE INDEX jobs_one_active_key ON jobs(task_key) WHERE state IN ('queued','leased');
CREATE INDEX jobs_ready ON jobs(available_at,created_at,id) WHERE state IN ('queued','leased');
