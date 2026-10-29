-- Corrections append a new fact. They never modify old decisions or source events.
ALTER TABLE platform.report_audit DROP CONSTRAINT report_audit_operation_check;
ALTER TABLE platform.report_audit ADD CONSTRAINT report_audit_operation_check
 CHECK(operation IN ('create','accept','reject','delete','claim','unclaim','assign','progress','priority','appeal','reopen','correct'));
ALTER TABLE platform.reports ADD COLUMN appeal_state TEXT NOT NULL DEFAULT 'none'
 CHECK(appeal_state IN ('none','pending','reviewed'));
CREATE INDEX report_audit_actor_target ON platform.report_audit(actor_id,report_id,created_at DESC,id DESC);
CREATE TABLE platform.report_corrections (
 decision_id BIGINT PRIMARY KEY REFERENCES platform.report_audit(id),
 correction_id BIGINT NOT NULL UNIQUE REFERENCES platform.report_audit(id),
 actor_id TEXT NOT NULL REFERENCES platform.users(id),
 reason TEXT NOT NULL CHECK(octet_length(reason) BETWEEN 1 AND 4000),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TRIGGER report_corrections_append_only BEFORE UPDATE OR DELETE ON platform.report_corrections
 FOR EACH ROW EXECUTE FUNCTION platform.staff_append_only();
