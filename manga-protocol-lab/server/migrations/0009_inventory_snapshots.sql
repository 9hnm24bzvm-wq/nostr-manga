CREATE INDEX events_inventory ON events(kind,state,id) INCLUDE(created_at);
CREATE TABLE inventory_snapshot_slots (
 slot SMALLINT PRIMARY KEY CHECK(slot BETWEEN 0 AND 7),
 session_id BYTEA UNIQUE CHECK(session_id IS NULL OR octet_length(session_id)=32),
 peer BYTEA CHECK(peer IS NULL OR octet_length(peer)=32),
 peer_epoch BIGINT,
 prefix TEXT NOT NULL DEFAULT '',
 filter TEXT NOT NULL DEFAULT '',
 snapshot BYTEA NOT NULL DEFAULT ''::bytea CHECK(octet_length(snapshot)<=4000000),
 expires_at TIMESTAMPTZ NOT NULL DEFAULT '-infinity',
 rounds INTEGER NOT NULL DEFAULT 0 CHECK(rounds BETWEEN 0 AND 256),
 protocol_bytes BIGINT NOT NULL DEFAULT 0 CHECK(protocol_bytes BETWEEN 0 AND 16777216)
);
INSERT INTO inventory_snapshot_slots(slot) SELECT generate_series(0,7);
