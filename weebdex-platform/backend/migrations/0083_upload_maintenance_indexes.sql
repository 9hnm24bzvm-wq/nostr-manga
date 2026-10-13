-- Bounded maintenance/retry eligibility must not scan every durable command.
CREATE INDEX protocol_publication_session_state
 ON platform.protocol_publication_commands(upload_session_id,state)
 WHERE upload_session_id IS NOT NULL;
CREATE INDEX media_objects_oldest ON platform.media_objects(created_at,id);
CREATE INDEX upload_session_expiry_batch ON platform.upload_sessions(expires_at,id)
 WHERE state='open';
