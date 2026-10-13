-- Instance-cookie library state is private by default. No Nostr key or relay publication.
CREATE TABLE platform.library_manga_follows (
 user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id),
 reading_status TEXT NOT NULL CHECK(reading_status IN ('reading','plan_to_read','completed','on_hold','re_reading','dropped')),
 notification BOOLEAN NOT NULL DEFAULT TRUE, is_private BOOLEAN NOT NULL DEFAULT TRUE,
 followed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(user_id,manga_id)
);
CREATE INDEX library_follows_owner_date ON platform.library_manga_follows(user_id,followed_at DESC,manga_id);
CREATE INDEX library_follows_owner_status ON platform.library_manga_follows(user_id,reading_status,manga_id);
CREATE INDEX library_follows_manga ON platform.library_manga_follows(manga_id,user_id);
CREATE TABLE platform.library_user_follows (
 user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 target_id TEXT NOT NULL REFERENCES platform.users(id), followed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(user_id,target_id), CHECK(user_id<>target_id)
);
CREATE INDEX library_user_follows_owner_date ON platform.library_user_follows(user_id,followed_at DESC,target_id);
CREATE TABLE platform.library_group_follows (
 user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 group_id TEXT NOT NULL REFERENCES platform.catalog_groups(id), followed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(user_id,group_id)
);
CREATE INDEX library_group_follows_owner_date ON platform.library_group_follows(user_id,followed_at DESC,group_id);
CREATE TABLE platform.library_chapter_history (
 user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id), chapter_id TEXT NOT NULL REFERENCES platform.catalog_chapters(id),
 read_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(user_id,chapter_id)
);
CREATE INDEX library_history_owner_manga ON platform.library_chapter_history(user_id,manga_id,chapter_id);
CREATE INDEX library_history_owner_date ON platform.library_chapter_history(user_id,read_at DESC,chapter_id);
CREATE TABLE platform.library_chapter_progress (
 user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 chapter_id TEXT NOT NULL REFERENCES platform.catalog_chapters(id), page INTEGER NOT NULL CHECK(page>=0),
 completed BOOLEAN NOT NULL DEFAULT FALSE, version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(user_id,chapter_id)
);
CREATE TABLE platform.library_lists (
 id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES platform.users(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK(char_length(name) BETWEEN 1 AND 200), description TEXT NOT NULL DEFAULT '' CHECK(octet_length(description)<=20000),
 visibility TEXT NOT NULL DEFAULT 'private' CHECK(visibility IN ('private','unlisted','public')),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX library_lists_owner ON platform.library_lists(user_id,created_at DESC,id);
CREATE INDEX library_lists_public ON platform.library_lists(user_id,created_at DESC,id) WHERE visibility='public';
CREATE TABLE platform.library_list_members (
 list_id TEXT NOT NULL REFERENCES platform.library_lists(id) ON DELETE CASCADE,
 manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id), position INTEGER NOT NULL CHECK(position>=0),
 PRIMARY KEY(list_id,manga_id), UNIQUE(list_id,position)
);
-- A real persisted discussion root is required by the unchanged user profile layout.
ALTER TABLE platform.catalog_threads DROP CONSTRAINT catalog_threads_entity_type_check;
ALTER TABLE platform.catalog_threads ADD CONSTRAINT catalog_threads_entity_type_check CHECK(entity_type IN ('manga','chapter','group','user'));
CREATE FUNCTION platform.library_create_user_thread() RETURNS TRIGGER LANGUAGE plpgsql AS $$
 BEGIN
 INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject)
 VALUES('user_thread_' || NEW.id,'user',NEW.id,NEW.id,NEW.username,NEW.username || ' profile discussion') ON CONFLICT(entity_type,entity_id) DO NOTHING;
 RETURN NEW;
 END
$$;
CREATE TRIGGER library_user_thread AFTER INSERT ON platform.users FOR EACH ROW EXECUTE FUNCTION platform.library_create_user_thread();
INSERT INTO platform.catalog_threads(id,entity_type,entity_id,author_id,author_name,subject)
 SELECT 'user_thread_' || id,'user',id,id,username,username || ' profile discussion' FROM platform.users ON CONFLICT(entity_type,entity_id) DO NOTHING;
CREATE FUNCTION platform.library_user_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(u.profile || jsonb_build_object('id',u.id,'name',u.username,'version',u.version,'roles',u.roles,
 'relationships',jsonb_build_object('groups',COALESCE((SELECT jsonb_agg(platform.catalog_group_json(g.id) ORDER BY g.name,g.id) FROM platform.catalog_group_members gm JOIN platform.catalog_groups g ON g.id=gm.group_id WHERE gm.user_id=u.id AND g.deleted_at IS NULL),'[]'::jsonb),'thread',platform.catalog_thread_json('user',u.id))))
 FROM platform.users u WHERE u.id=value AND NOT u.disabled
$$;
