CREATE TABLE approved_peers (
 service_key BYTEA PRIMARY KEY CHECK(octet_length(service_key)=32),
 endpoint TEXT NOT NULL,
 allowed BOOLEAN NOT NULL,
 version BIGINT NOT NULL DEFAULT 1 CHECK(version>0)
);
CREATE FUNCTION peer_approval_epoch() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.allowed IS DISTINCT FROM OLD.allowed OR NEW.endpoint IS DISTINCT FROM OLD.endpoint THEN NEW.version=OLD.version+1; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER peer_approval_epoch BEFORE UPDATE ON approved_peers FOR EACH ROW EXECUTE FUNCTION peer_approval_epoch();
