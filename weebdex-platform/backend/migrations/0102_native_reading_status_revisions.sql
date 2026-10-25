-- Status changes are independent of membership and wall-clock precision.
-- Observe the real native/session/import source, including removal tombstones.
LOCK TABLE platform.library_manga_follows IN SHARE ROW EXCLUSIVE MODE;
CREATE TABLE platform.library_reading_status_revisions (
 user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id),
 status TEXT CHECK(status IN ('reading','plan_to_read','completed','on_hold','re_reading','dropped')),
 followed BOOLEAN NOT NULL,
 version INTEGER NOT NULL CHECK(version>0),
 updated_at TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(user_id,manga_id),
 CHECK(followed=(status IS NOT NULL))
);
-- Baseline1 describes an existing observation, never an invented edit count.
INSERT INTO platform.library_reading_status_revisions(user_id,manga_id,status,followed,version,updated_at)
 SELECT user_id,manga_id,reading_status,TRUE,1,updated_at FROM platform.library_manga_follows;
-- Preserve an already known removed membership as an observed baseline
-- tombstone, without inventing its former status or a historical edit count.
INSERT INTO platform.library_reading_status_revisions(user_id,manga_id,status,followed,version,updated_at)
 SELECT r.user_id,r.target_id,NULL,FALSE,1,r.updated_at
 FROM platform.library_follow_revisions r JOIN platform.catalog_manga m ON m.id=r.target_id
 WHERE r.kind='manga' AND NOT r.followed ON CONFLICT(user_id,manga_id) DO NOTHING;
CREATE FUNCTION platform.record_library_reading_status_revision() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
DECLARE owner_id TEXT; target_id TEXT; status_value TEXT;
BEGIN
 IF TG_OP='UPDATE' AND OLD.reading_status IS NOT DISTINCT FROM NEW.reading_status THEN RETURN NEW; END IF;
 IF TG_OP='DELETE' THEN owner_id:=OLD.user_id; target_id:=OLD.manga_id; status_value:=NULL;
 ELSE owner_id:=NEW.user_id; target_id:=NEW.manga_id; status_value:=NEW.reading_status; END IF;
 IF TG_OP='DELETE' AND NOT EXISTS(SELECT 1 FROM platform.users WHERE id=owner_id) THEN RETURN OLD; END IF;
 INSERT INTO platform.library_reading_status_revisions(user_id,manga_id,status,followed,version,updated_at)
 VALUES(owner_id,target_id,status_value,TG_OP<>'DELETE',1,clock_timestamp())
 ON CONFLICT(user_id,manga_id) DO UPDATE SET status=EXCLUDED.status,followed=EXCLUDED.followed,
 version=platform.library_reading_status_revisions.version+1,updated_at=EXCLUDED.updated_at;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF; RETURN NEW;
END $$;
CREATE TRIGGER library_reading_status_revision AFTER INSERT OR DELETE OR UPDATE OF reading_status
 ON platform.library_manga_follows FOR EACH ROW EXECUTE FUNCTION platform.record_library_reading_status_revision();
