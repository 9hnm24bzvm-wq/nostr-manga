CREATE TABLE platform.catalog_authors (
 id TEXT PRIMARY KEY CHECK(length(id) BETWEEN 1 AND 128),
 owner_id TEXT NOT NULL, owner_name TEXT NOT NULL,
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 200),
 metadata JSONB NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(metadata)='object'),
 locked BOOLEAN NOT NULL DEFAULT FALSE, version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), deleted_at TIMESTAMPTZ
);
CREATE TABLE platform.catalog_groups (
 id TEXT PRIMARY KEY CHECK(length(id) BETWEEN 1 AND 128), owner_id TEXT NOT NULL, owner_name TEXT NOT NULL,
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 200), metadata JSONB NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(metadata)='object'),
 locked BOOLEAN NOT NULL DEFAULT FALSE, inactive BOOLEAN NOT NULL DEFAULT FALSE,
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), deleted_at TIMESTAMPTZ
);
CREATE TABLE platform.catalog_group_members (
 group_id TEXT REFERENCES platform.catalog_groups(id), user_id TEXT NOT NULL, name TEXT NOT NULL,
 is_leader BOOLEAN NOT NULL DEFAULT FALSE, is_officer BOOLEAN NOT NULL DEFAULT FALSE,
 PRIMARY KEY(group_id,user_id)
);
CREATE TABLE platform.catalog_tags (
 id TEXT PRIMARY KEY, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100),
 tag_group TEXT NOT NULL CHECK(tag_group IN ('content','format','genre','theme','tag'))
);
CREATE INDEX catalog_author_name ON platform.catalog_authors(lower(name),id) WHERE deleted_at IS NULL;
CREATE INDEX catalog_group_name ON platform.catalog_groups(lower(name),id) WHERE deleted_at IS NULL;
CREATE INDEX catalog_group_member_user ON platform.catalog_group_members(user_id,group_id);
