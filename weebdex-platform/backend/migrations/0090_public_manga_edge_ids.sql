-- RECONSTRUCTED, versioned edge identity after execution workspace replacement.
-- The lost, never-deployed feature migration90 checksum/edge codec is unknown.
-- This explicit v2 codec is not claimed byte-identical. Existing stored aliases
-- are immutable and never recomputed/replaced when an edge is deleted/re-added.
ALTER TABLE platform.public_resource_ids DROP CONSTRAINT public_resource_ids_kind_check;
ALTER TABLE platform.public_resource_ids ADD CONSTRAINT public_resource_ids_kind_check CHECK(kind IN ('manga','chapter','cover','author','group','tag','user','list','report','upload_session','upload_file','client','manga_relation','manga_recommendation'));
ALTER TABLE platform.public_resource_ids DROP CONSTRAINT public_resource_ids_native_id_check;
ALTER TABLE platform.public_resource_ids ADD CONSTRAINT public_resource_ids_native_id_check CHECK(octet_length(native_id) BETWEEN 1 AND 384);
CREATE FUNCTION platform.public_manga_edge_key(source TEXT,target TEXT) RETURNS TEXT LANGUAGE SQL IMMUTABLE STRICT PARALLEL SAFE AS $$
 SELECT 'edge-v2:' || octet_length(source)::TEXT || ':' || source || octet_length(target)::TEXT || ':' || target
$$;
ALTER FUNCTION platform.public_resource_exists(TEXT,TEXT) RENAME TO public_native_resource_exists_v89;
CREATE FUNCTION platform.public_resource_exists(resource_kind TEXT,resource_native TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT CASE resource_kind
 WHEN 'manga_relation' THEN EXISTS(SELECT 1 FROM platform.catalog_relations WHERE platform.public_manga_edge_key(manga_id,related_id)=resource_native)
 WHEN 'manga_recommendation' THEN EXISTS(SELECT 1 FROM platform.catalog_recommendations WHERE platform.public_manga_edge_key(manga_id,recommended_id)=resource_native)
 ELSE platform.public_native_resource_exists_v89(resource_kind,resource_native) END
$$;
CREATE FUNCTION platform.public_backfill_manga_edge_ids() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin)
 SELECT r.kind,platform.public_native_uuid(n.namespace,r.kind,r.native),r.native,
 NOT EXISTS(SELECT 1 FROM platform.public_resource_ids a WHERE a.kind=r.kind AND a.native_id=r.native AND a.preferred),'native'
 FROM (
  SELECT 'manga_relation'::TEXT kind,platform.public_manga_edge_key(manga_id,related_id) native FROM platform.catalog_relations
  UNION ALL SELECT 'manga_recommendation',platform.public_manga_edge_key(manga_id,recommended_id) FROM platform.catalog_recommendations
 )r CROSS JOIN platform.catalogue_namespace n WHERE n.singleton
 ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id;
END $$;
CREATE OR REPLACE FUNCTION platform.public_backfill_resource_ids() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 PERFORM platform.public_backfill_native_resource_ids();
 PERFORM platform.public_backfill_publisher_aliases();
 PERFORM platform.public_backfill_manga_edge_ids();
END $$;
CREATE FUNCTION platform.public_create_manga_edge_alias() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 PERFORM platform.public_ensure_resource_id(TG_ARGV[0],platform.public_manga_edge_key(NEW.manga_id,to_jsonb(NEW)->>TG_ARGV[1]));
 RETURN NEW;
END $$;
CREATE TRIGGER public_relation_alias AFTER INSERT OR UPDATE OF manga_id,related_id ON platform.catalog_relations FOR EACH ROW EXECUTE FUNCTION platform.public_create_manga_edge_alias('manga_relation','related_id');
CREATE TRIGGER public_recommendation_alias AFTER INSERT OR UPDATE OF manga_id,recommended_id ON platform.catalog_recommendations FOR EACH ROW EXECUTE FUNCTION platform.public_create_manga_edge_alias('manga_recommendation','recommended_id');
SELECT platform.public_backfill_manga_edge_ids();
