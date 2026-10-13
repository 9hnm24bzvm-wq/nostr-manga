CREATE TABLE platform.api_clients (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES platform.users(id),
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    secret_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(secret_hash)=32),
    scopes JSONB NOT NULL CHECK (jsonb_typeof(scopes)='array'),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp() + interval '90 days',
    revoked_at TIMESTAMPTZ
);
CREATE INDEX api_clients_owner_active ON platform.api_clients(user_id) WHERE revoked_at IS NULL;
