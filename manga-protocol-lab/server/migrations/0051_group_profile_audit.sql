-- Public group metadata is signed by its existing ordinary key; actor audit stays local.
ALTER TABLE group_action_audit DROP CONSTRAINT group_action_audit_action_check;
ALTER TABLE group_action_audit ADD CONSTRAINT group_action_audit_action_check CHECK(action IN('publish','deletion_request','profile'));
