-- Native cookie publication delegation only. Never signer authentication, a
-- protocol_identity_binding, original admission, group membership, or a key.
CREATE TABLE platform.native_publication_delegations (
 user_id TEXT PRIMARY KEY REFERENCES platform.users(id),
 publisher TEXT NOT NULL CHECK(publisher ~ '^[0-9a-f]{64}$'),
 publisher_admission_version BIGINT NOT NULL CHECK(publisher_admission_version>0),
 revision BIGINT NOT NULL DEFAULT 1 CHECK(revision>0),
 revoked_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION platform.native_publication_delegation_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Publication delegation history cannot be deleted'; END IF;
 IF TG_OP='UPDATE' THEN
  IF NEW.user_id<>OLD.user_id THEN RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Publication delegation account is immutable'; END IF;
  IF NEW.publisher=OLD.publisher AND NEW.publisher_admission_version=OLD.publisher_admission_version AND NEW.revoked_at IS NOT DISTINCT FROM OLD.revoked_at THEN NEW.revision:=OLD.revision; RETURN NEW; END IF;
 END IF;
 IF NOT pg_try_advisory_xact_lock(697241112) THEN RAISE EXCEPTION USING ERRCODE='P6012',MESSAGE='Another publication access change is in progress'; END IF;
 IF NEW.revoked_at IS NULL THEN
  PERFORM id FROM platform.users WHERE id=NEW.user_id AND id NOT LIKE 'protocol:%' AND verified AND NOT disabled AND NOT(roles ?| ARRAY['banned','restricted']) FOR KEY SHARE NOWAIT;
  IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Publication delegation requires an enabled verified unrestricted native account'; END IF;
 END IF;
 IF TG_OP='UPDATE' THEN
  IF OLD.revision=9223372036854775807 THEN RAISE EXCEPTION USING ERRCODE='P6013',MESSAGE='Publication delegation revision exhausted'; END IF;
  NEW.revision:=OLD.revision+1;
 ELSE NEW.revision:=1;
 END IF;
 NEW.updated_at:=clock_timestamp();RETURN NEW;
END $$;
CREATE TRIGGER native_publication_delegation_guard BEFORE INSERT OR UPDATE OR DELETE ON platform.native_publication_delegations FOR EACH ROW EXECUTE FUNCTION platform.native_publication_delegation_guard();
CREATE TABLE platform.publication_access_reviews (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 operator_label TEXT NOT NULL CHECK(char_length(operator_label) BETWEEN 1 AND 100),
 user_id TEXT NOT NULL REFERENCES platform.users(id),
 operation TEXT NOT NULL CHECK(operation IN('grant','revoke')),
 digest TEXT NOT NULL CHECK(digest ~ '^[0-9a-f]{64}$'),
 review JSONB NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 expires_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()+interval '10 minutes'
);
CREATE INDEX publication_access_reviews_expiry ON platform.publication_access_reviews(expires_at,id);
CREATE INDEX publication_access_reviews_user ON platform.publication_access_reviews(user_id,created_at);
CREATE INDEX publication_access_reviews_operator ON platform.publication_access_reviews(operator_label,created_at);
CREATE TABLE platform.publication_access_audit (
 id BIGSERIAL PRIMARY KEY,
 user_id TEXT NOT NULL,
 authority TEXT NOT NULL CHECK(authority IN('offline-operator','account-closure')),
 operator_label TEXT NOT NULL,
 review_id UUID,
 operation TEXT NOT NULL,
 reason TEXT NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 500),
 before_access JSONB,
 after_access JSONB,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX publication_access_audit_target ON platform.publication_access_audit(user_id,id DESC);
CREATE TABLE platform.publication_access_receipts (
 review_id UUID PRIMARY KEY REFERENCES platform.publication_access_reviews(id),
 operator_label TEXT NOT NULL,
 digest TEXT NOT NULL CHECK(digest ~ '^[0-9a-f]{64}$'),
 receipt JSONB NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION platform.publication_access_append_only() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Publication access records are append-only'; END $$;
CREATE TRIGGER publication_access_audit_append_only BEFORE UPDATE OR DELETE ON platform.publication_access_audit FOR EACH ROW EXECUTE FUNCTION platform.publication_access_append_only();
CREATE TRIGGER publication_access_receipts_append_only BEFORE UPDATE OR DELETE ON platform.publication_access_receipts FOR EACH ROW EXECUTE FUNCTION platform.publication_access_append_only();
CREATE TRIGGER publication_access_reviews_append_only BEFORE UPDATE OR DELETE ON platform.publication_access_reviews FOR EACH ROW EXECUTE FUNCTION platform.publication_access_append_only();
-- Existing original-bound commands retain their exact historical hash and worker
-- behavior. New delegated intents pin the complete authority epoch, never adopt it.
ALTER TABLE platform.protocol_publication_commands
 ADD COLUMN authority_kind TEXT NOT NULL DEFAULT 'legacy' CHECK(authority_kind IN('legacy','native-delegation')),
 ADD COLUMN delegation_revision BIGINT,
 ADD COLUMN delegation_publisher TEXT,
 ADD COLUMN delegation_admission_version BIGINT,
 ADD COLUMN delegation_lifecycle_id UUID;
ALTER TABLE platform.protocol_publication_commands ADD CONSTRAINT publication_command_delegation_capture CHECK(
 (authority_kind='legacy' AND delegation_revision IS NULL AND delegation_publisher IS NULL AND delegation_admission_version IS NULL AND delegation_lifecycle_id IS NULL)
 OR (authority_kind='native-delegation' AND delegation_revision IS NOT NULL AND delegation_admission_version IS NOT NULL AND delegation_publisher IS NOT NULL AND delegation_revision>0 AND delegation_publisher ~ '^[0-9a-f]{64}$' AND delegation_admission_version>0 AND delegation_lifecycle_id IS NOT NULL)
);

CREATE FUNCTION platform.publication_command_authority_immutable() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF ROW(NEW.authority_kind,NEW.delegation_revision,NEW.delegation_publisher,NEW.delegation_admission_version,NEW.delegation_lifecycle_id)
 IS DISTINCT FROM ROW(OLD.authority_kind,OLD.delegation_revision,OLD.delegation_publisher,OLD.delegation_admission_version,OLD.delegation_lifecycle_id) THEN
  RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Prepared publication authority capture is immutable';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER publication_command_authority_immutable BEFORE UPDATE ON platform.protocol_publication_commands
 FOR EACH ROW EXECUTE FUNCTION platform.publication_command_authority_immutable();
-- Bounded target-specific uncertainty checks in review/apply; never scan all work.
CREATE INDEX publication_access_unresolved_commands ON platform.protocol_publication_commands(user_id) WHERE state<>'materialized';
CREATE INDEX publication_access_unresolved_deletions ON platform.protocol_deletion_intents(actor_user_id) WHERE state='prepared';
