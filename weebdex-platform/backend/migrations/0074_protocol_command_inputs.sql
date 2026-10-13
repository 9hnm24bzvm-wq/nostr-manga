-- Immutable retry input for a real original signed-publication command.
ALTER TABLE platform.protocol_publication_commands
 ADD COLUMN source_manifest BYTEA CHECK(source_manifest IS NULL OR octet_length(source_manifest)<=1048576);
CREATE TABLE platform.protocol_publication_originals (
 command_id TEXT NOT NULL REFERENCES platform.protocol_publication_commands(command_id) ON DELETE CASCADE,
 hash TEXT NOT NULL CHECK(hash ~ '^[a-f0-9]{64}$'),
 object_id TEXT NOT NULL REFERENCES platform.media_objects(id),
 PRIMARY KEY(command_id,hash)
);
CREATE INDEX protocol_original_object_pin ON platform.protocol_publication_originals(object_id);
