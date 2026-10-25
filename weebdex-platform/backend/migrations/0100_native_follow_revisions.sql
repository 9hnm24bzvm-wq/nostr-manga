-- Private membership revisions cover both native sessions/imports and public
-- explicitly bound writes. Removal keeps a revision; it never resets CAS.
-- Block concurrent membership writes until baseline and triggers commit together.
LOCK TABLE platform.library_manga_follows, platform.library_group_follows,
 platform.library_user_follows IN SHARE ROW EXCLUSIVE MODE;
CREATE TABLE platform.library_follow_revisions (
 user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK(kind IN ('manga','group','user')),
 target_id TEXT NOT NULL,
 followed BOOLEAN NOT NULL,
 version INTEGER NOT NULL CHECK(version>0),
 updated_at TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(user_id,kind,target_id)
);
-- Version1 is the migration baseline for an already persisted membership,
-- not an invented historical edit count. Its source follow time is retained.
INSERT INTO platform.library_follow_revisions(user_id,kind,target_id,followed,version,updated_at)
 SELECT user_id,'manga',manga_id,TRUE,1,followed_at FROM platform.library_manga_follows
 UNION ALL SELECT user_id,'group',group_id,TRUE,1,followed_at FROM platform.library_group_follows
 UNION ALL SELECT user_id,'user',target_id,TRUE,1,followed_at FROM platform.library_user_follows;
CREATE FUNCTION platform.record_library_follow_revision() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
DECLARE row_value JSONB; owner_id TEXT; target_id_value TEXT;
BEGIN
 row_value:=CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 owner_id:=row_value->>'user_id'; target_id_value:=row_value->>TG_ARGV[1];
 -- Account deletion removes its own ledger through the FK cascade.
 IF TG_OP='DELETE' AND NOT EXISTS(SELECT 1 FROM platform.users WHERE id=owner_id) THEN RETURN OLD; END IF;
 INSERT INTO platform.library_follow_revisions(user_id,kind,target_id,followed,version,updated_at)
 VALUES(owner_id,TG_ARGV[0],target_id_value,TG_OP<>'DELETE',1,clock_timestamp())
 ON CONFLICT(user_id,kind,target_id) DO UPDATE SET followed=EXCLUDED.followed,
 version=platform.library_follow_revisions.version+1,updated_at=EXCLUDED.updated_at;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF; RETURN NEW;
END $$;
CREATE TRIGGER library_manga_follow_revision AFTER INSERT OR DELETE ON platform.library_manga_follows
 FOR EACH ROW EXECUTE FUNCTION platform.record_library_follow_revision('manga','manga_id');
CREATE TRIGGER library_group_follow_revision AFTER INSERT OR DELETE ON platform.library_group_follows
 FOR EACH ROW EXECUTE FUNCTION platform.record_library_follow_revision('group','group_id');
CREATE TRIGGER library_user_follow_revision AFTER INSERT OR DELETE ON platform.library_user_follows
 FOR EACH ROW EXECUTE FUNCTION platform.record_library_follow_revision('user','target_id');
