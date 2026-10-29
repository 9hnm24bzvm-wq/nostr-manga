-- Metadata only: one monotonic CAS token for the existing native account library.
-- Source-table triggers cover legacy writers and deletions, not only this adapter.
LOCK TABLE platform.library_manga_follows, platform.library_chapter_history,
 platform.library_chapter_progress, platform.library_lists, platform.library_list_members
 IN SHARE ROW EXCLUSIVE MODE;
CREATE TABLE platform.library_account_revisions (
 user_id TEXT PRIMARY KEY REFERENCES platform.users(id) ON DELETE CASCADE,
 revision BIGINT NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991)
);
INSERT INTO platform.library_account_revisions(user_id,revision)
 SELECT DISTINCT user_id,1 FROM (
 SELECT user_id FROM platform.library_manga_follows UNION SELECT user_id FROM platform.library_chapter_history
 UNION SELECT user_id FROM platform.library_chapter_progress UNION SELECT user_id FROM platform.library_lists
 ) source;
CREATE FUNCTION platform.record_account_library_revision() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE owner_id TEXT; old_value JSONB; new_value JSONB; owner_ids TEXT[];
BEGIN
 IF TG_OP='UPDATE' AND (to_jsonb(OLD)-'updated_at') IS NOT DISTINCT FROM (to_jsonb(NEW)-'updated_at') THEN RETURN NEW; END IF;
 IF TG_OP<>'INSERT' THEN old_value:=to_jsonb(OLD); END IF;
 IF TG_OP<>'DELETE' THEN new_value:=to_jsonb(NEW); END IF;
 IF TG_TABLE_NAME='library_list_members' THEN
   SELECT array_agg(DISTINCT user_id) INTO owner_ids FROM platform.library_lists
     WHERE id=ANY(ARRAY[old_value->>'list_id',new_value->>'list_id']);
 ELSE owner_ids:=ARRAY[old_value->>'user_id',new_value->>'user_id']; END IF;
 -- All exposed routes keep ownership immutable. Also fence both owners if a
 -- legacy maintenance writer moves a row; sorted locks avoid opposite ordering.
 FOR owner_id IN SELECT DISTINCT value FROM unnest(owner_ids) AS owners(value)
   WHERE value IS NOT NULL ORDER BY value LOOP
   -- Cascading account/list deletion is already fenced by the parent operation.
   PERFORM id FROM platform.users WHERE id=owner_id FOR NO KEY UPDATE;
   IF FOUND AND TG_WHEN='AFTER' THEN
     INSERT INTO platform.library_account_revisions(user_id,revision) VALUES(owner_id,1)
       ON CONFLICT(user_id) DO UPDATE SET revision=platform.library_account_revisions.revision+1;
   END IF;
 END LOOP;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF; RETURN NEW;
END $$;
CREATE TRIGGER account_library_follow_revision BEFORE INSERT OR UPDATE OR DELETE ON platform.library_manga_follows FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_history_revision BEFORE INSERT OR UPDATE OR DELETE ON platform.library_chapter_history FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_progress_revision BEFORE INSERT OR UPDATE OR DELETE ON platform.library_chapter_progress FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_list_revision BEFORE INSERT OR UPDATE OR DELETE ON platform.library_lists FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_members_revision BEFORE INSERT OR UPDATE OR DELETE ON platform.library_list_members FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_follow_changed AFTER INSERT OR UPDATE OR DELETE ON platform.library_manga_follows FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_history_changed AFTER INSERT OR UPDATE OR DELETE ON platform.library_chapter_history FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_progress_changed AFTER INSERT OR UPDATE OR DELETE ON platform.library_chapter_progress FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_list_changed AFTER INSERT OR UPDATE OR DELETE ON platform.library_lists FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
CREATE TRIGGER account_library_members_changed AFTER INSERT OR UPDATE OR DELETE ON platform.library_list_members FOR EACH ROW EXECUTE FUNCTION platform.record_account_library_revision();
