CREATE TABLE platform.catalog_audit (id BIGSERIAL PRIMARY KEY,actor_id TEXT NOT NULL,entity_type TEXT NOT NULL,entity_id TEXT NOT NULL,operation TEXT NOT NULL,old_version INTEGER,new_version INTEGER,created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp());
CREATE INDEX catalog_audit_entity ON platform.catalog_audit(entity_type,entity_id,id);
-- Protocol ids describe immutable source identities; editable fields remain instance-local overlays.
CREATE TABLE platform.catalog_protocol_links (kind TEXT NOT NULL CHECK(kind IN ('manga','chapter')),catalog_id TEXT NOT NULL,protocol_event_id TEXT NOT NULL,source_version TEXT,linked_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),PRIMARY KEY(kind,catalog_id),UNIQUE(kind,protocol_event_id));
