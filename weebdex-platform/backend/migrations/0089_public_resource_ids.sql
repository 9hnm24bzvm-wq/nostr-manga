-- RECONSTRUCTED-UNVERIFIED after executor reset; rerun migration tests.
-- Additive, instance-local presentation aliases. Native PK/FK values, media paths,
-- signed events and all original-module migrations remain unchanged.
CREATE TABLE platform.public_resource_ids (
 kind TEXT NOT NULL CHECK(kind IN ('manga','chapter','cover','author','group','tag','user','list','report','upload_session','upload_file','client')),
 public_id UUID NOT NULL,
 native_id TEXT NOT NULL CHECK(octet_length(native_id) BETWEEN 1 AND 128),
 preferred BOOLEAN NOT NULL DEFAULT FALSE,
 origin TEXT NOT NULL CHECK(origin IN ('native','account','protocol','legacy')),
 principal_type TEXT NOT NULL DEFAULT 'native' CHECK(principal_type IN ('native','protocol_publisher')),
 CHECK((principal_type='protocol_publisher' AND kind='user' AND native_id ~ '^protocol:[0-9a-f]{64}$') OR (principal_type='native' AND (kind<>'user' OR native_id !~ '^protocol:'))),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(kind,public_id)
);
CREATE UNIQUE INDEX public_resource_preferred ON platform.public_resource_ids(kind,native_id) WHERE preferred;
CREATE INDEX public_resource_native ON platform.public_resource_ids(kind,native_id,public_id);

CREATE FUNCTION platform.public_native_uuid(namespace UUID,kind TEXT,native_id TEXT) RETURNS UUID LANGUAGE plpgsql IMMUTABLE STRICT PARALLEL SAFE AS $$
DECLARE digest BYTEA;
BEGIN
 digest := sha256(convert_to('platform-public-id-v1:' || namespace::TEXT || ':' || kind || ':' || native_id,'UTF8'));
 digest := set_byte(digest,6,(get_byte(digest,6) & 15) | 128);
 digest := set_byte(digest,8,(get_byte(digest,8) & 63) | 128);
 RETURN encode(substring(digest FROM 1 FOR 16),'hex')::UUID;
END
$$;

CREATE FUNCTION platform.public_resource_id_immutable() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN
  RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='public aliases cannot be deleted';
 END IF;
 IF OLD.kind IS DISTINCT FROM NEW.kind OR OLD.public_id IS DISTINCT FROM NEW.public_id OR OLD.native_id IS DISTINCT FROM NEW.native_id OR OLD.preferred IS DISTINCT FROM NEW.preferred OR OLD.principal_type IS DISTINCT FROM NEW.principal_type OR OLD.origin IS DISTINCT FROM NEW.origin OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
  RAISE EXCEPTION USING ERRCODE='23505',MESSAGE='public alias identity and preferred choice are immutable';
 END IF;
 RETURN NEW;
END
$$;
CREATE TRIGGER public_resource_id_immutable BEFORE UPDATE OR DELETE ON platform.public_resource_ids FOR EACH ROW EXECUTE FUNCTION platform.public_resource_id_immutable();

-- Protocol mapping rows may precede their native projection. Serialize first
-- preferred selection and reject every kind+UUID collision, never silently rebind.
CREATE FUNCTION platform.public_register_id(resource_kind TEXT,resource_native TEXT,alias UUID,alias_origin TEXT,alias_principal TEXT DEFAULT 'native') RETURNS UUID LANGUAGE plpgsql AS $$
DECLARE target TEXT; canonical UUID;
BEGIN
 PERFORM pg_advisory_xact_lock(hashtextextended('platform-public-id:' || resource_kind || ':' || resource_native,0));
 INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin,principal_type)
 VALUES(resource_kind,alias,resource_native,NOT EXISTS(SELECT 1 FROM platform.public_resource_ids WHERE kind=resource_kind AND native_id=resource_native AND preferred),alias_origin,alias_principal)
 ON CONFLICT(kind,public_id) DO NOTHING;
 SELECT native_id INTO target FROM platform.public_resource_ids WHERE kind=resource_kind AND public_id=alias;
 IF target IS DISTINCT FROM resource_native THEN
  RAISE EXCEPTION USING ERRCODE='23505',MESSAGE='public UUID collision';
 END IF;
 SELECT public_id INTO STRICT canonical FROM platform.public_resource_ids WHERE kind=resource_kind AND native_id=resource_native AND preferred;
 RETURN canonical;
END
$$;

