-- Private, signature-verified native projection. Public relay reads the guarded
-- relay_visible_events view instead. No cascading FK: deletion is explicit and
-- verified absence must precede outbox retirement acknowledgement.
CREATE TABLE native_backend_events (
 id TEXT COLLATE "C" PRIMARY KEY CHECK(id ~ '^[0-9a-f]{64}$'),
 pubkey TEXT COLLATE "C" NOT NULL CHECK(pubkey ~ '^[0-9a-f]{64}$'),
 kind BIGINT NOT NULL CHECK(kind IN (0,3,10002,11999,1,7,1111,1001,1002)),
 created_at BIGINT NOT NULL CHECK(created_at >= 0),
 payload JSONB NOT NULL CHECK(jsonb_typeof(payload)='object'),
 CHECK(payload->>'id'=id AND payload->>'pubkey'=pubkey)
);
CREATE UNIQUE INDEX native_backend_replaceable_address
 ON native_backend_events(pubkey,kind) WHERE kind IN (0,3,10002,11999);
