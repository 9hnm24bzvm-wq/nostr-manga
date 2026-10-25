-- Durable instance-local aliases for native report reasons. Reports and audit
-- decisions stay private local moderation state, with no relay publication.
ALTER TABLE platform.public_resource_ids DROP CONSTRAINT public_resource_ids_kind_check;
ALTER TABLE platform.public_resource_ids ADD CONSTRAINT public_resource_ids_kind_check CHECK(kind IN ('manga','chapter','cover','author','group','tag','user','list','report','report_reason','upload_session','upload_file','client','manga_relation','manga_recommendation'));
ALTER TABLE platform.report_reasons ADD COLUMN version INTEGER NOT NULL DEFAULT 1 CHECK(version>0);
CREATE FUNCTION platform.report_reason_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.id IS DISTINCT FROM NEW.id OR OLD.category IS DISTINCT FROM NEW.category THEN
  RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='report reason identity is immutable';
 END IF;
 IF OLD.reason IS DISTINCT FROM NEW.reason OR OLD.details_required IS DISTINCT FROM NEW.details_required OR OLD.enabled IS DISTINCT FROM NEW.enabled THEN
  NEW.version := OLD.version+1;
 ELSE
  NEW.version := OLD.version;
 END IF;
 RETURN NEW;
END
$$;
CREATE TRIGGER report_reason_version BEFORE UPDATE ON platform.report_reasons FOR EACH ROW EXECUTE FUNCTION platform.report_reason_version();

CREATE OR REPLACE FUNCTION platform.public_resource_exists(resource_kind TEXT,resource_native TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT CASE resource_kind
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga WHERE id=resource_native)
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters WHERE id=resource_native)
 WHEN 'cover' THEN EXISTS(SELECT 1 FROM platform.catalog_covers WHERE id=resource_native)
 WHEN 'author' THEN EXISTS(SELECT 1 FROM platform.catalog_authors WHERE id=resource_native)
 WHEN 'group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups WHERE id=resource_native)
 WHEN 'tag' THEN EXISTS(SELECT 1 FROM platform.catalog_tags WHERE id=resource_native)
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users WHERE id=resource_native)
 WHEN 'list' THEN EXISTS(SELECT 1 FROM platform.library_lists WHERE id=resource_native)
 WHEN 'report_reason' THEN EXISTS(SELECT 1 FROM platform.report_reasons WHERE id=resource_native)
 WHEN 'report' THEN EXISTS(SELECT 1 FROM platform.reports WHERE id=resource_native)
 WHEN 'upload_session' THEN EXISTS(SELECT 1 FROM platform.upload_sessions WHERE id=resource_native)
 WHEN 'upload_file' THEN EXISTS(SELECT 1 FROM platform.upload_files WHERE id=resource_native)
 WHEN 'client' THEN EXISTS(SELECT 1 FROM platform.api_clients WHERE id=resource_native)
 WHEN 'manga_relation' THEN EXISTS(SELECT 1 FROM platform.catalog_relations WHERE platform.public_manga_edge_key(manga_id,related_id)=resource_native)
 WHEN 'manga_recommendation' THEN EXISTS(SELECT 1 FROM platform.catalog_recommendations WHERE platform.public_manga_edge_key(manga_id,recommended_id)=resource_native)
 ELSE FALSE END
$$;

CREATE FUNCTION platform.public_backfill_report_reason_ids() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO platform.public_resource_ids(kind,public_id,native_id,preferred,origin)
 SELECT 'report_reason',platform.public_native_uuid(n.namespace,'report_reason',r.id),r.id,
 NOT EXISTS(SELECT 1 FROM platform.public_resource_ids a WHERE a.kind='report_reason' AND a.native_id=r.id AND a.preferred),'native'
 FROM platform.report_reasons r CROSS JOIN platform.catalogue_namespace n WHERE n.singleton
 ON CONFLICT(kind,public_id) DO UPDATE SET native_id=EXCLUDED.native_id WHERE platform.public_resource_ids.native_id IS DISTINCT FROM EXCLUDED.native_id;
END
$$;
CREATE OR REPLACE FUNCTION platform.public_backfill_resource_ids() RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
 PERFORM platform.public_backfill_native_resource_ids();
 PERFORM platform.public_backfill_publisher_aliases();
 PERFORM platform.public_backfill_manga_edge_ids();
 PERFORM platform.public_backfill_report_reason_ids();
END
$$;
SELECT platform.public_backfill_report_reason_ids();
CREATE TRIGGER public_report_reason_alias AFTER INSERT ON platform.report_reasons FOR EACH ROW EXECUTE FUNCTION platform.public_create_native_alias('report_reason');
