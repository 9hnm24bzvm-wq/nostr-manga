ALTER TABLE platform.sessions ADD COLUMN purpose TEXT NOT NULL DEFAULT 'normal' CHECK (purpose IN ('normal','recovery'));
-- This durable issuance/delivery ledger never stores plaintext recovery links,
-- bearer secrets, message bodies, or recipient addresses. Only hashes survive.
CREATE TABLE platform.auth_email_tokens (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('verification','recovery')),
    user_id TEXT REFERENCES platform.users(id),
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash)=32),
    recipient_hash BYTEA NOT NULL CHECK (octet_length(recipient_hash)=32),
    flow_id TEXT NOT NULL REFERENCES platform.auth_flows(id) ON DELETE CASCADE,
    return_to TEXT NOT NULL,
    delivery_state TEXT NOT NULL CHECK (delivery_state IN ('pending','sent','failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL,
    delivered_at TIMESTAMPTZ,
    used_at TIMESTAMPTZ,
    CHECK (expires_at > created_at)
);
CREATE INDEX auth_email_tokens_expiry ON platform.auth_email_tokens(expires_at);
CREATE INDEX auth_email_tokens_account ON platform.auth_email_tokens(user_id,kind) WHERE used_at IS NULL;
