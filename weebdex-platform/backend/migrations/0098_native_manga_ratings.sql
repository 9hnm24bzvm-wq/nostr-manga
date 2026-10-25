-- Observed instance-account ratings. Existing aggregate counters are never
-- converted into invented per-user votes. Tombstones preserve CAS versions.
CREATE TABLE platform.library_manga_ratings (
 user_id TEXT NOT NULL REFERENCES platform.users(id),
 manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id),
 rating SMALLINT CHECK(rating BETWEEN 1 AND 10),
 version INTEGER NOT NULL CHECK(version>0),
 rated_at TIMESTAMPTZ,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(user_id,manga_id),
 CHECK((rating IS NULL)=(rated_at IS NULL))
);
CREATE INDEX library_manga_ratings_aggregate ON platform.library_manga_ratings(manga_id,rating) WHERE rating IS NOT NULL;
