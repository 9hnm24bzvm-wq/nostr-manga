-- Explicit native authority management; no data grants or startup bootstrap.
ALTER TABLE platform.users ADD COLUMN authority_version INTEGER NOT NULL DEFAULT 1 CHECK(authority_version>0);
ALTER TABLE platform.users ADD COLUMN authority_bundles JSONB NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(authority_bundles)='array');
ALTER TABLE platform.users ADD COLUMN native_session_epoch BIGINT NOT NULL DEFAULT 0 CHECK(native_session_epoch>=0);
CREATE FUNCTION platform.eligible_roles_administrator(verified BOOLEAN,disabled BOOLEAN,roles JSONB,permissions JSONB) RETURNS BOOLEAN LANGUAGE SQL IMMUTABLE AS $$
 SELECT verified AND NOT disabled AND roles ? 'sysop' AND permissions ? 'admin.roles.manage' AND NOT(roles ?| ARRAY['restricted','banned'])
$$;
CREATE INDEX users_eligible_roles_administrator ON platform.users(id) WHERE platform.eligible_roles_administrator(verified,disabled,roles,permissions);
CREATE FUNCTION platform.staff_authority_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE changed BOOLEAN; next_eligible BOOLEAN;
BEGIN
 IF TG_OP='DELETE' THEN changed:=TRUE; next_eligible:=FALSE;
 ELSE
  changed:=NOT(NEW.roles @> OLD.roles AND OLD.roles @> NEW.roles) OR NOT(NEW.permissions @> OLD.permissions AND OLD.permissions @> NEW.permissions)
   OR NEW.verified IS DISTINCT FROM OLD.verified OR NEW.disabled IS DISTINCT FROM OLD.disabled
   OR NEW.native_session_epoch IS DISTINCT FROM OLD.native_session_epoch;
  next_eligible:=platform.eligible_roles_administrator(NEW.verified,NEW.disabled,NEW.roles,NEW.permissions);
  IF NOT changed THEN NEW.authority_version:=OLD.authority_version; RETURN NEW; END IF;
 END IF;
 -- API and CLI acquire this before account/session locks. Older/native paths are
 -- protected too; lock-timeout/deadlock outcomes abort without partial changes.
 IF NOT pg_try_advisory_xact_lock(697241108) THEN
  RAISE EXCEPTION USING ERRCODE='P6003',MESSAGE='Another authority change is in progress; refresh and review again';
 END IF;
 IF OLD.id NOT LIKE 'protocol:%' AND platform.eligible_roles_administrator(OLD.verified,OLD.disabled,OLD.roles,OLD.permissions)
  AND NOT next_eligible AND NOT EXISTS(SELECT 1 FROM platform.users u WHERE u.id<>OLD.id AND u.id NOT LIKE 'protocol:%'
   AND platform.eligible_roles_administrator(u.verified,u.disabled,u.roles,u.permissions)) THEN
  RAISE EXCEPTION USING ERRCODE='P6001',MESSAGE='Hand off access to another enabled verified unrestricted roles administrator before changing this account';
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 IF OLD.authority_version=2147483647 THEN RAISE EXCEPTION USING ERRCODE='P6002',MESSAGE='Authority revision exhausted'; END IF;
 NEW.authority_version:=OLD.authority_version+1;
 RETURN NEW;
END $$;
CREATE TRIGGER staff_authority_guard BEFORE UPDATE OF roles,permissions,verified,disabled,native_session_epoch,authority_version OR DELETE ON platform.users FOR EACH ROW EXECUTE FUNCTION platform.staff_authority_guard();
CREATE TABLE platform.security_audit(
 id BIGSERIAL PRIMARY KEY, actor_id TEXT, actor_kind TEXT NOT NULL CHECK(actor_kind IN ('native','offline-bootstrap')),
 target_id TEXT NOT NULL, operation TEXT NOT NULL, reason TEXT NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 500),
 before_authority JSONB NOT NULL, after_authority JSONB NOT NULL,
 effects JSONB NOT NULL DEFAULT '{}', created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX security_audit_target ON platform.security_audit(target_id,id DESC);
CREATE FUNCTION platform.security_append_only() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Security records are append-only'; END $$;
CREATE TRIGGER security_audit_append_only BEFORE UPDATE OR DELETE ON platform.security_audit FOR EACH ROW EXECUTE FUNCTION platform.security_append_only();
CREATE TABLE platform.security_reviews(
 request_id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, target_id TEXT NOT NULL, actor_authority_version INTEGER NOT NULL CHECK(actor_authority_version>0),
 operation TEXT NOT NULL, target_authority_version INTEGER NOT NULL CHECK(target_authority_version>0),
 body_hash BYTEA NOT NULL CHECK(octet_length(body_hash)=32), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 expires_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()+interval '10 minutes'
);
CREATE INDEX security_reviews_expiry ON platform.security_reviews(expires_at,request_id);
CREATE INDEX security_reviews_actor_admission ON platform.security_reviews(actor_id,created_at);
CREATE TABLE platform.security_receipts(
 request_id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, target_id TEXT NOT NULL,
 body_hash BYTEA NOT NULL CHECK(octet_length(body_hash)=32), receipt JSONB NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TRIGGER security_receipts_append_only BEFORE UPDATE OR DELETE ON platform.security_receipts FOR EACH ROW EXECUTE FUNCTION platform.security_append_only();
