-- Existing unbound inhibitors remain fail-closed. No enabled exporter existed
-- before this migration; only explicit offline recovery may clear legacy rows.
ALTER TABLE public_export_holds ADD COLUMN root_identity BYTEA
 CHECK(root_identity IS NULL OR octet_length(root_identity)=32);
CREATE INDEX public_export_drain_queue ON public_export_holds
 (owner,volume,root_identity,updated_at,hash)
 WHERE state='draining' AND withdrawal IS NOT NULL;
