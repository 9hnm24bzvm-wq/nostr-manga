-- Server-observed retirement time, never an author-controlled event timestamp.
ALTER TABLE events ADD COLUMN retired_at TIMESTAMPTZ;
UPDATE events SET retired_at=clock_timestamp() WHERE kind=11999 AND state='removed';
CREATE INDEX events_library_retired ON events(retired_at,id)
 WHERE kind=11999 AND state='removed' AND retired_at IS NOT NULL;
-- Deleting old events must not scan all completed outbox rows or all users.
CREATE INDEX backend_outbox_event_history ON backend_outbox(event_id);
CREATE INDEX account_heads_event ON account_heads(event_id) WHERE event_id IS NOT NULL;
CREATE INDEX library_holds_wait_reference ON library_object_holds(wait_event) WHERE wait_event IS NOT NULL;
