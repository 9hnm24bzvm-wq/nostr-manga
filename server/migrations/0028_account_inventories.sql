ALTER TABLE inventory_snapshot_slots DROP CONSTRAINT inventory_snapshot_slots_inventory_check;
ALTER TABLE inventory_snapshot_slots ADD CONSTRAINT inventory_snapshot_slots_inventory_check CHECK(inventory IN ('chapters','series','profiles','deletions'));
