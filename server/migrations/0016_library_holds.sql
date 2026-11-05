CREATE TABLE library_object_holds (
 publisher BYTEA NOT NULL REFERENCES account_permissions(publisher),
 hash BYTEA NOT NULL,
 generation BYTEA NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('current','pending','grace')),
 wait_event BYTEA REFERENCES events(id),
 grace_until TIMESTAMPTZ,
 PRIMARY KEY(publisher,hash,generation),
 FOREIGN KEY(hash,generation) REFERENCES blob_intents(hash,generation),
 CHECK((state='current' AND wait_event IS NULL AND grace_until IS NULL)
    OR (state='pending' AND wait_event IS NOT NULL AND grace_until IS NULL)
    OR (state='grace' AND wait_event IS NULL AND grace_until IS NOT NULL))
);
CREATE INDEX library_holds_current_author ON library_object_holds(publisher,hash) WHERE state='current';
CREATE INDEX library_holds_active_object ON library_object_holds(hash,generation) WHERE state IN ('current','pending');
CREATE INDEX library_holds_grace_object ON library_object_holds(hash,generation,grace_until DESC) WHERE state='grace';
CREATE INDEX library_holds_backend_wait ON library_object_holds(wait_event,hash) WHERE state='pending';
CREATE INDEX library_holds_grace_cleanup ON library_object_holds(grace_until,publisher,hash) WHERE state='grace';