CREATE FUNCTION platform.public_resource_exists(resource_kind TEXT,resource_native TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT CASE resource_kind
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga WHERE id=resource_native)
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters WHERE id=resource_native)
 WHEN 'cover' THEN EXISTS(SELECT 1 FROM platform.catalog_covers WHERE id=resource_native)
 WHEN 'author' THEN EXISTS(SELECT 1 FROM platform.catalog_authors WHERE id=resource_native)
 WHEN 'group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups WHERE id=resource_native)
 WHEN 'tag' THEN EXISTS(SELECT 1 FROM platform.catalog_tags WHERE id=resource_native)
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users WHERE id=resource_native)
 WHEN 'list' THEN EXISTS(SELECT 1 FROM platform.library_lists WHERE id=resource_native)
 WHEN 'report' THEN EXISTS(SELECT 1 FROM platform.reports WHERE id=resource_native)
 WHEN 'upload_session' THEN EXISTS(SELECT 1 FROM platform.upload_sessions WHERE id=resource_native)
 WHEN 'upload_file' THEN EXISTS(SELECT 1 FROM platform.upload_files WHERE id=resource_native)
 WHEN 'client' THEN EXISTS(SELECT 1 FROM platform.api_clients WHERE id=resource_native)
 ELSE FALSE END
$$;

-- Read source aliases without changing source tables or binding Nostr accounts.
CREATE FUNCTION platform.public_sync_protocol_aliases(resource_kind TEXT,resource_native TEXT) RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE mapping RECORD; legacy RECORD;
BEGIN
 FOR mapping IN SELECT source_target,compatibility_id FROM platform.protocol_resource_ids WHERE kind=resource_kind AND catalog_id=resource_native LOOP
  PERFORM platform.public_register_id(resource_kind,resource_native,mapping.compatibility_id::UUID,'protocol');
  IF to_regclass('public.compatibility_ids') IS NOT NULL THEN
   FOR legacy IN EXECUTE 'SELECT id FROM public.compatibility_ids WHERE kind=$1 AND target=$2 ORDER BY preferred DESC,id' USING resource_kind,mapping.source_target LOOP
    PERFORM platform.public_register_id(resource_kind,resource_native,legacy.id::UUID,'legacy');
   END LOOP;
  END IF;
 END LOOP;
END
$$;

CREATE FUNCTION platform.public_ensure_resource_id(resource_kind TEXT,resource_native TEXT) RETURNS UUID LANGUAGE plpgsql AS $$
DECLARE alias UUID; alias_origin TEXT := 'native';
BEGIN
 IF NOT platform.public_resource_exists(resource_kind,resource_native) THEN
  RAISE EXCEPTION USING ERRCODE='23503',MESSAGE='native public resource does not exist';
 END IF;
 PERFORM platform.public_sync_protocol_aliases(resource_kind,resource_native);
 SELECT public_id INTO alias FROM platform.public_resource_ids WHERE kind=resource_kind AND native_id=resource_native AND preferred;
 IF FOUND THEN RETURN alias; END IF;
 IF resource_kind IN ('user','client') AND resource_native ~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$' THEN
  alias := resource_native::UUID; alias_origin := 'account';
 ELSE
  SELECT platform.public_native_uuid(namespace,resource_kind,resource_native) INTO STRICT alias FROM platform.catalogue_namespace WHERE singleton;
 END IF;
 RETURN platform.public_register_id(resource_kind,resource_native,alias,alias_origin);
END
$$;

CREATE FUNCTION platform.public_register_resource_alias(resource_kind TEXT,resource_native TEXT,alias UUID) RETURNS UUID LANGUAGE plpgsql AS $$
BEGIN
 PERFORM platform.public_ensure_resource_id(resource_kind,resource_native);
 RETURN platform.public_register_id(resource_kind,resource_native,alias,'legacy');
END
$$;

