-- A native retry belongs to one actual original lifecycle generation, not every
-- historical request that happened to target the same immutable event ID.
-- Legacy intents stay generation0 and are retained as historical evidence;
-- current actions must supply the real positive source generation explicitly.
ALTER TABLE platform.protocol_deletion_intents
 ADD COLUMN source_generation BIGINT NOT NULL DEFAULT 0 CHECK(source_generation>=0),
 ADD COLUMN intent_uuid UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE platform.protocol_deletion_intents
 DROP CONSTRAINT protocol_deletion_intents_pkey,
 ADD PRIMARY KEY(chapter_id,actor_user_id,source_generation),
 ADD UNIQUE(intent_uuid);
ALTER TABLE platform.protocol_deletion_intents ALTER COLUMN source_generation DROP DEFAULT;
