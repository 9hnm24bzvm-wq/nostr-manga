-- Blob deletion's foreign-key check searches all library reference states.
-- Partial active/grace indexes cannot support that state-free lookup. Keep the
-- complete object-leading index even when those specialized indexes exist.
CREATE INDEX library_holds_object_reference ON library_object_holds(hash,generation);
