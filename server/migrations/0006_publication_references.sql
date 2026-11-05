ALTER TABLE blob_intents ADD COLUMN retained BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE blob_intents ADD CONSTRAINT blob_intents_generation_unique UNIQUE(hash,generation);
CREATE TABLE publication_blobs (
 event_id BYTEA NOT NULL REFERENCES events(id),
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 mime TEXT NOT NULL,
 PRIMARY KEY(event_id,hash),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation)
);
CREATE INDEX publication_blobs_hash ON publication_blobs(hash,event_id);
CREATE TABLE publisher_permissions (
 publisher BYTEA PRIMARY KEY CHECK(octet_length(publisher)=32),
 allowed BOOLEAN NOT NULL,
 version BIGINT NOT NULL DEFAULT 1 CHECK(version>0)
);
CREATE FUNCTION publisher_permission_epoch() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.allowed IS DISTINCT FROM OLD.allowed THEN NEW.version=OLD.version+1; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER publisher_permission_epoch BEFORE UPDATE ON publisher_permissions FOR EACH ROW EXECUTE FUNCTION publisher_permission_epoch();
