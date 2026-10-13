-- Stable, public alias namespace; no authentication or signing authority.
-- A fresh catalogue receives a fresh UUID. Backup/restore preserves its identity.
CREATE TABLE platform.catalogue_namespace (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 namespace UUID NOT NULL UNIQUE DEFAULT gen_random_uuid()
);
INSERT INTO platform.catalogue_namespace(singleton) VALUES(TRUE);
