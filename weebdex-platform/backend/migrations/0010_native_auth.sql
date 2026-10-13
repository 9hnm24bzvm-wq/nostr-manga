CREATE TABLE platform.users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL CHECK (char_length(username) BETWEEN 3 AND 32),
    email TEXT NOT NULL CHECK (char_length(email) BETWEEN 3 AND 320),
    password_hash TEXT NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT false,
    disabled BOOLEAN NOT NULL DEFAULT false,
    roles JSONB NOT NULL DEFAULT '["user"]'::jsonb CHECK (jsonb_typeof(roles)='array'),
    permissions JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(permissions)='array'),
    profile JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(profile)='object'),
    preferences JSONB NOT NULL DEFAULT '{"items_per_page":"20","languages":[],"show_safe":true,"show_suggestive":true,"show_erotic":false,"show_hentai":false,"tags":[],"translated_languages":[],"uploaders":[],"groups":[],"data_saver":false,"version":1}'::jsonb,
    follows JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(follows)='object'),
    groups JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(groups)='array'),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE UNIQUE INDEX users_username_key ON platform.users(lower(username));
CREATE UNIQUE INDEX users_email_key ON platform.users(lower(email));
CREATE TABLE platform.sessions (
    id TEXT PRIMARY KEY,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash)=32),
    user_id TEXT NOT NULL REFERENCES platform.users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    authenticated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CHECK (expires_at > created_at)
);
CREATE INDEX sessions_user_active ON platform.sessions(user_id,expires_at) WHERE revoked_at IS NULL;
CREATE TABLE platform.auth_flows (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('login','registration','recovery','verification','settings','logout')),
    browser_hash BYTEA NOT NULL CHECK (octet_length(browser_hash)=32),
    csrf_token TEXT NOT NULL,
    session_id TEXT REFERENCES platform.sessions(id),
    return_to TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    CHECK (expires_at > created_at)
);
CREATE INDEX auth_flows_expiry ON platform.auth_flows(expires_at);
CREATE TABLE platform.auth_rate_limits (
    key_hash BYTEA PRIMARY KEY CHECK (octet_length(key_hash)=32),
    window_ends TIMESTAMPTZ NOT NULL,
    count INTEGER NOT NULL CHECK (count >= 1)
);
CREATE INDEX auth_rate_limits_expiry ON platform.auth_rate_limits(window_ends);
