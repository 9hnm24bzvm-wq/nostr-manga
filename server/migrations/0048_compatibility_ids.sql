-- Local UUID facade. These records never alter signed publication identities.
CREATE TABLE compatibility_ids (
 kind TEXT NOT NULL CHECK(kind IN ('manga','chapter','cover','user','group','author','tag')),
 id TEXT NOT NULL CHECK(id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'),
 target TEXT NOT NULL CHECK(octet_length(target)<=1024),
 payload JSONB NOT NULL DEFAULT '{}',
 preferred BOOLEAN NOT NULL DEFAULT FALSE,
 PRIMARY KEY(kind,id)
);
CREATE UNIQUE INDEX compatibility_preferred_target ON compatibility_ids(kind,target) WHERE preferred;
CREATE INDEX compatibility_target ON compatibility_ids(kind,target);
CREATE TABLE compatibility_indexed(event_id BYTEA PRIMARY KEY REFERENCES events(id));
CREATE INDEX compatibility_preferred_targets ON compatibility_ids(target,kind) WHERE preferred;

ALTER TABLE catalogue_metadata ADD COLUMN metadata_version INTEGER NOT NULL DEFAULT 1;
CREATE INDEX catalogue_title_sort ON catalogue_metadata((title_values[1]) COLLATE "C");
