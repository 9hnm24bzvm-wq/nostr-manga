-- Reversible instance-account disabling, never original signer revocation or erasure.
ALTER TABLE platform.users ADD COLUMN closed_at TIMESTAMPTZ;
-- Authenticated own-account correlation only, never an authority or preferred
-- public alias. Preserve across reactivation, backup and restore.
ALTER TABLE platform.users ADD COLUMN account_lifecycle_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid();
CREATE FUNCTION platform.account_lifecycle_identity_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.account_lifecycle_id IS DISTINCT FROM OLD.account_lifecycle_id THEN
  RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Account lifecycle identity is immutable';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER account_lifecycle_identity_immutable BEFORE UPDATE OF account_lifecycle_id ON platform.users
 FOR EACH ROW EXECUTE FUNCTION platform.account_lifecycle_identity_guard();
ALTER TABLE platform.protocol_identity_bindings ADD COLUMN revoked_at TIMESTAMPTZ;
CREATE TABLE platform.account_disable_confirmations (
 id TEXT PRIMARY KEY, token_hash BYTEA NOT NULL CHECK(octet_length(token_hash)=32),
 user_id TEXT NOT NULL REFERENCES platform.users(id), authority TEXT NOT NULL,
 account_id TEXT NOT NULL, public_user_id TEXT NOT NULL, user_version INTEGER NOT NULL CHECK(user_version>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 expires_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()+interval '10 minutes',
 used_at TIMESTAMPTZ, CHECK(expires_at>created_at)
);
CREATE INDEX account_disable_confirmations_owner ON platform.account_disable_confirmations(user_id);
CREATE TABLE platform.account_lifecycle_audit (
 id BIGSERIAL PRIMARY KEY, user_id TEXT NOT NULL REFERENCES platform.users(id),
 confirmation_id TEXT REFERENCES platform.account_disable_confirmations(id),
 operation TEXT NOT NULL CHECK(operation IN ('disable','reactivate')),
 authority TEXT NOT NULL, old_version INTEGER NOT NULL, new_version INTEGER NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), CHECK(new_version=old_version+1)
);
-- KEY SHARE is compatible with a signed caller's NO KEY UPDATE and independently
-- committed publication intents. Closure alone takes UPDATE, excludes both,
-- and never calls publication or inserts an independently committed intent.
CREATE FUNCTION platform.require_active_account(value TEXT) RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 PERFORM id FROM platform.users WHERE id=value AND NOT disabled FOR KEY SHARE NOWAIT;
 IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='42501',MESSAGE='Account is not active'; END IF;
END $$;
CREATE FUNCTION platform.account_grant_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE owner_id TEXT;
BEGIN
 IF TG_TABLE_NAME='protocol_identity_bindings' AND (to_jsonb(NEW)->>'revoked_at') IS NOT NULL THEN RETURN NEW; END IF;
 owner_id:=to_jsonb(NEW)->>TG_ARGV[0];
 IF owner_id IS NOT NULL THEN PERFORM platform.require_active_account(owner_id); END IF;
 RETURN NEW;
END $$;
-- These are authority grants/new intents, not historical attribution writes.
CREATE FUNCTION platform.account_membership_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE old_group TEXT; new_group TEXT;
BEGIN
 IF TG_OP<>'INSERT' THEN old_group:=OLD.group_id; END IF;
 IF TG_OP<>'DELETE' THEN new_group:=NEW.group_id; END IF;
 -- All membership writers, including deletion/demotion and direct core/import
 -- callers, serialize with the closure's sorted group recheck. Never a global lock.
 PERFORM id FROM platform.catalog_groups WHERE id=old_group OR id=new_group ORDER BY id FOR NO KEY UPDATE NOWAIT;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 IF TG_OP='INSERT' THEN
  PERFORM platform.require_active_account(NEW.user_id);
 ELSIF NEW.user_id<>OLD.user_id OR NEW.group_id<>OLD.group_id
   OR (NEW.is_leader AND NOT OLD.is_leader) OR (NEW.is_officer AND NOT OLD.is_officer) THEN
  PERFORM platform.require_active_account(NEW.user_id);
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER account_membership_grant BEFORE INSERT OR UPDATE OR DELETE ON platform.catalog_group_members
 FOR EACH ROW EXECUTE FUNCTION platform.account_membership_guard();
CREATE TRIGGER account_binding_grant BEFORE INSERT OR UPDATE OF user_id,revoked_at ON platform.protocol_identity_bindings
 FOR EACH ROW EXECUTE FUNCTION platform.account_grant_guard('user_id');
CREATE TRIGGER account_publication_intent BEFORE INSERT ON platform.protocol_publication_commands
 FOR EACH ROW EXECUTE FUNCTION platform.account_grant_guard('user_id');
CREATE TRIGGER account_deletion_intent BEFORE INSERT ON platform.protocol_deletion_intents
 FOR EACH ROW EXECUTE FUNCTION platform.account_grant_guard('actor_user_id');
CREATE TRIGGER account_upload_intent BEFORE INSERT ON platform.upload_sessions
 FOR EACH ROW EXECUTE FUNCTION platform.account_grant_guard('owner_id');
DO $$ BEGIN
 IF to_regclass('platform.scheduled_uploads') IS NOT NULL THEN
  EXECUTE 'CREATE TRIGGER account_schedule_intent BEFORE INSERT ON platform.scheduled_uploads FOR EACH ROW EXECUTE FUNCTION platform.account_grant_guard(''owner_id'')';
 END IF;
END $$;
-- Closed-account presentation remains an attribution identity. Private profile
-- JSON is never merged into the tombstone, including unknown extension keys.
CREATE OR REPLACE FUNCTION platform.community_user_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_build_object('id',u.id,'name',u.username,'version',u.version,'roles',CASE WHEN u.closed_at IS NOT NULL THEN '[]'::jsonb ELSE u.roles END)
 || CASE WHEN u.closed_at IS NOT NULL THEN jsonb_build_object('account_state','disabled') ELSE '{}'::jsonb END
 FROM platform.users u WHERE u.id=value AND (NOT u.disabled OR u.closed_at IS NOT NULL)
$$;
CREATE OR REPLACE FUNCTION platform.library_user_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT CASE WHEN u.closed_at IS NOT NULL THEN platform.community_user_json(u.id)
 ELSE jsonb_strip_nulls(u.profile || jsonb_build_object('id',u.id,'name',u.username,'version',u.version,'roles',u.roles,
 'relationships',jsonb_build_object('groups',COALESCE((SELECT jsonb_agg(platform.catalog_group_json(g.id) ORDER BY g.name,g.id) FROM platform.catalog_group_members gm JOIN platform.catalog_groups g ON g.id=gm.group_id WHERE gm.user_id=u.id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)),'[]'::jsonb),'thread',platform.catalog_thread_json('user',u.id)))) END
 FROM platform.users u WHERE u.id=value AND (NOT u.disabled OR u.closed_at IS NOT NULL)
$$;
