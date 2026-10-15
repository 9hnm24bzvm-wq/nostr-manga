-- RECONSTRUCTED UNVERIFIED: typed presentation aliases, never graph authority.
ALTER TABLE platform.public_resource_ids DROP CONSTRAINT public_resource_ids_principal_type_check;
ALTER TABLE platform.public_resource_ids ADD CONSTRAINT public_resource_ids_principal_type_check CHECK(principal_type IN ('native','protocol_publisher','protocol_list'));
ALTER TABLE platform.public_resource_ids DROP CONSTRAINT public_resource_ids_check;
ALTER TABLE platform.public_resource_ids ADD CONSTRAINT public_resource_ids_check CHECK(
 (principal_type='protocol_publisher' AND kind='user' AND native_id ~ '^protocol:[0-9a-f]{64}$') OR
 (principal_type='protocol_list' AND kind='list' AND native_id ~ '^protocol-list:[0-9a-f]{64}:[0-9a-f]{32}$') OR
 (principal_type='native' AND (kind<>'user' OR native_id !~ '^protocol:') AND (kind<>'list' OR native_id !~ '^protocol-list:')));
CREATE FUNCTION platform.public_protocol_list_key(publisher TEXT,list_id TEXT) RETURNS TEXT LANGUAGE plpgsql IMMUTABLE STRICT PARALLEL SAFE AS $$
BEGIN
 IF publisher !~ '^[0-9a-f]{64}$' OR list_id !~ '^[0-9a-f]{32}$' THEN RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='invalid signed public-list target';END IF;
 RETURN 'protocol-list:'||publisher||':'||list_id;
END $$;
CREATE FUNCTION platform.public_register_protocol_list(publisher TEXT,list_id TEXT) RETURNS UUID LANGUAGE plpgsql AS $$
DECLARE source_target TEXT:=publisher||':'||list_id;target TEXT;legacy RECORD;canonical UUID;
BEGIN
 target:=platform.public_protocol_list_key(publisher,list_id);
 IF target IS NULL THEN RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='invalid signed public-list target';END IF;
 PERFORM platform.public_register_protocol_publisher(publisher);
 IF to_regclass('public.compatibility_ids') IS NOT NULL THEN
  FOR legacy IN EXECUTE 'SELECT id FROM public.compatibility_ids WHERE kind=''list'' AND target=$1 ORDER BY preferred DESC,id' USING source_target LOOP
   PERFORM platform.public_register_id('list',target,legacy.id::UUID,'protocol','protocol_list');
  END LOOP;
 END IF;
 canonical:=platform.public_register_id('list',target,platform.public_protocol_uuid('list',source_target),'protocol','protocol_list');
 RETURN canonical;
END $$;
CREATE TABLE platform.protocol_public_list_alias_cursor(
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 after_publisher TEXT CHECK(after_publisher ~ '^[0-9a-f]{64}$'),
 active_publisher TEXT CHECK(active_publisher ~ '^[0-9a-f]{64}$'),
 active_head TEXT CHECK(active_head ~ '^[0-9a-f]{64}$'),
 next_offset BIGINT NOT NULL DEFAULT 0 CHECK(next_offset BETWEEN 0 AND 2000000),
 CHECK((active_publisher IS NULL AND active_head IS NULL AND next_offset=0) OR (active_publisher IS NOT NULL AND active_head IS NOT NULL)));
INSERT INTO platform.protocol_public_list_alias_cursor(singleton) VALUES(TRUE);
CREATE TABLE platform.protocol_public_list_alias_projection(
 publisher TEXT PRIMARY KEY CHECK(publisher ~ '^[0-9a-f]{64}$'),
 head TEXT NOT NULL CHECK(head ~ '^[0-9a-f]{64}$'),
 completed BOOLEAN NOT NULL,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp());
