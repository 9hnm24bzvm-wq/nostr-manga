CREATE TABLE public_export_health_work (
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL CHECK(octet_length(generation)=16),
 owner BYTEA NOT NULL,
 volume TEXT NOT NULL,
 root_identity BYTEA NOT NULL CHECK(octet_length(root_identity)=32),
 after_event BYTEA CHECK(after_event IS NULL OR octet_length(after_event)=32),
 current_event BYTEA CHECK(current_event IS NULL OR octet_length(current_event)=32),
 after_hash BYTEA CHECK(after_hash IS NULL OR octet_length(after_hash)=32),
 PRIMARY KEY(hash,generation)
);
CREATE INDEX public_export_health_owner ON public_export_health_work(owner,volume,root_identity,hash,generation);
CREATE FUNCTION queue_public_export_health() RETURNS trigger LANGUAGE plpgsql AS $$
 DECLARE target BYTEA; incarnation BYTEA;
 BEGIN
  IF TG_OP='DELETE' THEN target=OLD.hash;incarnation=OLD.generation;
  ELSE target=NEW.hash;incarnation=NEW.generation; END IF;
  INSERT INTO public_export_health_work(hash,generation,owner,volume,root_identity)
  SELECT b.hash,b.generation,r.owner,r.volume,r.root_identity FROM blob_intents b
   JOIN public_export_roles r ON r.owner=b.owner AND r.volume=b.volume
   WHERE b.hash=target AND b.generation=incarnation
    AND (EXISTS(SELECT 1 FROM publication_blobs p WHERE p.hash=b.hash AND p.generation=b.generation)
     OR EXISTS(SELECT 1 FROM public_export_holds h WHERE h.hash=b.hash AND h.generation=b.generation))
  ON CONFLICT(hash,generation) DO UPDATE SET after_event=NULL,current_event=NULL,after_hash=NULL;
  IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
 END
$$;
CREATE TRIGGER queue_public_export_health AFTER INSERT OR DELETE ON quarantined_blobs
 FOR EACH ROW EXECUTE FUNCTION queue_public_export_health();
-- Pausing maintenance must not discard changes to an existing public tree.
CREATE OR REPLACE FUNCTION enqueue_public_export(target BYTEA,bump BOOLEAN DEFAULT TRUE) RETURNS VOID LANGUAGE SQL AS $$
 INSERT INTO public_export_queue(hash,generation,owner,volume,root_identity)
 SELECT b.hash,b.generation,r.owner,r.volume,r.root_identity FROM blob_intents b
 JOIN public_export_roles r ON r.owner=b.owner AND r.volume=b.volume
 WHERE b.hash=target
 ON CONFLICT(hash) DO UPDATE SET version=public_export_queue.version+1,
 due_at=clock_timestamp(),attempts=0,last_error=NULL WHERE bump
$$;
