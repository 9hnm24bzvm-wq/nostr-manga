-- One-off, admitted-account review. Never grants publisher admission.
CREATE TABLE review_submissions (
 event_id BYTEA PRIMARY KEY CHECK(octet_length(event_id)=32),
 author BYTEA NOT NULL CHECK(octet_length(author)=32),
 account_version BIGINT NOT NULL,
 version BIGINT NOT NULL DEFAULT 1,
 state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','approved','rejected','withdrawn','expired')),
 raw_event JSONB NOT NULL,
 manifest BYTEA NOT NULL CHECK(octet_length(manifest)=32),
 declared_bytes BIGINT NOT NULL CHECK(declared_bytes>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 expires_at TIMESTAMPTZ NOT NULL,
 decided_at TIMESTAMPTZ,
 decided_by BYTEA CHECK(decided_by IS NULL OR octet_length(decided_by)=32),
 reason TEXT NOT NULL DEFAULT '' CHECK(octet_length(reason)<=4096)
);
CREATE INDEX review_pending_author ON review_submissions(author,event_id) WHERE state='pending';
CREATE INDEX review_pending_expiry ON review_submissions(expires_at,event_id) WHERE state='pending';
CREATE TABLE review_objects (
 submission BYTEA NOT NULL REFERENCES review_submissions(event_id),
 hash BYTEA NOT NULL CHECK(octet_length(hash)=32),
 size BIGINT NOT NULL CHECK(size>0),
 PRIMARY KEY(submission,hash)
);
CREATE TABLE review_blob_grants (
 submission BYTEA NOT NULL,
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 PRIMARY KEY(submission,hash),
 FOREIGN KEY(submission,hash) REFERENCES review_objects(submission,hash),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation) ON DELETE CASCADE
);
CREATE INDEX review_blob_protection ON review_blob_grants(hash,generation,submission);

CREATE TABLE review_counters (singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton), registrations BIGINT NOT NULL DEFAULT 0 CHECK(registrations>=0));
INSERT INTO review_counters(singleton) VALUES(TRUE);
