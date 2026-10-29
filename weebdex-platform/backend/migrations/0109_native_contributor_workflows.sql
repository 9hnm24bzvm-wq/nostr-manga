-- Private author-facing feedback. This is not the private moderator audit log.
CREATE TABLE platform.native_draft_reviews (
 manga_id TEXT NOT NULL REFERENCES platform.catalog_manga(id),
 version INTEGER NOT NULL CHECK(version>0),
 action TEXT NOT NULL CHECK(action IN ('submitted','published','rejected')),
 actor_id TEXT NOT NULL REFERENCES platform.users(id),
 reason TEXT NOT NULL DEFAULT '' CHECK(length(reason)<=2000),
 affected_fields TEXT[] NOT NULL DEFAULT '{}' CHECK(cardinality(affected_fields)<=16),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(manga_id,version),
 CHECK(action<>'rejected' OR length(btrim(reason))>0)
);
-- Account-targeted, single-use local team requests; no bearer secrets or email.
CREATE TABLE platform.native_group_requests (
 id UUID PRIMARY KEY,
 group_id TEXT NOT NULL REFERENCES platform.catalog_groups(id),
 user_id TEXT NOT NULL REFERENCES platform.users(id),
 created_by TEXT NOT NULL REFERENCES platform.users(id),
 kind TEXT NOT NULL CHECK(kind IN ('invitation','application')),
 status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','accepted','declined','revoked','cancelled','expired')),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 message TEXT NOT NULL DEFAULT '' CHECK(length(message)<=1000),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 expires_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()+interval '7 days',
 decided_at TIMESTAMPTZ,
 decided_by TEXT REFERENCES platform.users(id),
 CHECK(expires_at>created_at)
);
CREATE UNIQUE INDEX native_group_request_pending ON platform.native_group_requests(group_id,user_id,kind) WHERE status='pending';
CREATE INDEX native_group_request_inbox ON platform.native_group_requests(user_id,created_at DESC,id);
CREATE INDEX native_group_request_group ON platform.native_group_requests(group_id,created_at DESC,id);
-- Informational attribution only. Never an authority, membership or role grant.
CREATE TABLE platform.native_chapter_credits (
 chapter_id TEXT PRIMARY KEY REFERENCES platform.catalog_chapters(id),
 credits JSONB NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(credits)='array' AND jsonb_array_length(credits)<=30),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 updated_by TEXT NOT NULL REFERENCES platform.users(id),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
