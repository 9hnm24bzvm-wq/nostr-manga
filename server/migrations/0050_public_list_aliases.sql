-- List aliases are scoped by publishing key and exact native list ID.
ALTER TABLE compatibility_ids DROP CONSTRAINT compatibility_ids_kind_check;
ALTER TABLE compatibility_ids ADD CONSTRAINT compatibility_ids_kind_check CHECK(kind IN('manga','chapter','cover','user','group','author','tag','list'));
