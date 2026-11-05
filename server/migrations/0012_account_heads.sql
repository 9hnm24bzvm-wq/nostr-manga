-- Local posting permission is independent of receiving a remote author's events.
CREATE TABLE account_permissions (
 publisher BYTEA PRIMARY KEY CHECK(octet_length(publisher)=32),
 allowed BOOLEAN NOT NULL,
 version BIGINT NOT NULL DEFAULT 1 CHECK(version>0)
);
-- Retain the winning clock even if a future deletion clears event_id.
CREATE TABLE account_heads (
 publisher BYTEA NOT NULL REFERENCES account_permissions(publisher),
 kind INTEGER NOT NULL CHECK(kind IN (0,3,10002,11999)),
 winner_at BIGINT NOT NULL CHECK(winner_at>=0),
 winner_id BYTEA NOT NULL CHECK(octet_length(winner_id)=32),
 event_id BYTEA REFERENCES events(id),
 PRIMARY KEY(publisher,kind)
);
