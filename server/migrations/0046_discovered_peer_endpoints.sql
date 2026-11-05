-- Automatically learned destinations always use the stricter pinned outbound policy.
ALTER TABLE approved_peers ADD COLUMN discovered_endpoint BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX approved_peer_discovery_page ON approved_peers(service_key) WHERE allowed;
CREATE OR REPLACE FUNCTION peer_approval_epoch() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.allowed IS DISTINCT FROM OLD.allowed OR NEW.endpoint IS DISTINCT FROM OLD.endpoint OR NEW.discovered_endpoint IS DISTINCT FROM OLD.discovered_endpoint THEN NEW.version=OLD.version+1; END IF;
 RETURN NEW;
END $$;
