-- Private scheduling only: no catalogue/source/public-alias row is allocated here.
ALTER TABLE platform.upload_sessions DROP CONSTRAINT upload_sessions_state_check;
ALTER TABLE platform.upload_sessions ADD CONSTRAINT upload_sessions_state_check
 CHECK(state IN ('open','scheduled','committed','deleted'));
CREATE TABLE platform.scheduled_uploads (
 session_id TEXT PRIMARY KEY REFERENCES platform.upload_sessions(id),
 owner_id TEXT NOT NULL REFERENCES platform.users(id),
 publish_at TEXT NOT NULL, due_at TIMESTAMPTZ NOT NULL, retry_until TIMESTAMPTZ NOT NULL CHECK(retry_until>due_at AND retry_until<=due_at+interval '24 hours'),
 payload JSONB NOT NULL CHECK(jsonb_typeof(payload)='object'),
 payload_hash TEXT NOT NULL CHECK(payload_hash ~ '^[a-f0-9]{64}$'),
 session_version INTEGER NOT NULL CHECK(session_version>0),
 author TEXT NOT NULL CHECK(author ~ '^[a-f0-9]{64}$'),
 service TEXT NOT NULL CHECK(service ~ '^[a-f0-9]{64}$'),
 origin TEXT NOT NULL, admission_version BIGINT NOT NULL,
 files_fingerprint TEXT NOT NULL CHECK(files_fingerprint ~ '^[a-f0-9]{64}$'),
 signer TEXT NOT NULL CHECK(signer ~ '^[a-f0-9]{64}$'), signing_group TEXT,
 parent_source TEXT,
 parent_fingerprint TEXT NOT NULL CHECK(parent_fingerprint ~ '^[a-f0-9]{64}$'),
 parent_manifest BYTEA, parent_cover_id TEXT,
 chapter_id TEXT NOT NULL CHECK(chapter_id ~ '^[A-Za-z0-9]{10}$'), chapter_uuid UUID NOT NULL,
 retained_bytes BIGINT NOT NULL CHECK(retained_bytes>0),
 state TEXT NOT NULL DEFAULT 'scheduled' CHECK(state IN ('scheduled','starting','blocked','complete','cancelled')),
 started_at TIMESTAMPTZ, lease_token TEXT, lease_until TIMESTAMPTZ,
 generation BIGINT NOT NULL DEFAULT 0, failure_code TEXT,
 retry_attempts INTEGER NOT NULL DEFAULT 0 CHECK(retry_attempts BETWEEN 0 AND 5),
 next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 CHECK(retry_until>due_at), CHECK((lease_token IS NULL)=(lease_until IS NULL)),
 CHECK(state<>'starting' OR started_at IS NOT NULL),
 CHECK(state<>'cancelled' OR started_at IS NULL)
);
CREATE INDEX scheduled_uploads_due ON platform.scheduled_uploads(due_at,session_id)
 WHERE state IN ('scheduled','starting');
CREATE INDEX scheduled_uploads_owner ON platform.scheduled_uploads(owner_id,created_at,session_id);
CREATE FUNCTION platform.scheduled_upload_immutable() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF ROW(NEW.session_id,NEW.owner_id,NEW.publish_at,NEW.due_at,NEW.retry_until,NEW.payload,NEW.payload_hash,
 NEW.session_version,NEW.author,NEW.service,NEW.admission_version,NEW.origin,NEW.files_fingerprint,NEW.signer,NEW.signing_group,
 NEW.parent_source,NEW.parent_fingerprint,NEW.parent_manifest,NEW.parent_cover_id,NEW.chapter_id,NEW.chapter_uuid,NEW.retained_bytes,NEW.created_at)
 IS DISTINCT FROM ROW(OLD.session_id,OLD.owner_id,OLD.publish_at,OLD.due_at,OLD.retry_until,OLD.payload,OLD.payload_hash,
 OLD.session_version,OLD.author,OLD.service,OLD.admission_version,OLD.origin,OLD.files_fingerprint,OLD.signer,OLD.signing_group,
 OLD.parent_source,OLD.parent_fingerprint,OLD.parent_manifest,OLD.parent_cover_id,OLD.chapter_id,OLD.chapter_uuid,OLD.retained_bytes,OLD.created_at)
 OR (OLD.started_at IS NOT NULL AND NEW.started_at IS DISTINCT FROM OLD.started_at)
 OR (OLD.state IN ('complete','cancelled') AND NEW.state<>OLD.state) THEN
  RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Scheduled publication input and start boundary are immutable';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER scheduled_upload_immutable BEFORE UPDATE ON platform.scheduled_uploads
 FOR EACH ROW EXECUTE FUNCTION platform.scheduled_upload_immutable();

-- Provenance must not acquire a foreign-key lock on the outer upload/intent.
ALTER TABLE platform.protocol_publication_commands ADD COLUMN scheduled_session_id TEXT;
CREATE FUNCTION platform.publication_schedule_provenance_immutable() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.scheduled_session_id IS DISTINCT FROM OLD.scheduled_session_id THEN
  RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Publication schedule provenance is immutable';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER publication_schedule_provenance_immutable BEFORE UPDATE ON platform.protocol_publication_commands
 FOR EACH ROW EXECUTE FUNCTION platform.publication_schedule_provenance_immutable();
