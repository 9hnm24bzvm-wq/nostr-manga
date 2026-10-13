-- Durable real signed deletion requests, separate from instance soft tombstones.
-- No chapter FK: independently committing an intent must not lock a caller's
-- native chapter transaction. The native catalogue validates the target first.
CREATE TABLE platform.protocol_deletion_intents (
 chapter_id TEXT NOT NULL CHECK(chapter_id ~ '^[A-Za-z0-9]{10}$'),
 actor_user_id TEXT NOT NULL REFERENCES platform.users(id),
 source_event_id TEXT NOT NULL CHECK(source_event_id ~ '^[0-9a-f]{64}$'),
 authority_publisher TEXT NOT NULL CHECK(authority_publisher ~ '^[0-9a-f]{64}$'),
 member_publisher TEXT NOT NULL CHECK(member_publisher ~ '^[0-9a-f]{64}$'),
 request_event JSONB,
 state TEXT NOT NULL DEFAULT 'prepared' CHECK(state IN ('prepared','requested')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(chapter_id,actor_user_id)
);
