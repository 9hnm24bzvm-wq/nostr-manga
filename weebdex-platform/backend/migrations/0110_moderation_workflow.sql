-- Instance-private workflow. Report decision status remains independent of work state.
ALTER TABLE platform.reports
 ADD COLUMN assigned_to TEXT REFERENCES platform.users(id),
 ADD COLUMN work_state TEXT NOT NULL DEFAULT 'unassigned' CHECK(work_state IN ('unassigned','assigned','in_progress')),
 ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal' CHECK(priority IN ('low','normal','high','urgent')),
 ADD CONSTRAINT reports_assignment_consistent CHECK((assigned_to IS NULL)=(work_state='unassigned'));
CREATE INDEX reports_assigned_work ON platform.reports(assigned_to,work_state,priority,created_at,id) WHERE deleted_at IS NULL AND status='waiting';
ALTER TABLE platform.report_audit DROP CONSTRAINT report_audit_operation_check;
ALTER TABLE platform.report_audit ADD CONSTRAINT report_audit_operation_check
 CHECK(operation IN ('create','accept','reject','delete','claim','unclaim','assign','progress','priority'));

CREATE FUNCTION platform.staff_append_only() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION USING ERRCODE='23514',MESSAGE='Staff history and receipts are append-only'; END $$;
CREATE TRIGGER report_audit_append_only BEFORE UPDATE OR DELETE ON platform.report_audit
 FOR EACH ROW EXECUTE FUNCTION platform.staff_append_only();
CREATE TABLE platform.staff_report_receipts (
 actor_id TEXT NOT NULL REFERENCES platform.users(id),
 request_id TEXT NOT NULL CHECK(request_id ~ '^[A-Za-z0-9_-]{16,80}$'),
 request_hash TEXT NOT NULL CHECK(request_hash ~ '^[a-f0-9]{64}$'),
 report_id TEXT NOT NULL REFERENCES platform.reports(id),
 receipt JSONB NOT NULL CHECK(jsonb_typeof(receipt)='object' AND octet_length(receipt::text)<=131072),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(actor_id,request_id)
);
CREATE TRIGGER staff_report_receipts_append_only BEFORE UPDATE OR DELETE ON platform.staff_report_receipts
 FOR EACH ROW EXECUTE FUNCTION platform.staff_append_only();
