-- Opt-in storage-owner queue. No role row exists on an ordinary node.
CREATE TABLE public_export_roles (
 owner BYTEA NOT NULL CHECK(octet_length(owner)=32),
 volume TEXT NOT NULL,
 root_identity BYTEA NOT NULL CHECK(octet_length(root_identity)=32),
 enabled BOOLEAN NOT NULL DEFAULT TRUE,
 backfill_after BYTEA CHECK(backfill_after IS NULL OR octet_length(backfill_after)=32),
 backfill_done BOOLEAN NOT NULL DEFAULT FALSE,
 PRIMARY KEY(owner,volume)
);
CREATE TABLE public_export_queue (
 hash BYTEA PRIMARY KEY REFERENCES blob_intents(hash) ON DELETE CASCADE,
 generation BYTEA NOT NULL CHECK(octet_length(generation)=16),
 owner BYTEA NOT NULL,
 volume TEXT NOT NULL,
 root_identity BYTEA NOT NULL CHECK(octet_length(root_identity)=32),
 version BIGINT NOT NULL DEFAULT 1 CHECK(version>0),
 due_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 32),
 last_error TEXT CHECK(last_error IS NULL OR length(last_error)<=512)
);
CREATE INDEX public_export_queue_due ON public_export_queue(owner,volume,root_identity,due_at,hash);
CREATE INDEX blob_intents_owner_volume_hash ON blob_intents(owner,volume,hash);
CREATE FUNCTION enqueue_public_export(target BYTEA,bump BOOLEAN DEFAULT TRUE) RETURNS VOID LANGUAGE SQL AS $$
 INSERT INTO public_export_queue(hash,generation,owner,volume,root_identity)
 SELECT b.hash,b.generation,r.owner,r.volume,r.root_identity FROM blob_intents b
 JOIN public_export_roles r ON r.owner=b.owner AND r.volume=b.volume AND r.enabled
 WHERE b.hash=target
 ON CONFLICT(hash) DO UPDATE SET version=public_export_queue.version+1,
 due_at=clock_timestamp(),attempts=0,last_error=NULL WHERE bump
$$;
CREATE FUNCTION queue_public_export_reference() RETURNS trigger LANGUAGE plpgsql AS $$
 BEGIN
  IF TG_OP='DELETE' THEN
   IF OLD.mime IN ('image/png','image/jpeg','image/webp') THEN PERFORM enqueue_public_export(OLD.hash); END IF;
   RETURN OLD;
  END IF;
  IF NEW.mime IN ('image/png','image/jpeg','image/webp') THEN PERFORM enqueue_public_export(NEW.hash); END IF;
  RETURN NEW;
 END
$$;
CREATE TRIGGER queue_public_export_reference AFTER INSERT OR DELETE ON publication_blobs
 FOR EACH ROW EXECUTE FUNCTION queue_public_export_reference();
CREATE FUNCTION queue_public_export_event() RETURNS trigger LANGUAGE plpgsql AS $$
 DECLARE target BYTEA;
 BEGIN
  FOR target IN SELECT hash FROM publication_blobs WHERE event_id=NEW.id
   AND mime IN ('image/png','image/jpeg','image/webp') ORDER BY hash
  LOOP PERFORM enqueue_public_export(target); END LOOP;
  RETURN NEW;
 END
$$;
CREATE TRIGGER queue_public_export_event AFTER UPDATE OF state ON events
 FOR EACH ROW WHEN(OLD.state IS DISTINCT FROM NEW.state) EXECUTE FUNCTION queue_public_export_event();
CREATE FUNCTION queue_public_export_projection() RETURNS trigger LANGUAGE plpgsql AS $$
 DECLARE target BYTEA;
 BEGIN
  FOR target IN SELECT hash FROM publication_blobs WHERE event_id=NEW.event_id
   AND mime IN ('image/png','image/jpeg','image/webp') ORDER BY hash
  LOOP PERFORM enqueue_public_export(target); END LOOP;
  RETURN NEW;
 END
$$;
CREATE TRIGGER queue_public_export_projection AFTER UPDATE OF completed_at ON backend_outbox
 FOR EACH ROW WHEN(OLD.completed_at IS NULL AND NEW.completed_at IS NOT NULL)
 EXECUTE FUNCTION queue_public_export_projection();
CREATE FUNCTION queue_public_export_creation() RETURNS trigger LANGUAGE plpgsql AS $$
 BEGIN
  IF NEW.state='creating' THEN PERFORM enqueue_public_export(NEW.hash,FALSE); END IF;
  RETURN NEW;
 END
$$;
CREATE TRIGGER queue_public_export_creation AFTER INSERT OR UPDATE OF state ON public_export_holds
 FOR EACH ROW EXECUTE FUNCTION queue_public_export_creation();
