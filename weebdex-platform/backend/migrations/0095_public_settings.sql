-- Single native preference value source, immutable versioned validation schemas.
-- Reservation93 is permanently retired; this additive migration is95.
CREATE TABLE platform.settings_templates (
 id UUID PRIMARY KEY,
 document JSONB NOT NULL CHECK(jsonb_typeof(document)='object' AND octet_length(document::text)<=131072),
 created_by TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
INSERT INTO platform.settings_templates(id,document) VALUES('3d4e082d-f013-46d9-9b24-643bf65e27ea', $schema${"$schema":"http://json-schema.org/draft-07/schema#","title":"Instance native reading preferences","type":"object","required":["items_per_page","languages","show_safe","show_suggestive","show_erotic","show_hentai","tags","translated_languages","uploaders","groups","data_saver","version"],"properties":{"show_safe":{"type":"boolean"},"show_suggestive":{"type":"boolean"},"show_erotic":{"type":"boolean"},"show_hentai":{"type":"boolean"},"data_saver":{"type":"boolean"},"items_per_page":{"type":"string","pattern":"^(?:[1-9][0-9]?|100)$"},"version":{"type":"integer","minimum":1,"maximum":2147483647},"languages":{"type":"array","maxItems":100,"uniqueItems":true,"items":{"type":"string","maxLength":128}},"tags":{"type":"array","maxItems":100,"uniqueItems":true,"items":{"type":"string","maxLength":128}},"translated_languages":{"type":"array","maxItems":100,"uniqueItems":true,"items":{"type":"string","maxLength":128}},"uploaders":{"type":"array","maxItems":100,"uniqueItems":true,"items":{"type":"string","maxLength":128}},"groups":{"type":"array","maxItems":100,"uniqueItems":true,"items":{"type":"string","maxLength":128}}},"additionalProperties":true}$schema$::jsonb);
CREATE TABLE platform.settings_template_head (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 template_id UUID NOT NULL REFERENCES platform.settings_templates(id)
);
INSERT INTO platform.settings_template_head VALUES(TRUE,'3d4e082d-f013-46d9-9b24-643bf65e27ea');
ALTER TABLE platform.users ADD COLUMN preferences_template_id UUID NOT NULL DEFAULT '3d4e082d-f013-46d9-9b24-643bf65e27ea' REFERENCES platform.settings_templates(id);
ALTER TABLE platform.users ADD COLUMN preferences_updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp();
CREATE FUNCTION platform.settings_templates_immutable() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Settings templates are immutable'; END $$;
CREATE TRIGGER settings_templates_immutable BEFORE UPDATE OR DELETE ON platform.settings_templates FOR EACH ROW EXECUTE FUNCTION platform.settings_templates_immutable();
CREATE FUNCTION platform.preferences_timestamp() RETURNS TRIGGER LANGUAGE plpgsql AS $$
 BEGIN
  IF NEW.preferences IS DISTINCT FROM OLD.preferences OR NEW.preferences_template_id IS DISTINCT FROM OLD.preferences_template_id THEN
   NEW.preferences_updated_at := greatest(clock_timestamp(),OLD.preferences_updated_at + interval '1 microsecond');
  ELSE NEW.preferences_updated_at := OLD.preferences_updated_at;
  END IF;
  RETURN NEW;
 END
$$;
CREATE TRIGGER preferences_timestamp BEFORE UPDATE ON platform.users FOR EACH ROW EXECUTE FUNCTION platform.preferences_timestamp();
-- Existing sysops must still prove their separate role AND this explicit grant.
UPDATE platform.users SET permissions=permissions || '["settings.template.create"]'::jsonb WHERE roles ? 'sysop' AND NOT permissions ? 'settings.template.create';