CREATE FUNCTION platform.public_backfill_native_resource_ids() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin)
 SELECT p.kind,p.compatibility_id::UUID,p.catalog_id,NOT EXISTS(SELECT 1 FROM platform.public_resource_ids a WHERE a.kind=p.kind AND a.native_id=p.catalog_id AND a.preferred),'protocol'
 FROM platform.protocol_resource_ids p
 ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id;
 IF to_regclass('public.compatibility_ids') IS NOT NULL THEN
  EXECUTE $query$
   INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin)
   SELECT c.kind,c.id::UUID,p.catalog_id,FALSE,'legacy' FROM public.compatibility_ids c JOIN platform.protocol_resource_ids p ON p.kind=c.kind AND p.source_target=c.target
   ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id
  $query$;
 END IF;
 INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin)
 SELECT r.kind,CASE WHEN r.kind IN ('user','client') AND r.id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$' THEN r.id::UUID ELSE platform.public_native_uuid(n.namespace,r.kind,r.id) END,r.id,
 NOT EXISTS(SELECT 1 FROM platform.public_resource_ids a WHERE a.kind=r.kind AND a.native_id=r.id AND a.preferred),
 CASE WHEN r.kind IN ('user','client') AND r.id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$' THEN 'account' ELSE 'native' END
 FROM (
 SELECT 'manga'::TEXT kind,id FROM platform.catalog_manga UNION ALL SELECT 'chapter',id FROM platform.catalog_chapters
 UNION ALL SELECT 'cover',id FROM platform.catalog_covers UNION ALL SELECT 'author',id FROM platform.catalog_authors
 UNION ALL SELECT 'group',id FROM platform.catalog_groups UNION ALL SELECT 'tag',id FROM platform.catalog_tags
 UNION ALL SELECT 'user',id FROM platform.users UNION ALL SELECT 'list',id FROM platform.library_lists
 UNION ALL SELECT 'report',id FROM platform.reports UNION ALL SELECT 'upload_session',id FROM platform.upload_sessions
 UNION ALL SELECT 'upload_file',id FROM platform.upload_files UNION ALL SELECT 'client',id FROM platform.api_clients
 ) r CROSS JOIN platform.catalogue_namespace n WHERE n.singleton
 ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id;
END
$$;

CREATE FUNCTION platform.public_create_native_alias() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 PERFORM platform.public_ensure_resource_id(TG_ARGV[0],NEW.id);
 RETURN NEW;
END
$$;
CREATE TRIGGER public_manga_alias AFTER INSERT ON platform.catalog_manga FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('manga');
CREATE TRIGGER public_chapter_alias AFTER INSERT ON platform.catalog_chapters FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('chapter');
CREATE TRIGGER public_cover_alias AFTER INSERT ON platform.catalog_covers FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('cover');
CREATE TRIGGER public_author_alias AFTER INSERT ON platform.catalog_authors FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('author');
CREATE TRIGGER public_group_alias AFTER INSERT ON platform.catalog_groups FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('group');
CREATE TRIGGER public_tag_alias AFTER INSERT ON platform.catalog_tags FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('tag');
CREATE TRIGGER public_user_alias AFTER INSERT ON platform.users FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('user');
CREATE TRIGGER public_list_alias AFTER INSERT ON platform.library_lists FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('list');
CREATE TRIGGER public_report_alias AFTER INSERT ON platform.reports FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('report');
CREATE TRIGGER public_upload_session_alias AFTER INSERT ON platform.upload_sessions FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('upload_session');
CREATE TRIGGER public_upload_file_alias AFTER INSERT ON platform.upload_files FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('upload_file');
CREATE TRIGGER public_client_alias AFTER INSERT ON platform.api_clients FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('client');

CREATE FUNCTION platform.public_create_protocol_alias() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 PERFORM platform.public_sync_protocol_aliases(NEW.kind,NEW.catalog_id);
 RETURN NEW;
END
$$;
CREATE TRIGGER public_protocol_alias AFTER INSERT OR UPDATE OF compatibility_id,catalog_id,source_target ON platform.protocol_resource_ids FOR EACH ROW EXECUTE FUNCTION platform.public_create_protocol_alias();

CREATE FUNCTION platform.public_protocol_uuid(kind TEXT,target TEXT) RETURNS UUID LANGUAGE plpgsql IMMUTABLE STRICT PARALLEL SAFE AS $$
DECLARE digest BYTEA;
BEGIN
 digest := sha256(convert_to('manga-compat-v1:' || kind || ':' || target,'UTF8'));
 digest := set_byte(digest,6,(get_byte(digest,6) & 15) | 128);
 digest := set_byte(digest,8,(get_byte(digest,8) & 63) | 128);
 RETURN encode(substring(digest FROM 1 FOR 16),'hex')::UUID;
END
$$;

CREATE FUNCTION platform.public_protocol_publishers() RETURNS TABLE(pubkey TEXT) LANGUAGE SQL STABLE AS $$
 SELECT substring(owner_id FROM 10) FROM platform.catalog_manga WHERE owner_id ~ '^protocol:[0-9a-f]{64}$'
 UNION SELECT substring(owner_id FROM 10) FROM platform.catalog_authors WHERE owner_id ~ '^protocol:[0-9a-f]{64}$'
 UNION SELECT substring(owner_id FROM 10) FROM platform.catalog_groups WHERE owner_id ~ '^protocol:[0-9a-f]{64}$'
 UNION SELECT substring(uploader_id FROM 10) FROM platform.catalog_chapters WHERE uploader_id ~ '^protocol:[0-9a-f]{64}$'
 UNION SELECT substring(uploader_id FROM 10) FROM platform.catalog_covers WHERE uploader_id ~ '^protocol:[0-9a-f]{64}$'
 UNION SELECT substring(user_id FROM 10) FROM platform.catalog_group_members WHERE user_id ~ '^protocol:[0-9a-f]{64}$'
