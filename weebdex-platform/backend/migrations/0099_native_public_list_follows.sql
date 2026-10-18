-- Private native-account membership pointing only to typed original public
-- protocol-list identities. This is not a signed graph ownership/mutation.
CREATE TABLE platform.library_public_list_follows (
 user_id TEXT NOT NULL REFERENCES platform.users(id),
 list_key TEXT NOT NULL CHECK(list_key ~ '^protocol-list:[0-9a-f]{64}:[0-9a-f]{32}$'),
 followed BOOLEAN NOT NULL,
 version INTEGER NOT NULL CHECK(version>0),
 followed_at TIMESTAMPTZ,
 source_head_at_follow TEXT CHECK(source_head_at_follow ~ '^[0-9a-f]{64}$'),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(user_id,list_key),
 CHECK((followed_at IS NOT NULL)=followed),
 CHECK(NOT followed OR source_head_at_follow IS NOT NULL)
);
CREATE INDEX library_public_list_follows_owner ON platform.library_public_list_follows(user_id,followed_at DESC,list_key) WHERE followed;
