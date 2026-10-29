-- Private proof-of-control association only. Never a protocol publisher binding,
-- native login credential, public profile field, admission, role or permission.
CREATE TABLE platform.native_identity_links (
    user_id text PRIMARY KEY REFERENCES platform.users(id),
    public_key text CHECK (public_key IS NULL OR public_key ~ '^[0-9a-f]{64}$'),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE UNIQUE INDEX native_identity_link_one_account_per_key
    ON platform.native_identity_links(public_key) WHERE public_key IS NOT NULL;
CREATE TABLE platform.native_identity_link_challenges (
    id text PRIMARY KEY,
    user_id text NOT NULL REFERENCES platform.users(id),
    session_id text NOT NULL REFERENCES platform.sessions(id),
    action text NOT NULL CHECK (action IN ('link','unlink')),
    public_key text NOT NULL CHECK (public_key ~ '^[0-9a-f]{64}$'),
    revision bigint NOT NULL,
    payload text NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at timestamptz,
    outcome text CHECK (outcome IN ('replaced','linked','unlinked','account-closed')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX native_identity_link_challenge_owner ON platform.native_identity_link_challenges(user_id,created_at);
