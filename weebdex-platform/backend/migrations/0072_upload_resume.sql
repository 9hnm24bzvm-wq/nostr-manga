-- Instance-only form state and exact owner-scoped original-command retry input.
-- No tokens, signing keys or raw authenticated requests are stored here.
ALTER TABLE platform.upload_sessions
 ADD COLUMN resume_draft JSONB CHECK(resume_draft IS NULL OR jsonb_typeof(resume_draft)='object'),
 ADD COLUMN resume_page_order TEXT[] NOT NULL DEFAULT '{}'
 CHECK(cardinality(resume_page_order)<=500);
ALTER TABLE platform.protocol_publication_commands
 ADD COLUMN native_commit_payload JSONB
 CHECK(native_commit_payload IS NULL OR jsonb_typeof(native_commit_payload)='object');
