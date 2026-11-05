-- A narrow local tag index accelerates NIP01 lookups. MD5 is only an index
-- fingerprint: every query rechecks the exact original tag value, so collisions
-- cannot change filtering or authorization. Content identities remain SHA256.
CREATE TABLE relay_tags (
 event_id BYTEA NOT NULL CHECK(octet_length(event_id)=32),
 name TEXT COLLATE "C" NOT NULL CHECK(name ~ '^[A-Za-z]$'),
 value_hash TEXT COLLATE "C" NOT NULL CHECK(length(value_hash)=32),
 PRIMARY KEY(event_id,name,value_hash)
);
CREATE INDEX relay_tags_lookup ON relay_tags(name,value_hash,event_id);
CREATE FUNCTION index_relay_event() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN DELETE FROM relay_tags WHERE event_id=OLD.id;RETURN OLD;END IF;
 INSERT INTO relay_tags(event_id,name,value_hash)
 SELECT NEW.id,t->>0,md5(t->>1) FROM jsonb_array_elements(NEW.raw_event->'tags') t
 WHERE jsonb_typeof(t)='array' AND t->>0 ~ '^[A-Za-z]$' AND jsonb_typeof(t->1)='string'
 ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER events_relay_index AFTER INSERT OR DELETE ON events FOR EACH ROW EXECUTE FUNCTION index_relay_event();
INSERT INTO relay_tags(event_id,name,value_hash)
 SELECT e.id,t->>0,md5(t->>1) FROM events e CROSS JOIN LATERAL jsonb_array_elements(e.raw_event->'tags') t
 WHERE jsonb_typeof(t)='array' AND t->>0 ~ '^[A-Za-z]$' AND jsonb_typeof(t->1)='string' ON CONFLICT DO NOTHING;
CREATE FUNCTION index_relay_deletion() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE body jsonb;
BEGIN
 SELECT raw_event INTO body FROM deletion_requests WHERE id=NEW.request_id;
 INSERT INTO relay_tags(event_id,name,value_hash)
 SELECT NEW.request_id,t->>0,md5(t->>1) FROM jsonb_array_elements(body->'tags') t
 WHERE jsonb_typeof(t)='array' AND t->>0 ~ '^[A-Za-z]$' AND jsonb_typeof(t->1)='string' ON CONFLICT DO NOTHING;
 PERFORM pg_notify('manga_events',encode(NEW.request_id,'hex'));
 RETURN NEW;
END $$;
CREATE TRIGGER ordinary_deletion_relay_index AFTER INSERT ON ordinary_deletion_requests FOR EACH ROW EXECUTE FUNCTION index_relay_deletion();
INSERT INTO relay_tags(event_id,name,value_hash)
 SELECT d.id,t->>0,md5(t->>1) FROM deletion_requests d JOIN ordinary_deletion_requests o ON o.request_id=d.id CROSS JOIN LATERAL jsonb_array_elements(d.raw_event->'tags') t
 WHERE jsonb_typeof(t)='array' AND t->>0 ~ '^[A-Za-z]$' AND jsonb_typeof(t->1)='string' ON CONFLICT DO NOTHING;
CREATE FUNCTION notify_projected_event() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.completed_at IS NULL AND NEW.completed_at IS NOT NULL AND NEW.action='import' THEN
  PERFORM pg_notify('manga_events',encode(NEW.event_id,'hex'));
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER projected_event_notification AFTER UPDATE OF completed_at ON backend_outbox FOR EACH ROW EXECUTE FUNCTION notify_projected_event();
CREATE VIEW relay_visible_events AS
 SELECT e.id,e.publisher,e.kind,e.created_at,e.raw_event FROM events e
 WHERE e.state='hosted'
 AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id)
 AND NOT EXISTS(SELECT 1 FROM library_object_holds h JOIN quarantined_blobs q ON q.hash=h.hash AND q.generation=h.generation WHERE e.kind=11999 AND h.publisher=e.publisher AND h.state='current')
 AND NOT EXISTS(SELECT 1 FROM backend_outbox o WHERE o.event_id=e.id AND o.completed_at IS NULL)
 UNION ALL
 SELECT d.id,d.author,5,(d.raw_event->>'created_at')::bigint,d.raw_event
 FROM deletion_requests d JOIN ordinary_deletion_requests o ON o.request_id=d.id;
