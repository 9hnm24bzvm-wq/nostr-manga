CREATE TABLE platform.catalog_relations (
 manga_id TEXT REFERENCES platform.catalog_manga(id), related_id TEXT REFERENCES platform.catalog_manga(id),
 relation_type TEXT NOT NULL CHECK(relation_type IN ('main_story','side_story','prequel','sequel','adapted_from','spin_off','based_on','doujinshi','monochrome','colored','preserialization','serialization','alternate_story','alternate_version','same_franchise','shared_universe')),
 created_by TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(manga_id,related_id), CHECK(manga_id<>related_id)
);
CREATE TABLE platform.catalog_recommendations (
 manga_id TEXT REFERENCES platform.catalog_manga(id), recommended_id TEXT REFERENCES platform.catalog_manga(id),
 score BIGINT NOT NULL DEFAULT 0, PRIMARY KEY(manga_id,recommended_id),CHECK(manga_id<>recommended_id)
);
CREATE INDEX catalog_relations_reverse ON platform.catalog_relations(related_id,manga_id);
