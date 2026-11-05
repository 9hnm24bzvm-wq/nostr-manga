-- Only freshly staged objects need the one-time staging-credit transfer.
CREATE INDEX blob_retention_credits_unreleased ON blob_retention_credits(hash) WHERE NOT staging_released;
-- 0017 duplicated the active-selection index already introduced in0004.
-- Preserve migration history but remove the redundant hot-write index.
DROP INDEX capacity_grants_active_holder;
