ALTER TABLE node_identity ADD COLUMN restore_generation BIGINT NOT NULL DEFAULT 0 CHECK(restore_generation>=0);
ALTER TABLE node_identity ADD COLUMN auth_not_before BIGINT NOT NULL DEFAULT 0 CHECK(auth_not_before>=0);
ALTER TABLE node_identity ADD COLUMN restored_at TIMESTAMPTZ;
