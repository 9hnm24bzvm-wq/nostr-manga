CREATE TABLE platform.catalog_manga (
 id TEXT PRIMARY KEY CHECK(length(id) BETWEEN 1 AND 128), owner_id TEXT NOT NULL, owner_name TEXT NOT NULL,
 title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 500),
 year INTEGER NOT NULL DEFAULT 0 CHECK(year BETWEEN 0 AND 9999), language TEXT NOT NULL CHECK(length(language) BETWEEN 2 AND 12),
 demographic TEXT CHECK(demographic IN ('shounen','shoujo','josei','seinen','none')),
 publication_status TEXT NOT NULL CHECK(publication_status IN ('ongoing','completed','hiatus','cancelled')),
 content_rating TEXT NOT NULL CHECK(content_rating IN ('safe','suggestive','erotica','pornographic')),
 state TEXT NOT NULL DEFAULT 'draft' CHECK(state IN ('draft','submitted','published','rejected')),
 metadata JSONB NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(metadata)='object'),
 locked BOOLEAN NOT NULL DEFAULT FALSE, version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 published_at TIMESTAMPTZ, deleted_at TIMESTAMPTZ,
 protocol_series_id TEXT UNIQUE
);
CREATE TABLE platform.catalog_manga_tags (manga_id TEXT REFERENCES platform.catalog_manga(id), tag_id TEXT REFERENCES platform.catalog_tags(id),PRIMARY KEY(manga_id,tag_id));
CREATE TABLE platform.catalog_manga_creators (manga_id TEXT REFERENCES platform.catalog_manga(id), author_id TEXT REFERENCES platform.catalog_authors(id), role TEXT CHECK(role IN ('author','artist')), PRIMARY KEY(manga_id,author_id,role));
CREATE INDEX catalog_manga_public_sort ON platform.catalog_manga(created_at DESC,id) WHERE state='published' AND deleted_at IS NULL;
CREATE INDEX catalog_manga_title ON platform.catalog_manga(lower(title),id) WHERE deleted_at IS NULL;
CREATE INDEX catalog_manga_filter ON platform.catalog_manga(language,publication_status,content_rating,year,id) WHERE state='published' AND deleted_at IS NULL;
CREATE INDEX catalog_tag_titles ON platform.catalog_manga_tags(tag_id,manga_id);
CREATE INDEX catalog_creator_titles ON platform.catalog_manga_creators(author_id,role,manga_id);
