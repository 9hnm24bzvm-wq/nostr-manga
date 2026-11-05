-- Stable sessions are bound to the inventory domain, never just filter/prefix.
ALTER TABLE inventory_snapshot_slots ADD COLUMN inventory TEXT NOT NULL DEFAULT 'chapters' CHECK(inventory IN ('chapters','series'));
