-- Derived typed metadata; the signed event and exact canonical blob remain authority.
CREATE TABLE catalogue_metadata (
 event_id BYTEA PRIMARY KEY REFERENCES events(id),
 metadata JSONB NOT NULL CHECK(jsonb_typeof(metadata)='object'),
 indexed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX catalogue_metadata_gin ON catalogue_metadata USING GIN(metadata jsonb_path_ops);
CREATE TABLE catalogue_index_errors(event_id BYTEA PRIMARY KEY REFERENCES events(id),reason TEXT NOT NULL,observed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp());
ALTER TABLE catalogue_metadata ADD COLUMN title_values TEXT[] COLLATE "C" NOT NULL DEFAULT '{}';
CREATE INDEX catalogue_titles_gin ON catalogue_metadata USING GIN(title_values);
CREATE INDEX catalogue_status ON catalogue_metadata((metadata->>'status') COLLATE "C");
CREATE INDEX catalogue_year ON catalogue_metadata(((metadata->>'year')::bigint));
ALTER TABLE events ADD COLUMN volume TEXT;
ALTER TABLE events ADD COLUMN title TEXT;