$$;

CREATE FUNCTION platform.public_register_protocol_publisher(publisher TEXT) RETURNS UUID LANGUAGE plpgsql AS $$
DECLARE legacy RECORD; canonical UUID; target TEXT := 'protocol:' || publisher;
BEGIN
 IF publisher !~ '^[0-9a-f]{64}$' OR publisher IS NULL THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='invalid protocol publisher';
 END IF;
 IF to_regclass('public.compatibility_ids') IS NOT NULL THEN
  FOR legacy IN EXECUTE 'SELECT id FROM public.compatibility_ids WHERE kind=''user'' AND target=$1 ORDER BY preferred DESC,id' USING publisher LOOP
   PERFORM platform.public_register_id('user',target,legacy.id::UUID,'protocol','protocol_publisher');
  END LOOP;
 END IF;
 canonical := platform.public_register_id('user',target,platform.public_protocol_uuid('user',publisher),'protocol','protocol_publisher');
 RETURN canonical;
END
$$;

CREATE FUNCTION platform.public_backfill_publisher_aliases() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 IF to_regclass('public.compatibility_ids') IS NOT NULL THEN
  EXECUTE $query$
   INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin,principal_type)
   SELECT 'user',c.id::UUID,'protocol:' || p.pubkey,NOT EXISTS(SELECT 1 FROM platform.public_resource_ids a WHERE a.kind='user' AND a.native_id='protocol:' || p.pubkey AND a.preferred),'protocol','protocol_publisher'
   FROM platform.public_protocol_publishers() p JOIN LATERAL (SELECT id FROM public.compatibility_ids WHERE kind='user' AND target=p.pubkey ORDER BY preferred DESC,id LIMIT 1) c ON TRUE
   ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id;
   INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin,principal_type)
   SELECT 'user',c.id::UUID,'protocol:' || p.pubkey,FALSE,'protocol','protocol_publisher'
   FROM platform.public_protocol_publishers() p JOIN public.compatibility_ids c ON c.kind='user' AND c.target=p.pubkey
   ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id
  $query$;
 END IF;
 INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin,principal_type)
 SELECT 'user',platform.public_protocol_uuid('user',p.pubkey),'protocol:' || p.pubkey,NOT EXISTS(SELECT 1 FROM platform.public_resource_ids a WHERE a.kind='user' AND a.native_id='protocol:' || p.pubkey AND a.preferred),'protocol','protocol_publisher'
 FROM platform.public_protocol_publishers() p
 ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id;
END
$$;

CREATE FUNCTION platform.public_backfill_resource_ids() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 PERFORM platform.public_backfill_native_resource_ids();
 PERFORM platform.public_backfill_publisher_aliases();
END
$$;
SELECT platform.public_backfill_resource_ids();

CREATE FUNCTION platform.public_create_publisher_alias() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE reference TEXT := to_jsonb(NEW)->>TG_ARGV[0];
BEGIN
 IF reference ~ '^protocol:[0-9a-f]{64}$' THEN
  PERFORM platform.public_register_protocol_publisher(substring(reference FROM 10));
 END IF;
 RETURN NEW;
END
$$;
CREATE TRIGGER public_manga_publisher AFTER INSERT OR UPDATE OF owner_id ON platform.catalog_manga FOR EACH ROW EXECUTE FUNCTION platform.public_create_publisher_alias('owner_id');
CREATE TRIGGER public_author_publisher AFTER INSERT OR UPDATE OF owner_id ON platform.catalog_authors FOR EACH ROW EXECUTE FUNCTION platform.public_create_publisher_alias('owner_id');
CREATE TRIGGER public_group_publisher AFTER INSERT OR UPDATE OF owner_id ON platform.catalog_groups FOR EACH ROW EXECUTE FUNCTION platform.public_create_publisher_alias('owner_id');
CREATE TRIGGER public_chapter_publisher AFTER INSERT OR UPDATE OF uploader_id ON platform.catalog_chapters FOR EACH ROW EXECUTE FUNCTION platform.public_create_publisher_alias('uploader_id');
CREATE TRIGGER public_cover_publisher AFTER INSERT OR UPDATE OF uploader_id ON platform.catalog_covers FOR EACH ROW EXECUTE FUNCTION platform.public_create_publisher_alias('uploader_id');
CREATE TRIGGER public_group_member_publisher AFTER INSERT OR UPDATE OF user_id ON platform.catalog_group_members FOR EACH ROW EXECUTE FUNCTION platform.public_create_publisher_alias('user_id');
